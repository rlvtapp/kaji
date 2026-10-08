//! Opt-in native forward-compatible model surfaces.
use serde_json::Value;
use std::{collections::BTreeSet, fmt::Write};

pub(crate) fn render_open_enum(output: &mut String, name: &str, values: &[Value]) {
    let _ = writeln!(
        output,
        "/// Extensible string enum. Unknown response values retain their original wire string.\n#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]\n#[serde(transparent)]\npub struct {name}(pub String);\nimpl {name} {{\n    pub fn as_str(&self) -> &str {{ &self.0 }}\n    pub fn is_known(&self) -> bool {{ matches!(self.as_str(),"
    );
    let literals = values
        .iter()
        .map(|v| format!("{:?}", v.as_str().unwrap()))
        .collect::<Vec<_>>()
        .join(" | ");
    let _ = writeln!(output, "        {literals}) }}");
    let mut used = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        let raw = value.as_str().unwrap();
        let base = super::render::enum_helper_name(raw);
        let mut helper = base.clone();
        let mut suffix = 2;
        while !used.insert(helper.clone()) {
            helper = format!("{base}_{suffix}");
            suffix += 1;
        }
        if helper.is_empty() {
            helper = format!("value_{index}");
        }
        let _ = writeln!(
            output,
            "    pub fn {helper}() -> Self {{ Self({raw:?}.to_owned()) }}"
        );
    }
    let _ = writeln!(
        output,
        "}}\nimpl From<String> for {name} {{ fn from(value: String) -> Self {{ Self(value) }} }}\nimpl From<&str> for {name} {{ fn from(value: &str) -> Self {{ Self(value.to_owned()) }} }}\nimpl std::fmt::Display for {name} {{ fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{ self.0.fmt(f) }} }}"
    );
}

#[cfg(test)]
mod tests {
    use crate::PackageExt;
    use poolster_core::{AdditionalProperties, Api, Field, Schema, SchemaKind, SchemaValue};
    fn api() -> Api {
        let mut state = SchemaValue::new(SchemaKind::String);
        state.enum_values = vec![
            serde_json::json!("known-value"),
            serde_json::json!("as-str"),
            serde_json::json!("known_value"),
        ];
        let mut optional = SchemaValue::reference("#/components/schemas/State");
        optional.nullable = true;
        Api {
            name: "Compatibility".into(),
            schemas: vec![
                Schema::new("State", state),
                Schema::new(
                    "Envelope",
                    SchemaValue::new(SchemaKind::Object {
                        fields: vec![Field {
                            name: "state".into(),
                            value: optional,
                            required: false,
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
    fn typed_open_enums_are_explicit_and_compose_with_independent_models() {
        for sdk in [true, false] {
            for enabled in [true, false] {
                let package = crate::package("sdk").open_enums(enabled);
                let packages = poolster_core::engine::Packages::new();
                let tree = if sdk {
                    packages.package(package.with(crate::sdk()))
                } else {
                    packages.package(package.with(crate::models()))
                }
                .generate(&api(), None)
                .unwrap();
                let source = tree.iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n");
                assert_eq!(source.contains("pub struct State(pub String)"), enabled);
                if enabled {
                    assert!(source.contains("pub fn known_value_2()"));
                    assert!(source.contains("pub fn as_str_value()"));
                }
            }
        }
    }
    #[test]
    fn extension_bag_cannot_shadow_declared_model_field() {
        let mut api = api();
        let SchemaKind::Object { fields, .. } = &mut api.schemas[1].value.kind else {
            unreachable!()
        };
        fields[0].name = "additional_properties".into();
        let tree = poolster_core::engine::Packages::new()
            .package(crate::package("sdk").with(crate::sdk()))
            .generate(&api, None)
            .unwrap();
        let source = tree.iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n");
        assert!(source.contains("pub additional_properties_: std::collections::BTreeMap"));
        assert!(source.contains("pub additional_properties: Option<Option<"));
    }
    #[test]
    #[ignore = "requires cached generated Cargo dependencies"]
    fn native_nested_future_enum_absent_null_and_unknown_fields_roundtrip() {
        let root = tempfile::tempdir().unwrap();
        poolster_core::engine::Packages::new()
            .package(crate::package("sdk").open_enums(true).with(crate::sdk()))
            .generate(&api(), None)
            .unwrap()
            .write_to(root.path())
            .unwrap();
        let client_path = root.path().join("sdk/src/client/mod.rs");
        let client_source = std::fs::read_to_string(&client_path).unwrap()
            + r#"
#[cfg(test)] mod sequence_probe {
 use super::*;
 #[test] fn framing() {
  let lines:Vec<serde_json::Value>=poolster_decode_json(b"false\n0\nnull\n{\"future\":true}\r\n", "application/x-ndjson").unwrap();
  assert_eq!(lines,serde_json::json!([false,0,null,{"future":true}]).as_array().unwrap().clone());
  let seq:Vec<serde_json::Value>=poolster_decode_json(b"\x1e{\n\"future\":true\n}\n\x1e0\n", "application/json-seq").unwrap();
  assert_eq!(seq.len(),2);
  for malformed in [b"missing separator".as_slice(), b"\x1e\x1e0", b"\x1e0 trailing"] { assert!(poolster_decode_json::<Vec<serde_json::Value>>(malformed,"application/json-seq").is_err()); }
  assert!(poolster_decode_json::<Vec<serde_json::Value>>(b"{\n\"a\":1\n}","application/x-ndjson").is_err());
  let records=serde_json::json!([false,0,null,{"future":"雪"}]);
  for media in ["application/x-ndjson","application/json-seq"] {let encoded=poolster_encode_sequence(&records,media).unwrap();let decoded:serde_json::Value=poolster_decode_json(&encoded,media).unwrap();assert_eq!(decoded,records);}
  assert!(poolster_encode_sequence(&serde_json::json!({}),"application/x-ndjson").is_err());
  assert!(poolster_valid_raw_query("a=0&b=false&name=%E9%9B%AA"));
  for invalid in ["?a=1","a=#fragment","a=%ZZ","a=%","a=\n"] { assert!(!poolster_valid_raw_query(invalid)); }
 }
}
"#;
        std::fs::write(client_path, client_source).unwrap();
        let path = root.path().join("sdk/src/lib.rs");
        let source = std::fs::read_to_string(&path).unwrap()
            + r#"
#[cfg(test)] mod compatibility_probe {
 #[test] fn roundtrip() {
  for wire in [serde_json::json!({}),serde_json::json!({"state":null}),serde_json::json!({"state":"future","nested":{"enum":"unknown","zero":0,"false":false,"null":null}})] {
   let value:crate::models::Envelope=serde_json::from_value(wire.clone()).unwrap();
   if !wire.as_object().unwrap().contains_key("state") { assert!(value.state.is_none()); }
   else if wire["state"].is_null() { assert!(matches!(value.state,Some(None))); }
   else { assert!(!value.state.as_ref().unwrap().as_ref().unwrap().is_known()); }
   assert_eq!(serde_json::to_value(value).unwrap(),wire);
  }
  assert!(crate::models::State::known_value().is_known());
  assert_eq!(crate::models::State::known_value_2().as_str(),"known_value");
 }
}
"#;
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
