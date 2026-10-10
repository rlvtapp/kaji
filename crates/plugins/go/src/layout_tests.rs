use super::*;
use poolster_core::{HttpMethod, OperationMediaType, OperationResponse};
use std::process::Command;

#[test]
fn import_detection_ignores_documentation_literals_and_struct_tags() {
    assert_eq!(
        selectors(
            "// fmt.String\n/* json.RawMessage */\n`json:\"io.ReadCloser\"`\n\"strings.Replace\"\nhttp.Header{}\ncontext . Context"
        ),
        BTreeSet::from(["http", "context"])
    );
}

#[test]
fn split_response_registry_retains_shapes_across_chunk_boundaries() {
    let api = Api {
        schemas: (0..201)
            .map(|index| {
                Schema::new(
                    format!("Model{index}"),
                    SchemaValue::new(SchemaKind::String),
                )
            })
            .collect(),
        ..Api::default()
    };
    let root = tempfile::tempdir().unwrap();
    render_sdk(&api, "sdk", Some("probe"), SdkClientStyle::Flat, 0)
        .unwrap()
        .write_to(root.path())
        .unwrap();
    std::fs::write(root.path().join("sdk/registry_test.go"), r#"package probe
import ("reflect"; "testing")
func TestAllDescriptorChunks(t *testing.T) {
 if len(poolsterResponseShapes) != 201 { t.Fatalf("missing descriptors: %d",len(poolsterResponseShapes)) }
 for _, name := range []string{"Model0","Model100","Model200"} { if poolsterResponseShapes[name].Kind != "string" { t.Fatalf("missing %s",name) } }
 if err := poolsterValidateResponse("ok",reflect.TypeOf(Model200("")),"$",0); err != nil {t.Fatal(err)}
 if err := poolsterValidateResponse(true,reflect.TypeOf(Model200("")),"$",0); err == nil {t.Fatal("validation lost across chunk boundary")}
}
"#).unwrap();
    assert!(
        Command::new("go")
            .args(["test", "./..."])
            .current_dir(root.path().join("sdk"))
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn output_names_are_bounded_and_do_not_collide_after_normalization() {
    assert_ne!(filename("model", "foo-bar"), filename("model", "foo_bar"));
    let name = "LongGraphResource".repeat(100);
    assert!(filename("operation", &name).len() < 160);
}

#[test]
fn split_sdk_compiles_with_recursive_models_and_all_response_kinds() {
    let mut api = Api {
        name: "Graph sample".into(),
        version: "1.0.0".into(),
        ..Api::default()
    };
    api.schemas.push(Schema::new(
        "Node",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![poolster_core::Field {
                name: "parent".into(),
                value: SchemaValue::reference("#/components/schemas/Node"),
                required: false,
                annotations: BTreeMap::new(),
            }],
            additional_properties: AdditionalProperties::Forbidden,
        }),
    ));
    let mut enum_schema = SchemaValue::new(SchemaKind::String);
    enum_schema.enum_values = vec![
        serde_json::json!("-INF"),
        serde_json::json!("INF"),
        serde_json::json!("INF2"),
    ];
    api.schemas.push(Schema::new("NumberLiteral", enum_schema));
    for (id, media, schema) in [
        (
            "getNode",
            "application/json",
            SchemaValue::reference("#/components/schemas/Node"),
        ),
        (
            "getText",
            "text/plain",
            SchemaValue::new(SchemaKind::String),
        ),
        (
            "getBinary",
            "application/octet-stream",
            SchemaValue::new(SchemaKind::String),
        ),
        (
            "getEvents",
            "text/event-stream",
            SchemaValue::new(SchemaKind::String),
        ),
    ] {
        api.operations.push(Operation {
            id: id.into(),
            method: HttpMethod::Get,
            path: format!("/nodes/{id}"),
            responses: vec![OperationResponse {
                status: "200".into(),
                description: None,
                media_types: vec![OperationMediaType {
                    content_type: media.into(),
                    schema: Some(schema),
                }],
            }],
            ..Operation::default()
        });
    }
    // Force several bounded service files, even when all operations share one tag.
    for index in 0..120 {
        api.operations.push(Operation {
            id: format!("listNodes{index}"),
            method: HttpMethod::Get,
            path: format!("/nodes/{index}/json.literal"),
            ..Operation::default()
        });
    }
    api.operations[0].parameters = vec![
        OperationParameter {
            name: "user-id".into(),
            location: "path".into(),
            required: true,
            schema: Some(SchemaValue::new(SchemaKind::String)),
            description: None,
            annotations: BTreeMap::new(),
        },
        OperationParameter {
            name: "userId".into(),
            location: "query".into(),
            required: false,
            schema: Some(SchemaValue::new(SchemaKind::String)),
            description: None,
            annotations: BTreeMap::new(),
        },
    ];
    api.operations[0].path = "/nodes/{user-id}".into();
    let tree = render_sdk(&api, "sdk", Some("graph"), SdkClientStyle::Namespaced, 0).unwrap();
    let serial = render_sdk(&api, "sdk", Some("graph"), SdkClientStyle::Namespaced, 1).unwrap();
    let parallel = render_sdk(&api, "sdk", Some("graph"), SdkClientStyle::Namespaced, 4).unwrap();
    assert_eq!(serial, parallel);
    assert_eq!(tree, parallel);
    let operation = tree
        .get(format!("sdk/{}", filename("operation", "getNode")))
        .unwrap();
    assert!(operation.contains("PathUserID string"));
    assert!(operation.contains("QueryUserID *string"));
    assert!(operation.contains("fmt.Sprint(input.PathUserID)"));
    assert!(operation.contains("addQuery(query, \"userId\", input.QueryUserID)"));
    assert!(tree.get("sdk/models.go").is_none());
    assert!(
        !tree
            .get("sdk/client.go")
            .unwrap()
            .contains("func (client *Client) GetNode(")
    );
    assert_eq!(
        tree.iter()
            .filter(|(p, _)| p
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("operation_"))
            .count(),
        124
    );
    assert_eq!(
        tree.iter()
            .filter(|(p, _)| p
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("service_"))
            .count(),
        3
    );
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let output = Command::new("go")
        .args(["test", "./..."])
        .current_dir(root.path().join("sdk"))
        .env("GOCACHE", root.path().join("go-cache"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
