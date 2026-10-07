use super::*;
pub(crate) fn selected(operation: &Operation) -> bool {
    operation.request_body.as_ref().is_some_and(|body| {
        body.media_types.iter().any(|media| {
            media
                .content_type
                .to_ascii_lowercase()
                .starts_with("multipart/")
        })
    })
}
fn fields<'a>(api: &'a Api, operation: &Operation) -> Result<&'a [kaji_core::Field]> {
    let body = operation.request_body.as_ref().unwrap();
    anyhow::ensure!(
        body.media_types.len() == 1
            && body.media_types[0]
                .content_type
                .eq_ignore_ascii_case("multipart/form-data"),
        "multipart operation '{}' requires exactly one multipart/form-data media type",
        operation.id
    );
    let schema = body.media_types[0].schema.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "multipart operation '{}' requires a named object schema",
            operation.id
        )
    })?;
    anyhow::ensure!(
        !schema.nullable && !schema.nullish,
        "multipart operation '{}' cannot use a nullable root",
        operation.id
    );
    let SchemaKind::Reference { reference } = &schema.kind else {
        anyhow::bail!(
            "multipart operation '{}' requires a named object reference",
            operation.id
        )
    };
    let name = reference.rsplit('/').next().unwrap_or(reference);
    let schema = api.schemas.iter().find(|s| s.name == name).ok_or_else(|| {
        anyhow::anyhow!(
            "multipart operation '{}' references an unknown schema",
            operation.id
        )
    })?;
    anyhow::ensure!(
        !schema.value.nullable && !schema.value.nullish,
        "multipart operation '{}' resolves to a nullable object",
        operation.id
    );
    let SchemaKind::Object {
        fields,
        additional_properties,
    } = &schema.value.kind
    else {
        anyhow::bail!(
            "multipart operation '{}' requires a direct object schema",
            operation.id
        )
    };
    anyhow::ensure!(
        matches!(additional_properties, AdditionalProperties::Forbidden),
        "multipart operation '{}' requires a closed object; extra parts need an adapter",
        operation.id
    );
    for field in fields {
        anyhow::ensure!(
            !field.value.nullable
                && !field.value.nullish
                && matches!(
                    field.value.kind,
                    SchemaKind::String
                        | SchemaKind::Boolean
                        | SchemaKind::Integer
                        | SchemaKind::Number
                ),
            "multipart operation '{}' field '{}' requires a direct nonnullable scalar or binary string; arrays, references and unions need an adapter",
            operation.id,
            field.name
        );
        anyhow::ensure!(
            !field.name.is_empty()
                && field
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)),
            "multipart operation '{}' has an unsupported part name",
            operation.id
        );
        anyhow::ensure!(
            field.value.format.as_deref() != Some("byte"),
            "multipart operation '{}' field '{}' uses ambiguous base64 byte format; use binary for raw file bytes",
            operation.id,
            field.name
        );
    }
    Ok(fields)
}
pub(crate) fn validate(api: &Api) -> Result<()> {
    for op in api.operations.iter().filter(|op| selected(op)) {
        fields(api, op)?;
    }
    Ok(())
}
fn binary(field: &kaji_core::Field) -> bool {
    field.value.format.as_deref() == Some("binary")
        && matches!(field.value.kind, SchemaKind::String)
}
pub(crate) fn emit(api: &Api, root: &str, package: &str, tree: &mut GeneratedTree) -> Result<()> {
    if !api.operations.iter().any(selected) {
        return Ok(());
    }
    let path = package.replace('.', "/");
    let prefix = if root.is_empty() || root == "." {
        String::new()
    } else {
        format!("{root}/")
    };
    anyhow::ensure!(
        !api.schemas
            .iter()
            .any(|s| type_name(&s.name) == "MultipartBody"),
        "MultipartBody model name is reserved by multipart encoding"
    );
    tree.insert(GeneratedFile::new(
        format!("{prefix}src/main/java/{path}/MultipartBody.java"),
        include_str!("multipart.java.txt").replace("__PACKAGE__", package),
    )?)?;
    for op in api.operations.iter().filter(|op| selected(op)) {
        let name = format!("{}MultipartBody", type_name(&op.id));
        anyhow::ensure!(
            !api.schemas.iter().any(|s| type_name(&s.name) == name),
            "multipart body name collides with a schema"
        );
        let fields = fields(api, op)?;
        let mut components = Vec::new();
        let mut encode = String::new();
        let mut checks = String::new();
        for field in fields {
            let property = field_name(&field.name);
            anyhow::ensure!(
                !matches!(
                    property.as_str(),
                    "encode"
                        | "clone"
                        | "finalize"
                        | "getClass"
                        | "hashCode"
                        | "notify"
                        | "notifyAll"
                        | "toString"
                        | "wait"
                ),
                "multipart field collides with encode method"
            );
            let ty = if binary(field) {
                "MultipartBody.FilePart".into()
            } else {
                java_type(&field.value)
            };
            components.push(format!("{ty} {property}"));
            if field.required {
                checks.push_str(&format!(
                    "java.util.Objects.requireNonNull({property}, \"required multipart field\");\n"
                ));
            }
            if matches!(field.value.kind, SchemaKind::Number) {
                checks.push_str(&format!("if({property}!=null&&!Double.isFinite({property}))throw new IllegalArgumentException(\"nonfinite multipart number\");\n"));
            }
            let part = if binary(field) {
                format!("new MultipartBody.Part({:?},null,{property})", field.name)
            } else {
                format!(
                    "new MultipartBody.Part({:?},String.valueOf({property}),null)",
                    field.name
                )
            };
            encode.push_str(&format!("if({property}!=null)parts.add({part});\n"));
        }
        let source = format!(
            "package {package};\npublic record {name}({}) implements MultipartBody {{\npublic {name} {{ {checks} }}\n@Override public MultipartBody.Encoded encode(){{var parts=new java.util.ArrayList<MultipartBody.Part>();{encode}return MultipartBody.encodeParts(parts);}}\n}}\n",
            components.join(",")
        );
        tree.insert(GeneratedFile::new(
            format!("{prefix}src/main/java/{path}/{name}.java"),
            source,
        )?)?;
    }
    tree.insert(GeneratedFile::new(format!("{prefix}MULTIPART.md"), "Multipart/form-data operations accept a generated <Operation>MultipartBody record through their native request record. File fields use MultipartBody.FilePart(filename, contentType, bytes), or MultipartBody.FilePart.bytes(bytes). File bytes are copied; scalars use UTF-8. Only one multipart/form-data media type and a named closed object with direct nonnullable scalar/binary fields are supported. Optional parts with null values are omitted. Arrays, nested/reference fields, unions, extra parts, base64 byte format and custom per-part encoding require adapters. Part names use ASCII letters/digits/dot/underscore/hyphen; filenames use printable ASCII excluding quotes/backslashes. Files are buffered; no streaming/file-system access is implied. Java JSON model APIs remain separate.")?)?;
    Ok(())
}
pub(crate) fn runtime(source: String) -> String {
    source.replace("} else {\n                builder.header(\"Content-Type\", \"application/json\");", "} else if (body instanceof MultipartBody multipart) {\n                var encoded=multipart.encode(); builder.setHeader(\"Content-Type\",encoded.contentType()); builder.method(method,HttpRequest.BodyPublishers.ofByteArray(encoded.bytes()));\n            } else {\n                builder.header(\"Content-Type\", \"application/json\");")
 .replace("else if (body instanceof byte[] bytes)", "else if (body instanceof MultipartBody multipart) { var encoded=multipart.encode();builder.setHeader(\"Content-Type\",encoded.contentType());builder.method(method,HttpRequest.BodyPublishers.ofByteArray(encoded.bytes())); }\n                else if (body instanceof byte[] bytes)")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn api() -> Api {
        let mut file = SchemaValue::new(SchemaKind::String);
        file.format = Some("binary".into());
        let field = |name: &str, value, required| kaji_core::Field {
            name: name.into(),
            value,
            required,
            annotations: Default::default(),
        };
        Api {
            name: "Multipart".into(),
            schemas: vec![Schema::new(
                "Upload",
                SchemaValue::new(SchemaKind::Object {
                    fields: vec![
                        field("title", SchemaValue::new(SchemaKind::String), true),
                        field("flag", SchemaValue::new(SchemaKind::Boolean), true),
                        field("count", SchemaValue::new(SchemaKind::Integer), true),
                        field("file", file, true),
                        field("missing", SchemaValue::new(SchemaKind::String), false),
                    ],
                    additional_properties: AdditionalProperties::Forbidden,
                }),
            )],
            operations: vec![Operation {
                id: "uploadThing".into(),
                method: kaji_core::HttpMethod::Post,
                path: "/upload".into(),
                request_body: Some(kaji_core::OperationRequestBody {
                    required: true,
                    description: None,
                    media_types: vec![kaji_core::OperationMediaType {
                        content_type: "multipart/form-data".into(),
                        schema: Some(SchemaValue::reference("#/components/schemas/Upload")),
                    }],
                }),
                responses: vec![kaji_core::OperationResponse {
                    status: "204".into(),
                    description: None,
                    media_types: vec![],
                }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }
    #[test]
    fn rejects_unsupported_shapes_before_emission() {
        let mut source = api();
        let SchemaKind::Object { fields, .. } = &mut source.schemas[0].value.kind else {
            unreachable!()
        };
        fields[0].value = SchemaValue::new(SchemaKind::Array {
            items: Box::new(SchemaValue::new(SchemaKind::String)),
        });
        assert!(
            validate(&source)
                .unwrap_err()
                .to_string()
                .contains("arrays")
        );
        let mut source = api();
        source.operations[0]
            .request_body
            .as_mut()
            .unwrap()
            .media_types
            .push(kaji_core::OperationMediaType {
                content_type: "application/json".into(),
                schema: Some(SchemaValue::new(SchemaKind::String)),
            });
        assert!(
            validate(&source)
                .unwrap_err()
                .to_string()
                .contains("exactly one")
        );
    }
    #[test]
    fn emits_native_typed_multipart_in_all_response_drivers() {
        let tree = kaji_core::engine::Packages::new()
            .package(
                crate::package("sdk")
                    .name("io.kaji.multipart")
                    .with(crate::sdk()),
            )
            .generate(&api(), None)
            .unwrap();
        let client = tree
            .get("sdk/src/main/java/io/kaji/multipart/ClientBase.java")
            .unwrap();
        assert_eq!(
            client
                .matches("body instanceof MultipartBody multipart")
                .count(),
            4
        );
        let dto = tree
            .get("sdk/src/main/java/io/kaji/multipart/UploadThingMultipartBody.java")
            .unwrap();
        assert!(dto.contains("MultipartBody.FilePart file"));
        assert!(dto.contains("String.valueOf(flag)"));
        let operation = tree
            .iter()
            .find(|(_, source)| source.contains("public record UploadThingRequest"))
            .unwrap()
            .1;
        assert!(operation.contains("UploadThingMultipartBody body"));
    }
    #[test]
    #[ignore = "requires JDK17+Maven; parses native HTTP multipart request bytes without network"]
    fn native_multipart_http_bytes_preserve_unicode_falsy_and_binary() {
        let tree = kaji_core::engine::Packages::new()
            .package(
                crate::package("sdk")
                    .name("io.kaji.multipart")
                    .with(crate::sdk())
                    .with(crate::operation_tests()),
            )
            .generate(&api(), None)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        let mut source = include_str!("operation_driver.java.txt")
            .replace("__PACKAGE__", "io.kaji.multipart")
            .replace("__CASES__", include_str!("multipart_probe_main.java.txt"));
        let start = source.find("        void assertRequest(").unwrap();
        let end = source[start..]
            .find("        public <T> HttpResponse<T> send(")
            .unwrap()
            + start;
        source.replace_range(start..end, include_str!("multipart_probe_assert.java.txt"));
        std::fs::write(
            dir.path()
                .join("sdk/src/test/java/io/kaji/multipart/KajiOperationTests.java"),
            source,
        )
        .unwrap();
        let output = std::process::Command::new("mvn")
            .args([
                "-q",
                "test-compile",
                "org.codehaus.mojo:exec-maven-plugin:3.5.0:java",
                "-Dexec.mainClass=io.kaji.multipart.KajiOperationTests",
                "-Dexec.classpathScope=test",
            ])
            .current_dir(dir.path().join("sdk"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
