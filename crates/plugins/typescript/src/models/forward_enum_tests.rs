use super::*;

#[test]
fn unspecified_properties_keep_typed_fields_and_unknown_extension_values() {
    let fields = vec![poolster_core::Field {
        name: "name".into(),
        value: SchemaValue::new(SchemaKind::String),
        required: true,
        annotations: Default::default(),
    }];
    let open = render_object(
        &fields,
        &AdditionalProperties::Unspecified,
        &Default::default(),
    );
    assert!(open.contains("name: string"));
    assert!(open.contains("[key: string]: unknown"));
    assert!(
        !render_object(
            &fields,
            &AdditionalProperties::Forbidden,
            &Default::default()
        )
        .contains("[key:")
    );
    assert!(
        render_object(&[], &AdditionalProperties::Unspecified, &Default::default())
            .contains("[key: string]: unknown")
    );
    assert!(!render_object_fields(&fields, &Default::default()).contains("[key:"));
}

#[test]
fn open_enum_types_preserve_literals_and_future_primitives() {
    let mut value = SchemaValue::new(SchemaKind::String);
    value.enum_values = vec![serde_json::json!("known")];
    for enum_type in [
        EnumType::Literal,
        EnumType::AsConst,
        EnumType::Enum,
        EnumType::ConstEnum,
    ] {
        let options = ModelOptions {
            enum_type,
            open_enums: true,
            ..Default::default()
        };
        let source = render_schema("State", &value, &options, "");
        assert!(source.contains("(string & {})"));
        assert!(source.contains("\"known\"") || source.contains("'known'"));
        assert!(!source.contains("any"));
        assert!(!source.contains("unknown"));
    }
    let strict = render_schema("State", &value, &Default::default(), "");
    assert!(!strict.contains("string &"));
    assert!(
        render_value(
            &value,
            &ModelOptions {
                open_enums: true,
                ..Default::default()
            }
        )
        .contains("string &")
    );
}

#[test]
#[ignore = "requires Node and TypeScript compiler"]
fn native_open_enum_types_accept_future_values_and_preserve_wire() {
    let mut value = SchemaValue::new(SchemaKind::String);
    value.enum_values = vec![serde_json::json!("known")];
    let root = tempfile::tempdir().unwrap();
    for (index, enum_type) in [
        EnumType::Literal,
        EnumType::AsConst,
        EnumType::Enum,
        EnumType::ConstEnum,
    ]
    .into_iter()
    .enumerate()
    {
        let options = ModelOptions {
            enum_type,
            open_enums: true,
            ..Default::default()
        };
        let type_name = if enum_type == EnumType::AsConst {
            "StateKey"
        } else {
            "State"
        };
        let source = render_schema("State", &value, &options, "")
            + &format!(
                "\nconst known: {type_name} = 'known';\nconst future: {type_name} = 'future';\nconst wire = {{state: future, extra: {{nested: [null, false, 0]}}}};\nif (JSON.stringify(JSON.parse(JSON.stringify(wire))) !== JSON.stringify(wire)) throw new Error('wire changed');\n"
            );
        let path = root.path().join(format!("probe{index}.ts"));
        std::fs::write(&path, source).unwrap();
        let compiler = std::env::var("POOLSTER_TSC_JS").expect("set POOLSTER_TSC_JS");
        let output = std::process::Command::new("node")
            .arg(&compiler)
            .args([
                "--strict",
                "--target",
                "es2020",
                "--module",
                "commonjs",
                "--skipLibCheck",
            ])
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            std::process::Command::new("node")
                .arg(path.with_extension("js"))
                .status()
                .unwrap()
                .success()
        );
    }
}
