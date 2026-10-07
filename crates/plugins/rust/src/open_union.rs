#[cfg(test)]
mod tests {
    use crate::PackageExt;
    use kaji_core::{AdditionalProperties, Api, Field, Schema, SchemaKind, SchemaValue};
    fn api() -> Api {
        Api {
            name: "Future".into(),
            schemas: vec![
                Schema::new(
                    "Event",
                    SchemaValue::new(SchemaKind::OneOf {
                        variants: vec![
                            SchemaValue::new(SchemaKind::String),
                            SchemaValue::new(SchemaKind::Integer),
                        ],
                    }),
                ),
                Schema::new(
                    "Envelope",
                    SchemaValue::new(SchemaKind::Object {
                        fields: vec![Field {
                            name: "event".into(),
                            value: SchemaValue::reference("#/components/schemas/Event"),
                            required: true,
                            annotations: Default::default(),
                        }],
                        additional_properties: AdditionalProperties::Any,
                    }),
                ),
            ],
            ..Default::default()
        }
    }
    #[test]
    fn open_union_setting_applies_to_sdk_and_independent_models() {
        for sdk in [true, false] {
            for enabled in [true, false] {
                let package = crate::package("sdk").open_unions(enabled);
                let packages = kaji_core::engine::Packages::new();
                let tree = if sdk {
                    packages.package(package.with(crate::sdk()))
                } else {
                    packages.package(package.with(crate::models()))
                }
                .generate(&api(), None)
                .unwrap();
                let source = tree
                    .iter()
                    .map(|(_, source)| source)
                    .collect::<Vec<_>>()
                    .join("\n");
                assert_eq!(source.contains("Unknown(serde_json::Value)"), enabled);
            }
        }
    }
    #[test]
    #[ignore = "requires cached generated Cargo dependencies"]
    fn native_unknown_union_roundtrips_and_default_decoding_remains_strict() {
        for enabled in [true, false] {
            let root = tempfile::tempdir().unwrap();
            kaji_core::engine::Packages::new()
                .package(
                    crate::package("sdk")
                        .open_unions(enabled)
                        .with(crate::sdk()),
                )
                .generate(&api(), None)
                .unwrap()
                .write_to(root.path())
                .unwrap();
            let path = root.path().join("sdk/src/lib.rs");
            let assertion = if enabled {
                r#"let envelope: models::Envelope=serde_json::from_value(wire.clone()).unwrap(); assert!(matches!(envelope.event, models::Event::Unknown(_))); assert_eq!(serde_json::to_value(envelope).unwrap(),wire);"#
            } else {
                "assert!(serde_json::from_value::<models::Envelope>(wire).is_err());"
            };
            let source = std::fs::read_to_string(&path).unwrap()
                + &format!(
                    r#"
#[cfg(test)] mod future_probe {{
use crate::models;
#[test] fn future_wire() {{
let wire=serde_json::json!({{"event":{{"future":true,"zero":0,"null":null}},"extra":{{"value":false}}}});
{assertion}
let known:models::Event=serde_json::from_value(serde_json::json!(0)).unwrap();
assert!(matches!(known,models::Event::Variant1(0)));
}}
}}
"#
                );
            std::fs::write(path, source).unwrap();
            let output = crate::native_cargo()
                .args(["test", "--lib"])
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
    }
}
