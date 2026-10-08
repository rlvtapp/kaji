use super::*;

#[test]
fn page_number_pagers_execute_normalized_plan_with_native_encoding() {
    let mut api = contact_api();
    let group = SchemaValue::new(SchemaKind::Object {
        fields: vec![Field {
            name: "a/b~c".into(),
            value: SchemaValue::new(SchemaKind::Array {
                items: Box::new(string_schema()),
            }),
            required: true,
            annotations: BTreeMap::new(),
        }],
        additional_properties: AdditionalProperties::Forbidden,
    });
    api.schemas.push(Schema::new(
        "Page",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![Field {
                name: "groups".into(),
                value: SchemaValue::new(SchemaKind::Array {
                    items: Box::new(group),
                }),
                required: true,
                annotations: BTreeMap::new(),
            }],
            additional_properties: AdditionalProperties::Forbidden,
        }),
    ));
    let operation = &mut api.operations[0];
    operation.id = "listContacts".into();
    operation.path = "/contacts".into();
    operation.request_body = None;
    operation.parameters = [("page", "query"), ("limit", "header")]
        .into_iter()
        .map(|(name, location)| OperationParameter {
            name: name.into(),
            location: location.into(),
            required: false,
            schema: Some(SchemaValue::new(SchemaKind::Integer)),
            description: None,
            annotations: BTreeMap::new(),
        })
        .collect();
    operation.responses[0].media_types[0].schema =
        Some(SchemaValue::reference("#/components/schemas/Page"));
    operation.annotations.insert("x-kaji-pagination".into(),serde_json::json!({"type":"page","inputs":[{"name":"page","type":"page","in":"parameters"},{"name":"limit","type":"limit","in":"parameters"}],"outputs":{"results":"/groups/0/a~1b~0c"}}));
    let mut required = operation.clone();
    required.id = "listRequired".into();
    required
        .parameters
        .iter_mut()
        .for_each(|parameter| parameter.required = true);
    api.operations.push(required);
    let root = tempfile::tempdir().unwrap();
    render_test_sdk(&api, "sdk", Some("email"))
        .unwrap()
        .write_to(root.path())
        .unwrap();
    fs::write(
        root.path().join("sdk/page_test.go"),
        include_str!("../../tests/fixtures/pagination_test.go"),
    )
    .unwrap();
    assert!(
        Command::new("go")
            .args(["test", "-race", "./..."])
            .current_dir(root.path().join("sdk"))
            .env("GOCACHE", root.path().join("go-cache"))
            .status()
            .unwrap()
            .success()
    );
}
#[test]
fn declared_additional_properties_does_not_collide_with_extension_bag() {
    let fields = ["additionalProperties", "additionalProperties2"]
        .into_iter()
        .map(|name| Field {
            name: name.into(),
            value: SchemaValue::new(SchemaKind::String),
            required: false,
            annotations: Default::default(),
        })
        .collect();
    let schema = Schema::new(
        "Claim",
        SchemaValue::new(SchemaKind::Object {
            fields,
            additional_properties: AdditionalProperties::Any,
        }),
    );
    let mut source = "package probe\nimport \"encoding/json\"\n".to_owned();
    render_schema(&mut source, &schema);
    assert!(source.contains("AdditionalProperties3 map[string]json.RawMessage"));
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("go.mod"), "module probe\ngo 1.22\n").unwrap();
    fs::write(root.path().join("model.go"), source).unwrap();
    fs::write(root.path().join("model_test.go"), r#"package probe
import ("encoding/json"; "testing")
func TestCollisionRoundTrip(t *testing.T) {
 var claim Claim
 if err:=json.Unmarshal([]byte(`{"additionalProperties":"declared","additionalProperties2":"second","future":{"value":1}}`),&claim);err!=nil {t.Fatal(err)}
 if claim.AdditionalProperties==nil || *claim.AdditionalProperties!="declared" || claim.AdditionalProperties2==nil || *claim.AdditionalProperties2!="second" || string(claim.AdditionalProperties3["future"])!=`{"value":1}` {t.Fatalf("lost fields: %+v",claim)}
 claim.AdditionalProperties3["additionalProperties"]=json.RawMessage(`"shadow"`)
 raw,err:=json.Marshal(claim);if err!=nil {t.Fatal(err)}
 var wire map[string]json.RawMessage;if err=json.Unmarshal(raw,&wire);err!=nil {t.Fatal(err)}
 if string(wire["additionalProperties"])!=`"declared"` || string(wire["future"])!=`{"value":1}` {t.Fatal(string(raw))}
}
"#).unwrap();
    let output = Command::new("go")
        .args(["test", "./..."])
        .current_dir(root.path())
        .env("GOCACHE", root.path().join("go-cache"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn native_future_models_preserve_enum_union_null_absence_and_unknown_fields() {
    let mut state = string_schema();
    state.enum_values = vec![serde_json::json!("known")];
    let mut note = SchemaValue::reference("#/components/schemas/State");
    note.nullable = true;
    let api = Api {
        name: "Compatibility".into(),
        schemas: vec![
            Schema::new("State", state),
            Schema::new(
                "Choice",
                SchemaValue::new(SchemaKind::OneOf {
                    variants: vec![string_schema(), SchemaValue::new(SchemaKind::Integer)],
                }),
            ),
            Schema::new(
                "Envelope",
                SchemaValue::new(SchemaKind::Object {
                    fields: vec![
                        Field {
                            name: "state".into(),
                            value: note,
                            required: false,
                            annotations: Default::default(),
                        },
                        Field {
                            name: "choice".into(),
                            value: SchemaValue::reference("#/components/schemas/Choice"),
                            required: false,
                            annotations: Default::default(),
                        },
                    ],
                    additional_properties: AdditionalProperties::Any,
                }),
            ),
        ],
        ..Default::default()
    };
    let root = tempfile::tempdir().unwrap();
    render_test_sdk(&api, "sdk", Some("compatibility"))
        .unwrap()
        .write_to(root.path())
        .unwrap();
    fs::write(root.path().join("sdk/compatibility_test.go"), r#"package compatibility
import("encoding/json";"reflect";"testing")
func TestFutureModels(t *testing.T) {
 for _,wire:=range []string{`{}`,`{"state":null}`,`{"state":"future","choice":{"future":[false,0,null]},"extra":{"nested":"future"}}`} {
  var model Envelope;if err:=json.Unmarshal([]byte(wire),&model);err!=nil{t.Fatal(err)}
  raw,err:=json.Marshal(model);if err!=nil{t.Fatal(err)}
  var before,after any;json.Unmarshal([]byte(wire),&before);json.Unmarshal(raw,&after)
  if !reflect.DeepEqual(before,after){t.Fatalf("roundtrip %s -> %s",wire,raw)}
 }
 var empty Envelope; raw,_:=json.Marshal(empty); if string(raw)!=`{}`{t.Fatal(string(raw))}
}
"#).unwrap();
    let output = Command::new("go")
        .args(["test", "-race", "./..."])
        .current_dir(root.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn response_validation_and_attempt_middleware_execute_natively() {
    let mut api = contact_api();
    let SchemaKind::Object {
        fields,
        additional_properties,
    } = &mut api.schemas[0].value.kind
    else {
        unreachable!()
    };
    *additional_properties = AdditionalProperties::Any;
    fields[0].value.enum_values = vec![serde_json::json!("known")];
    fields.push(Field {
        name: "children".into(),
        value: SchemaValue::new(SchemaKind::Array {
            items: Box::new(SchemaValue::reference("#/components/schemas/Contact")),
        }),
        required: false,
        annotations: BTreeMap::new(),
    });
    let mut secret = string_schema();
    secret.write_only = true;
    fields.push(Field {
        name: "secret".into(),
        value: secret.clone(),
        required: true,
        annotations: BTreeMap::new(),
    });
    fields.push(Field {
        name: "referenced_secret".into(),
        value: SchemaValue::reference("#/components/schemas/Secret"),
        required: true,
        annotations: BTreeMap::new(),
    });
    fields.push(Field {
        name: "labels".into(),
        value: SchemaValue::new(SchemaKind::Object {
            fields: vec![],
            additional_properties: AdditionalProperties::Schema {
                value: Box::new(SchemaValue::new(SchemaKind::Integer)),
            },
        }),
        required: false,
        annotations: BTreeMap::new(),
    });
    api.schemas.push(Schema::new("Secret", secret));
    let root = tempfile::tempdir().unwrap();
    render_test_sdk(&api, "sdk", Some("email"))
        .unwrap()
        .write_to(root.path())
        .unwrap();
    fs::write(
        root.path().join("sdk/validation_test.go"),
        include_str!("../../tests/fixtures/response_validation_test.go"),
    )
    .unwrap();
    assert!(
        Command::new("go")
            .args(["test", "-race", "./..."])
            .current_dir(root.path().join("sdk"))
            .env("GOCACHE", root.path().join("go-cache"))
            .status()
            .unwrap()
            .success()
    );
}
