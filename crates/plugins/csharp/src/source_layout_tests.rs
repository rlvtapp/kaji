use super::*;
fn fixture() -> poolster_core::Api {
    let mut api = poolster_core::Api {
        name: "Layout boundary".into(),
        version: "1.0.0".into(),
        ..Default::default()
    };
    api.operations = (0..201)
        .map(|index| poolster_core::Operation {
            id: format!("listItems{index}{}", "y".repeat(64)),
            method: poolster_core::HttpMethod::Get,
            path: format!("/items/{index}"),
            parameters: (0..48)
                .map(|field| poolster_core::OperationParameter {
                    name: format!("field{field}{}", "x".repeat(112)),
                    location: "query".into(),
                    required: false,
                    schema: Some(poolster_core::SchemaValue::new(
                        poolster_core::SchemaKind::String,
                    )),
                    description: None,
                    annotations: Default::default(),
                })
                .collect(),
            annotations: std::collections::BTreeMap::from([(
                "tags".into(),
                serde_json::json!(["Items"]),
            )]),
            ..Default::default()
        })
        .collect();
    api.schemas.push(poolster_core::Schema::new(
        "Large",
        poolster_core::SchemaValue::new(poolster_core::SchemaKind::Object {
            fields: (0..900)
                .map(|index| poolster_core::Field {
                    name: format!("field{index}{}", "z".repeat(128)),
                    value: poolster_core::SchemaValue::new(poolster_core::SchemaKind::String),
                    required: false,
                    annotations: Default::default(),
                })
                .collect(),
            additional_properties: poolster_core::AdditionalProperties::Any,
        }),
    ));
    api
}
#[test]
fn byte_budget_splits_verbose_single_resource_before_count_limit() {
    let tree = presence::render(
        &fixture(),
        "sdk",
        Some("layout-sdk"),
        SdkClientStyle::Namespaced,
        false,
        true,
    )
    .unwrap();
    let chunks = tree
        .iter()
        .filter(|(path, _)| {
            path.to_string_lossy().starts_with("sdk/Operations")
                && path.extension().and_then(|extension| extension.to_str()) == Some("cs")
        })
        .collect::<Vec<_>>();
    assert!(chunks.len() > 2);
    for (path, source) in chunks {
        assert!(
            source.len() <= 128 * 1024,
            "{}: {}",
            path.display(),
            source.len()
        );
    }
    let models = tree
        .iter()
        .filter(|(path, _)| path.to_string_lossy().contains("Models/Large_"))
        .collect::<Vec<_>>();
    assert!(models.len() > 1);
    for (path, source) in models {
        assert!(
            source.len() <= 128 * 1024,
            "{} {}",
            path.display(),
            source.len()
        );
        assert!(source.contains("partial record Large"));
        assert!(source.contains("Presence<string?>"));
    }
    let resources = tree
        .iter()
        .filter(|(path, _)| {
            path.to_string_lossy().starts_with("sdk/Resources")
                && path.extension().and_then(|value| value.to_str()) == Some("cs")
        })
        .collect::<Vec<_>>();
    assert!(resources.len() > 1);
    for (path, source) in resources {
        assert!(
            source.len() <= 128 * 1024,
            "{} {}",
            path.display(),
            source.len()
        );
    }
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let path = tree
        .iter()
        .find(|(path, _)| {
            path.to_string_lossy().starts_with("sdk/Operations")
                && path.extension().and_then(|extension| extension.to_str()) == Some("cs")
        })
        .unwrap()
        .0;
    std::fs::write(root.path().join(path), "customer edit").unwrap();
    assert!(
        tree.write_to(root.path())
            .unwrap_err()
            .to_string()
            .contains("locally modified generated file")
    );
}
#[test]
#[ignore = "requires native SDK toolchain and cached dependencies"]
fn native_byte_grouped_sdk_compiles() {
    let tree = presence::render(
        &fixture(),
        "sdk",
        Some("layout-sdk"),
        SdkClientStyle::Namespaced,
        false,
        true,
    )
    .unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let project = root.path().join("sdk/LayoutSdk.csproj");
    let contents = std::fs::read_to_string(&project).unwrap().replace(
        "<TargetFramework>",
        "<OutputType>Exe</OutputType><TargetFramework>",
    );
    std::fs::write(project, contents).unwrap();
    std::fs::write(root.path().join("sdk/Program.cs"), r#"using System.Text.Json;
using Poolster.LayoutSdk;
var first="field0"+new string('z',128);var last="field899"+new string('z',128);
var input="{\""+first+"\":null,\""+last+"\":\"last\",\"future\":{\"nested\":[1,null,true]}}";
var model=JsonSerializer.Deserialize<Large>(input)!;
var output=JsonSerializer.SerializeToElement(model);
if(output.GetProperty(first).ValueKind!=JsonValueKind.Null || output.GetProperty(last).GetString()!="last" || output.GetProperty("future").GetProperty("nested").GetArrayLength()!=3 || output.TryGetProperty("field1"+new string('z',128),out _))throw new Exception("split partial record roundtrip");
"#).unwrap();
    let result = std::process::Command::new("dotnet")
        .args([
            "run",
            "--project",
            "LayoutSdk.csproj",
            "--nologo",
            "--disable-build-servers",
            "-p:UseSharedCompilation=false",
        ])
        .current_dir(root.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn oversized_atomic_operation_is_diagnosed_without_slicing() {
    let mut api = fixture();
    api.schemas.clear();
    api.operations.truncate(1);
    let parameter = api.operations[0].parameters[0].clone();
    api.operations[0].parameters = (0..800)
        .map(|index| {
            let mut parameter = parameter.clone();
            parameter.name = format!("parameter{index}{}", "p".repeat(128));
            parameter
        })
        .collect();
    let tree = render_test_sdk(&api, "sdk", Some("layout-sdk")).unwrap();
    let diagnostics: serde_json::Value = serde_json::from_str(
        tree.get("sdk/.poolster/source-layout-diagnostics.json")
            .unwrap(),
    )
    .unwrap();
    assert!(
        diagnostics
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["bytes"].as_u64().unwrap() > 128 * 1024)
    );
    let sources = tree.iter().map(|(_, source)| source).collect::<String>();
    assert!(sources.contains(&format!("parameter799{}", "p".repeat(128))));
}
