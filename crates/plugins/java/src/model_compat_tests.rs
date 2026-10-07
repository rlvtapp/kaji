use super::*;
use kaji_core::engine::Packages;
fn api() -> Api {
    let mut enumeration = SchemaValue::new(SchemaKind::String);
    enumeration.enum_values = vec![serde_json::json!("known-value")];
    Api {
        name: "Compatibility".into(),
        version: "1.0.0".into(),
        schemas: vec![
            Schema::new("Count", SchemaValue::new(SchemaKind::Integer)),
            Schema::new(
                "Choice",
                SchemaValue::new(SchemaKind::OneOf {
                    variants: vec![
                        SchemaValue::new(SchemaKind::String),
                        SchemaValue::new(SchemaKind::Integer),
                    ],
                }),
            ),
            Schema::new("State", enumeration),
        ],
        ..Default::default()
    }
}
#[test]
fn transparent_aliases_and_opt_in_enum_values_keep_wire_representation() {
    let tree = Packages::new()
        .package(
            crate::package("sdk")
                .name("io.kaji.compat")
                .with(crate::sdk().open_enums(true)),
        )
        .generate(&api(), None)
        .unwrap();
    assert!(
        tree.get("sdk/src/main/java/io/kaji/compat/model/Count.java")
            .unwrap()
            .contains("JsonCreator.Mode.DELEGATING")
    );
    assert!(
        tree.get("sdk/src/main/java/io/kaji/compat/model/Choice.java")
            .unwrap()
            .contains("JsonNode value")
    );
    assert!(
        tree.get("sdk/src/main/java/io/kaji/compat/model/State.java")
            .unwrap()
            .contains("Extensible wire value")
    );
}
#[test]
#[ignore = "requires Maven+JDK17; executes transparent aliases, union JSON and unknown string enums"]
fn native_models_preserve_alias_union_and_unknown_enum_wire_values() {
    let tree = Packages::new()
        .package(
            crate::package("sdk")
                .name("io.kaji.compat")
                .with(crate::sdk().open_enums(true))
                .with(crate::operation_tests()),
        )
        .generate(&api(), None)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    std::fs::write(dir.path().join("sdk/src/test/java/io/kaji/compat/ModelsProbe.java"), r#"package io.kaji.compat;
import io.kaji.compat.model.*;
import com.fasterxml.jackson.databind.*;
public final class ModelsProbe {
 public static void main(String[] args)throws Exception {
  var mapper=new ObjectMapper();
  var count=mapper.readValue("42",Count.class);if(count.value()!=42L || !mapper.writeValueAsString(count).equals("42"))throw new AssertionError("scalar alias");
  for(var wire:new String[]{"42","true","\"future\"","{\"unknown\":[1,null]}"}) {
   var choice=mapper.readValue(wire,Choice.class);
   if(!mapper.readTree(mapper.writeValueAsString(choice)).equals(mapper.readTree(wire)))throw new AssertionError("union wire");
  }
  var state=mapper.readValue("\"future\"",State.class);
  if(!state.value().equals("future") || !mapper.writeValueAsString(state).equals("\"future\""))throw new AssertionError("unknown enum");
 }
}"#).unwrap();
    let result = std::process::Command::new("mvn")
        .args([
            "-q",
            "test-compile",
            "org.codehaus.mojo:exec-maven-plugin:3.5.0:java",
            "-Dexec.mainClass=io.kaji.compat.ModelsProbe",
            "-Dexec.classpathScope=test",
        ])
        .current_dir(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
