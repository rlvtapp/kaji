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
                .name("Kaji.Compat")
                .with(crate::sdk().open_enums(true)),
        )
        .generate(&api(), None)
        .unwrap();
    assert!(
        tree.get(std::path::Path::new("sdk/Models").join(bounded_filename("Count", 0, "cs")))
            .unwrap()
            .contains("JsonSerializer.Deserialize<long>(ref reader")
    );
    assert!(
        tree.get(std::path::Path::new("sdk/Models").join(bounded_filename("Choice", 1, "cs")))
            .unwrap()
            .contains("JsonSerializer.Deserialize<JsonElement>(ref reader")
    );
    assert!(
        tree.get(std::path::Path::new("sdk/Models").join(bounded_filename("State", 2, "cs")))
            .unwrap()
            .contains("public sealed record State(string Value)")
    );
    assert!(
        tree.get(std::path::Path::new("sdk/Models").join(bounded_filename("State", 2, "cs")))
            .unwrap()
            .contains("new(\"known-value\")")
    );
}
#[test]
#[ignore = "requires .NET8; executes transparent aliases, union JSON and unknown string enums"]
fn native_models_preserve_alias_union_and_unknown_enum_wire_values() {
    let tree = Packages::new()
        .package(
            crate::package("sdk")
                .name("Kaji.Compat")
                .with(crate::sdk().open_enums(true))
                .with(crate::operation_tests()),
        )
        .generate(&api(), None)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    std::fs::write(dir.path().join("sdk/tests/OperationTests/Program.cs"), r#"using System.Text.Json;
using Kaji.KajiCompat;
class Probe {
 static void Main() {
  var count=JsonSerializer.Deserialize<Count>("42")!;
  if(count.Value!=42 || JsonSerializer.Serialize(count)!="42")throw new Exception("scalar alias");
  foreach(var wire in new[]{"42","true","\\\"future\\\"","{\\\"unknown\\\":[1,null]}"}) {
   var choice=JsonSerializer.Deserialize<Choice>(wire)!;
   if(JsonSerializer.Serialize(choice)!=wire)throw new Exception("union wire");
  }
  var state=JsonSerializer.Deserialize<State>("\\\"future\\\"")!;
  if(state.Value!="future" || JsonSerializer.Serialize(state)!="\\\"future\\\"")throw new Exception("unknown enum");
  if(JsonSerializer.Serialize(State.KnownValue)!="\\\"known-value\\\"")throw new Exception("known wire name");
 }
}"#).unwrap();
    let result = std::process::Command::new("dotnet")
        .args([
            "run",
            "--project",
            "tests/OperationTests/OperationTests.csproj",
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
