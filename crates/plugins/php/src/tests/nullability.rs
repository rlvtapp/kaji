use super::*;

#[test]
#[ignore = "requires PHP 8.2+; dependency-free model/query wire probe"]
fn native_required_null_and_scalar_query_serialization() {
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
    let model = render_model(&schema, "Example", &Default::default());
    let client = render_client(&api(), "Example", SdkClientStyle::Flat);
    let start = client
        .find("    private function poolsterQueryString(")
        .unwrap();
    let end = client.rfind("\n}\n").unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Model.php"), model).unwrap();
    let script = format!(
        "<?php\nrequire __DIR__.'/Model.php';\nclass Probe {{ {}\n public function encode(array $query):string {{return $this->poolsterQueryString($query);}} }}\n",
        &client[start..end]
    ) + r#"
$model=Example\Models\WireInput::fromArray(['enabled'=>false,'count'=>0,'note'=>null]);
$encoded=json_decode(json_encode($model, JSON_THROW_ON_ERROR),true,flags:JSON_THROW_ON_ERROR);
if($encoded!==['enabled'=>false,'count'=>0,'note'=>null])throw new Exception('model null/presence assertion');
$query=(new Probe())->encode(['text'=>'héllo 雪','flag'=>false,'count'=>0,'tags'=>['a','b'],'missing'=>null]);
if($query!=='text=h%C3%A9llo%20%E9%9B%AA&flag=false&count=0&tags=a&tags=b')throw new Exception('scalar query assertion');
"#;
    std::fs::write(root.path().join("probe.php"), script).unwrap();
    let result = std::process::Command::new("php")
        .arg(root.path().join("probe.php"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
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
    let model = render_model(&schema, "Example", &Default::default());
    assert!(!model.contains("$source"));
    assert!(model.contains("note: $data['note'] === null ? null"));
    assert!(model.contains("$value['note'] = $this->note"));
    let client = render_client(&api(), "Example", SdkClientStyle::Flat);
    assert!(client.contains("$scalar = is_bool($item) ? ($item ? 'true' : 'false')"));
    assert!(client.contains("foreach (is_array($value) ? $value : [$value] as $item)"));
}

#[test]
fn enum_references_keep_known_cases_and_allow_future_wire_values() {
    let mut api = api();
    let mut value = SchemaValue::new(SchemaKind::String);
    value.enum_values = vec![serde_json::json!("known")];
    api.schemas.push(Schema::new("State", value));
    let types = NamedTypes::from_api(&api);
    let mut reference = SchemaValue::new(SchemaKind::Reference {
        reference: "#/components/schemas/State".into(),
    });
    assert_eq!(php_type(&reference, &types), "State|string");
    reference.nullable = true;
    assert_eq!(php_type(&reference, &types), "State|string|null");
    assert_eq!(
        from_value("$value", &reference, &types),
        "($value === null ? null : (State::tryFrom($value) ?? $value))"
    );
}

#[test]
#[ignore = "requires PHP 8.2 or newer; native forward enum and model roundtrip"]
fn native_future_enums_preserve_known_types_and_unknown_wire_values() {
    let mut api = api();
    let mut value = SchemaValue::new(SchemaKind::String);
    value.enum_values = vec![serde_json::json!("known")];
    api.schemas.push(Schema::new("State", value));
    api.schemas.push(Schema::new(
        "Future",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![poolster_core::Field {
                name: "state".into(),
                value: SchemaValue::new(SchemaKind::Reference {
                    reference: "#/components/schemas/State".into(),
                }),
                required: false,
                annotations: Default::default(),
            }],
            additional_properties: AdditionalProperties::Any,
        }),
    ));
    let types = NamedTypes::from_api(&api);
    let root = tempfile::tempdir().unwrap();
    for schema in api
        .schemas
        .iter()
        .filter(|schema| ["State", "Future"].contains(&schema.name.as_str()))
    {
        std::fs::write(
            root.path().join(format!("{}.php", schema.name)),
            render_model(schema, "Probe", &types),
        )
        .unwrap();
    }
    let script = r#"<?php
require __DIR__ . '/State.php';
require __DIR__ . '/Future.php';
$original = new \Probe\Models\Future();
$manual = $original->withPresentFields(['state']);
if (json_encode($manual) !== '{"state":null}' || json_encode($original) !== '{}') throw new Exception('explicit null helper changed original');
try { $original->withPresentFields(['missing']); throw new Exception('unknown field accepted'); } catch (InvalidArgumentException $e) {}

foreach ([[], ['state' => null], ['state' => 'future', 'extra' => ['null' => null, 'zero' => 0, 'false' => false]], ['state' => 'known']] as $wire) {
$model = \Probe\Models\Future::fromArray($wire);
if (json_decode(json_encode($model)) != json_decode(json_encode((object) $wire))) { throw new Exception('roundtrip changed'); }
if (($wire['state'] ?? null) === 'known' && !($model->state instanceof \Probe\Models\State)) { throw new Exception('known enum lost'); }
}
"#;
    let path = root.path().join("probe.php");
    std::fs::write(&path, script).unwrap();
    let output = std::process::Command::new("php")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn open_models_preserve_unknown_properties_at_original_keys() {
    let schema = Schema::new(
        "Future",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![poolster_core::Field {
                name: "additionalProperties".into(),
                value: SchemaValue::new(SchemaKind::String),
                required: false,
                annotations: Default::default(),
            }],
            additional_properties: AdditionalProperties::Any,
        }),
    );
    let model = render_model(&schema, "Example", &Default::default());
    assert!(model.contains("public readonly array $additionalProperties_"));
    assert!(
        model.contains("array_diff_key($data, array_fill_keys(['additionalProperties'], true))")
    );
    assert!(model.contains("$instance->poolsterPresentFields = array_keys($data)"));
    assert!(model.contains("return (object) $value;"));
    assert!(
        model
            .contains("in_array('additionalProperties', $this->poolsterPresentFields ?? [], true)")
    );
}
