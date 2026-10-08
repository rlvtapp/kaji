use super::*;

#[test]
#[ignore = "requires PHP8.2+; generated reserved models and resource methods syntax probe"]
fn native_reserved_model_and_resource_names_compile() {
    let mut source = Api {
        name: "Keywords".into(),
        ..Default::default()
    };
    for name in [
        "String",
        "Object",
        "List",
        "Enum",
        "Client",
        "Client-ID",
        "Client_ID",
    ] {
        source.schemas.push(Schema::new(
            name,
            SchemaValue::new(SchemaKind::Object {
                fields: vec![],
                additional_properties: AdditionalProperties::Forbidden,
            }),
        ));
    }
    for name in [
        "case",
        "function",
        "namespace",
        "trait",
        "list",
        "request",
        "forCall",
    ] {
        source.operations.push(Operation {
            id: name.into(),
            method: poolster_core::HttpMethod::Get,
            path: format!("/{name}"),
            parameters: vec![],
            request_body: None,
            responses: vec![],
            security: vec![],
            annotations: Default::default(),
        });
    }
    let tree = render_sdk(
        &source,
        "sdk",
        Some("poolster/keywords"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    for (path, _) in tree
        .iter()
        .filter(|(path, _)| path.extension().is_some_and(|extension| extension == "php"))
    {
        let result = std::process::Command::new("php")
            .arg("-l")
            .arg(root.path().join(path))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}: {}",
            path.display(),
            String::from_utf8_lossy(&result.stdout)
        );
    }
}
#[test]
#[ignore = "requires php toolchain; dependency-free collision and alias wire probe"]
fn native_collision_models_preserve_wire_and_alias_decoding() {
    let schema = Schema::new(
        "Probe",
        SchemaValue::new(SchemaKind::Object {
            fields: ["+1", "-1", "x-axis", "x_axis"]
                .iter()
                .map(|name| poolster_core::Field {
                    name: (*name).into(),
                    value: SchemaValue::new(SchemaKind::String),
                    required: true,
                    annotations: Default::default(),
                })
                .collect(),
            additional_properties: AdditionalProperties::Forbidden,
        }),
    );
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("model.php"),
        render_model(&schema, "Collision", &Default::default()),
    )
    .unwrap();
    let mut choices = SchemaValue::new(SchemaKind::String);
    choices.enum_values = vec![
        serde_json::json!("+1"),
        serde_json::json!("-1"),
        serde_json::json!("+1"),
        serde_json::json!("class"),
    ];
    std::fs::write(
        root.path().join("choices.php"),
        render_enum("Choices", &choices, "Collision"),
    )
    .unwrap();
    let script = r#"<?php
require __DIR__.'/model.php';
require __DIR__.'/choices.php';
foreach(['+1','-1','class'] as $value) if(Collision\Models\Choices::tryFrom($value)?->value!==$value) throw new Exception('enum collision/backing wire');
if(count(Collision\Models\Choices::cases())!==3) throw new Exception('duplicate enum literal');
$wire=['+1'=>'positive','-1'=>'negative','x-axis'=>'dash','x_axis'=>'underscore'];
if(json_decode(json_encode(Collision\Models\Probe::fromArray($wire), JSON_THROW_ON_ERROR), true)!==$wire) throw new Exception('collision wire roundtrip');
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
