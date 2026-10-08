use super::*;

#[test]
fn very_large_native_models_and_inputs_avoid_jvm_constructor_limits() {
    let fields = (0..260)
        .map(|index| poolster_core::Field {
            name: format!("field{index}"),
            value: SchemaValue::new(SchemaKind::String),
            required: false,
            annotations: Default::default(),
        })
        .collect::<Vec<_>>();
    let schema = poolster_core::Schema::new(
        "Large",
        SchemaValue::new(SchemaKind::Object {
            fields,
            additional_properties: poolster_core::AdditionalProperties::Any,
        }),
    );
    let model = super::render_model(&schema, "example", false);
    assert!(model.contains("public final class Large"));
    assert!(model.contains("@JsonProperty(\"field259\") private String field259"));
    assert!(model.contains("@JsonAnySetter"));
    let operation = Operation {
        id: "large".into(),
        parameters: (0..260)
            .map(|index| poolster_core::OperationParameter {
                name: format!("field{index}"),
                location: "query".into(),
                schema: Some(SchemaValue::new(SchemaKind::String)),
                required: false,
                description: None,
                annotations: Default::default(),
            })
            .collect(),
        ..Default::default()
    };
    let mut generated = String::new();
    super::render_operation(&mut generated, &operation);
    assert!(generated.contains("public static final class LargeRequest"));
    assert!(generated.contains("public LargeRequest field259(String value)"));
}

#[test]
fn required_nullable_and_optional_omission_are_distinct() {
    let mut nullable = SchemaValue::new(SchemaKind::String);
    nullable.nullable = true;
    let schema = Schema::new(
        "WireInput",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![
                Field {
                    name: "enabled".into(),
                    value: SchemaValue::new(SchemaKind::Boolean),
                    required: true,
                    annotations: Default::default(),
                },
                Field {
                    name: "count".into(),
                    value: SchemaValue::new(SchemaKind::Integer),
                    required: true,
                    annotations: Default::default(),
                },
                Field {
                    name: "note".into(),
                    value: nullable,
                    required: true,
                    annotations: Default::default(),
                },
                Field {
                    name: "missing".into(),
                    value: SchemaValue::new(SchemaKind::String),
                    required: false,
                    annotations: Default::default(),
                },
            ],
            additional_properties: AdditionalProperties::Forbidden,
        }),
    );
    let model = super::render_model(&schema, "example", false);
    assert!(model.contains(
        "@JsonInclude(JsonInclude.Include.NON_NULL)\n        @JsonProperty(\"missing\")"
    ));
    assert!(model.contains("        @JsonProperty(\"note\") String note"));
}

#[test]
fn open_object_records_flatten_typed_unknown_properties() {
    let mut source = contact_api();
    source.schemas.push(Schema::new(
        "Future",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![Field {
                name: "additionalProperties".into(),
                value: string(),
                required: false,
                annotations: Default::default(),
            }],
            additional_properties: AdditionalProperties::Schema {
                value: Box::new(SchemaValue::new(SchemaKind::String)),
            },
        }),
    ));
    let tree = render_test_sdk(&source, "java", Some("io.example")).unwrap();
    let model = tree
        .get("java/src/main/java/io/example/model/Future.java")
        .unwrap();
    assert!(
        model.contains("@JsonAnyGetter @JsonAnySetter Map<String, String> additionalProperties_")
    );
    assert!(!model.contains("@JsonIgnoreProperties(ignoreUnknown = true)"));
    assert!(
        model.contains("Collections.unmodifiableMap(new LinkedHashMap<>(additionalProperties_))")
    );
    assert!(model.contains("additionalProperties_.containsKey(\"additionalProperties\")"));
    assert!(
        tree.get("java/src/main/java/io/example/model/Contact.java")
            .unwrap()
            .contains("@JsonIgnoreProperties(ignoreUnknown = true)")
    );
}

#[test]
fn open_enums_are_opt_in_and_preserve_unknown_wire_values() {
    let mut source = contact_api();
    let mut value = SchemaValue::new(SchemaKind::String);
    value.enum_values = vec![serde_json::json!("active"), serde_json::json!("paused")];
    source.schemas.push(Schema::new("Status", value));
    let closed = render_sdk(&source, "java", Some("io.example"), SdkClientStyle::Flat).unwrap();
    let open = render_sdk_with_policy(
        &source,
        "java",
        Some("io.example"),
        SdkClientStyle::Flat,
        true,
    )
    .unwrap();
    let path = "java/src/main/java/io/example/model/Status.java";
    assert!(closed.get(path).unwrap().contains("public enum Status"));
    let model = open.get(path).unwrap();
    assert!(model.contains("public final class Status"));
    assert!(model.contains("public static final Status ACTIVE"));
    assert!(model.contains("return new Status(value);"));
    assert!(model.contains("@JsonValue"));
    assert!(model.contains("fromKnownValue"));
    assert!(model.contains("value.equals(candidate.value)"));
}

#[test]
fn emits_buildable_java_17_package_with_typed_transport() {
    let tree = render_test_sdk(&contact_api(), "sdks/java", Some("com.poolster.email")).unwrap();
    assert!(
        tree.get("sdks/java/build.gradle")
            .unwrap()
            .contains("JavaLanguageVersion.of(17)")
    );
    let maven = tree.get("sdks/java/pom.xml").unwrap();
    assert!(maven.contains("<maven.compiler.source>17</maven.compiler.source>"));
    assert!(maven.contains("<maven.compiler.target>17</maven.compiler.target>"));
    assert!(
        tree.get("sdks/java/pom.xml")
            .unwrap()
            .contains("<maven.compiler.release>17</maven.compiler.release>")
    );
    let model = tree
        .get("sdks/java/src/main/java/com/poolster/email/model/Contact.java")
        .unwrap();
    assert!(model.contains("public record Contact("));
    assert!(model.contains("@JsonProperty(\"display_name\") String displayName"));
    let client = rendered_java(&tree);
    assert!(
        tree.get("sdks/java/src/main/java/com/poolster/email/ClientBase.java")
            .is_some()
    );
    assert!(
        tree.get("sdks/java/src/main/java/com/poolster/email/internal/Operations000.java")
            .unwrap()
            .contains("public class Operations000 extends com.poolster.email.ClientBase")
    );
    assert!(
        !tree
            .get("sdks/java/src/main/java/com/poolster/email/Client.java")
            .unwrap()
            .contains("public record GetContactRequest")
    );
    assert!(client.contains("public record GetContactRequest("));
    assert!(client.contains("path.replace(\"{contact_id}\", pathValue(input.contactId()))"));
    assert!(client.contains("new QueryParameter(\"expand\", input.expand())"));
    assert!(client.contains("headers.put(\"X-Request-ID\", String.valueOf(input.xRequestID()))"));
    assert!(client.contains("return decode(response, Contact.class);"));
    assert!(client.contains("requestWithRetry"));
    assert!(client.contains("retryAllowed"));
    assert!(client.contains("Retry-After"));
    assert!(
        tree.get("sdks/java/src/main/java/com/poolster/email/RetryConfig.java")
            .unwrap()
            .contains("maxAttempts")
    );
    assert!(
        tree.get("sdks/java/src/main/java/com/poolster/email/ClientHooks.java")
            .unwrap()
            .contains("beforeRequest")
    );
}
