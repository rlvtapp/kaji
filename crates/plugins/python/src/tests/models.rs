use super::*;

#[test]
fn open_enum_setting_preserves_known_literals_and_scalar_wire_types() {
    let mut value = SchemaValue::new(SchemaKind::String);
    value.enum_values = vec![serde_json::json!("known")];
    let strict = render_model(&Schema::new("State", value.clone()));
    assert!(strict.contains("State = Literal[\"known\"]"));
    value
        .extensions
        .insert("x-kaji-open-enum".into(), serde_json::json!(true));
    let open = render_model(&Schema::new("State", value));
    assert!(open.contains("State = Literal[\"known\"] | str"));
    assert!(!open.contains("State = Any"));
}

#[test]
fn forward_values_preserve_known_models_unknown_unions_and_optional_nulls() {
    let mut source = api();
    let mut enum_value = SchemaValue::new(SchemaKind::String);
    enum_value.enum_values = vec![serde_json::json!("known")];
    source.schemas = vec![
        Schema::new("State", enum_value),
        Schema::new(
            "Variant",
            SchemaValue::new(SchemaKind::OneOf {
                variants: vec![
                    SchemaValue::reference("#/components/schemas/State"),
                    SchemaValue::new(SchemaKind::Integer),
                ],
            }),
        ),
        Schema::new(
            "FutureModel",
            SchemaValue::new(SchemaKind::Object {
                fields: ["state", "variant"]
                    .iter()
                    .map(|name| Field {
                        name: (*name).into(),
                        value: SchemaValue::reference(if *name == "state" {
                            "#/components/schemas/State"
                        } else {
                            "#/components/schemas/Variant"
                        }),
                        required: false,
                        annotations: Default::default(),
                    })
                    .collect(),
                additional_properties: AdditionalProperties::Any,
            }),
        ),
    ];
    let root = tempfile::tempdir().unwrap();
    render_test_sdk(&source, "sdk/python", Some("example-api-sdk"))
        .unwrap()
        .write_to(root.path())
        .unwrap();
    let script = r#"from example_api_sdk.models import FutureModel, _to_wire
for wire in [{}, {"state": None}, {"state": "future", "variant": {"new": [0, False, None]}, "extra": {"nested": None}}, {"state": "known", "variant": 0}]:
    restored = FutureModel.from_dict(wire)
    assert isinstance(restored, FutureModel)
    assert _to_wire(restored) == wire
"#;
    let output = std::process::Command::new("python3")
        .args(["-c", script])
        .env("PYTHONPATH", root.path().join("sdk/python/src"))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn additional_properties_round_trip_at_original_wire_keys() {
    let mut source = api();
    source.schemas = vec![Schema::new(
        "OpenModel",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![Field {
                name: "display-name".into(),
                value: SchemaValue::new(SchemaKind::String),
                required: true,
                annotations: Default::default(),
            }],
            additional_properties: AdditionalProperties::Any,
        }),
    )];
    let root = tempfile::tempdir().unwrap();
    render_test_sdk(&source, "sdk/python", Some("example-api-sdk"))
        .unwrap()
        .write_to(root.path())
        .unwrap();
    let script = r#"from example_api_sdk.models import OpenModel, _to_wire
wire = {"display-name": "Alice", "future": {"nested": [1, None, "x"]}, "additional_properties": 7}
model = OpenModel.from_dict(wire)
assert model.additional_properties == {"future": {"nested": [1, None, "x"]}, "additional_properties": 7}
assert _to_wire(model) == wire
model.additional_properties["display-name"] = "override"
assert _to_wire(model)["display-name"] == "Alice"
"#;
    let status = Command::new("python3")
        .args(["-c", script])
        .env("PYTHONPATH", root.path().join("sdk/python/src"))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn generates_a_deterministic_typed_python_package() {
    let first = render_test_sdk(&api(), "sdk/python", Some("example-api-sdk")).unwrap();
    let second = render_test_sdk(&api(), "sdk/python", Some("example-api-sdk")).unwrap();
    assert_eq!(first, second);
    assert!(
        first
            .get("sdk/python/pyproject.toml")
            .unwrap()
            .contains("version = \"2026.09.19\"")
    );
    assert!(
        first
            .get("sdk/python/src/example_api_sdk/models/contact_87bae2710b2492c3.py")
            .unwrap()
            .contains("class Contact:")
    );
    assert!(
        first
            .get("sdk/python/src/example_api_sdk/runtime.py")
            .is_some()
    );
    assert!(
        first
            .get("sdk/python/src/example_api_sdk/operations_000.py")
            .unwrap()
            .contains("class Operations000:")
    );
    assert!(
        !first
            .get("sdk/python/src/example_api_sdk/client.py")
            .unwrap()
            .contains("def get_contact")
    );
    let client = rendered_python(&first);
    assert!(client.contains(
        "def get_contact(self, *, contact_id: str, include_deleted: bool | None = None) -> Contact:"
    ));
    assert!(client.contains("path.replace(\"{contactId}\", quote(str(contact_id), safe=\"\"))"));
    assert!(client.contains("Authorization\", f\"Bearer {self.api_key}\""));
    assert!(client.contains("def health(self) -> None:"));
}

#[test]
fn partitions_model_exports_without_changing_model_imports() {
    let mut source = api();
    source.schemas = (0..101)
        .map(|index| {
            Schema::new(
                format!("Model{index}"),
                SchemaValue::new(SchemaKind::String),
            )
        })
        .collect();
    let tree = render_test_sdk(&source, "sdk", Some("example-api-sdk")).unwrap();
    assert!(
        tree.get("sdk/src/example_api_sdk/models/chunks/exports_001.py")
            .unwrap()
            .contains("Model100")
    );
    assert!(
        tree.get("sdk/src/example_api_sdk/models/__init__.py")
            .unwrap()
            .contains("from .chunks.exports_001 import *")
    );

    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let status = Command::new("python3")
            .args([
                "-c",
                "from example_api_sdk import Model100; import example_api_sdk; assert not hasattr(example_api_sdk, '_to_wire')",
            ])
            .env("PYTHONPATH", root.path().join("sdk/src"))
            .env("PYTHONPYCACHEPREFIX", root.path().join("python-cache"))
            .status()
            .unwrap();
    assert!(status.success());
}

#[test]
fn generated_package_is_valid_python() {
    let root = tempfile::tempdir().unwrap();
    render_sdk(
        &api(),
        "sdk/python",
        Some("example-api-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap()
    .write_to(root.path())
    .unwrap();
    let status = Command::new("python3")
        .args(["-m", "compileall", "-q"])
        .env("PYTHONPYCACHEPREFIX", root.path().join("python-cache"))
        .arg(root.path().join("sdk/python/src"))
        .status()
        .unwrap();
    assert!(status.success());
}
