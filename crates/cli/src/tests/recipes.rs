use super::*;

#[test]
fn idempotency_recipe_is_package_local_and_disabled_by_default() {
    let packages: Vec<PackageConfig> = serde_json::from_value(serde_json::json!([
        {"language":"typescript","path":"enabled","plugins":[{"name":"sdk"}], "idempotency":{"operations":{"createItem":{"header":"X-Request-Key"}}}},
        {"language":"typescript","path":"disabled","plugins":[{"name":"sdk"}]}
    ])).unwrap();
    assert!(packages[0].idempotency.operations["createItem"].enabled);
    assert!(!packages[0].idempotency.operations["createItem"].auto_generate);
    assert!(packages[1].idempotency.defaults.is_none());
    let api = Api {
        name: "Example".into(),
        version: "1.0.0".into(),
        operations: vec![poolster_core::Operation {
            id: "createItem".into(),
            method: poolster_core::HttpMethod::Post,
            path: "/items".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let tree = poolster::generate(
        &api,
        config_profiles(SdkClientStyle::Flat, &packages).unwrap(),
    )
    .unwrap();
    let mut enabled = String::new();
    let mut disabled = String::new();
    for (path, contents) in tree.iter() {
        if path.starts_with("enabled") || path.starts_with("./enabled") {
            enabled.push_str(contents);
        } else if path.starts_with("disabled") || path.starts_with("./disabled") {
            disabled.push_str(contents);
        }
    }
    assert!(enabled.contains("X-Request-Key"));
    assert!(!disabled.contains("X-Request-Key"));
    assert!(api.operations[0].parameters.is_empty());
    assert!(serde_json::from_value::<PackageConfig>(serde_json::json!({"language":"typescript","path":"sdk","plugins":[{"name":"sdk"}],"idempotency":{"defaults":{"auto_generate":"yes"}}})).is_err());
}

pub(super) fn combined_optional_packages() -> serde_json::Value {
    serde_json::json!([
      {"language":"python","path":"python","api_reference":true,"plugins":[{"name":"sdk"},{"name":"operation-tests"}],"idempotency":{"defaults":{"enabled":false},"operations":{"createItem":{"header":"X-Key","auto_generate":true}}}},
      {"language":"terraform","path":"terraform","api_reference":true,"plugins":[{"name":"provider","data_sources":true,"infer":false,"resources":[{"name":"item","create":"createItem","read":"getItem","update":"updateItem","delete":"deleteItem","id_parameter":"id","id_field":"id"}]}]}
    ])
}
#[test]
fn optional_security_and_operation_test_consumers_are_available_in_recipes() {
    use poolster_core::{HttpMethod, OperationResponse, SchemaKind, SchemaValue};
    let packages: Vec<PackageConfig> = serde_json::from_value(serde_json::json!([
        {"language":"typescript","path":"ts","plugins":[{"name":"sdk","id":"native"},{"name":"operation-tests","uses":{"operations":"native","transport":"native"}},{"name":"oauth","uses":{"transport":"native"}}]},
        {"language":"rust","path":"rust","plugins":[{"name":"sdk"},{"name":"operation-tests"},{"name":"oauth"}]},
        {"language":"go","path":"go","plugins":[{"name":"sdk"},{"name":"operation-tests"},{"name":"oauth"},{"name":"webhooks"}]},
        {"language":"ruby","path":"ruby","plugins":[{"name":"sdk"},{"name":"webhooks"},{"name":"oauth"}]},
        {"language":"java","path":"java","plugins":[{"name":"sdk"},{"name":"oauth"}]},
        {"language":"csharp","path":"csharp","plugins":[{"name":"sdk"},{"name":"oauth"}]},
        {"language":"swift","path":"swift","plugins":[{"name":"sdk"},{"name":"oauth"}]},
        {"language":"php","path":"php","plugins":[{"name":"sdk"},{"name":"oauth"}]},
        {"language":"elixir","path":"elixir","plugins":[{"name":"sdk"},{"name":"oauth"}]}
    ])).unwrap();
    let api = Api {
        name: "Consumer".into(),
        operations: vec![Operation {
            id: "getItem".into(),
            method: HttpMethod::Get,
            path: "/items".into(),
            responses: vec![OperationResponse::json(
                "200",
                SchemaValue::new(SchemaKind::String),
            )],
            ..Default::default()
        }],
        ..Default::default()
    };
    let tree = poolster::generate(
        &api,
        config_profiles(SdkClientStyle::Namespaced, &packages).unwrap(),
    )
    .unwrap();
    for path in [
        "./ts/tests/operation-tests.ts",
        "./ts/oauth.ts",
        "./rust/src/operation_tests.rs",
        "./rust/src/oauth.rs",
        "./go/oauth.go",
        "./go/webhooks.go",
        "./go/.poolster/operation-test-diagnostics.json",
        "./ruby/lib/consumer_sdk/webhooks.rb",
        "./ruby/lib/consumer_sdk/oauth.rb",
    ] {
        assert!(tree.get(path).is_some(), "missing {path}");
    }
    for (language, helper) in [
        ("java", "OAuthClientCredentials.java"),
        ("csharp", "OAuthClientCredentials.cs"),
        ("swift", "OAuth.swift"),
        ("php", "OAuthClient.php"),
        ("elixir", "oauth.ex"),
    ] {
        assert!(
            tree.iter().any(
                |(path, _)| path.starts_with(format!("./{language}")) && path.ends_with(helper)
            ),
            "missing {language} OAuth helper"
        );
    }
    let mut malformed = packages;
    malformed[0].plugins[1]
        .uses
        .insert("client".into(), "native".into());
    assert!(config_profiles(SdkClientStyle::Namespaced, &malformed).is_err());
}
#[test]
fn rust_open_union_recipe_is_explicit_and_rejects_other_targets() {
    let mut packages: Vec<PackageConfig> = serde_json::from_value(serde_json::json!([
        {"language":"rust","path":"rust","plugins":[{"name":"sdk","open_unions":true}]}
    ]))
    .unwrap();
    let api = Api {
        name: "Future".into(),
        schemas: vec![poolster_core::Schema::new(
            "Event",
            poolster_core::SchemaValue::new(poolster_core::SchemaKind::OneOf {
                variants: vec![poolster_core::SchemaValue::new(
                    poolster_core::SchemaKind::String,
                )],
            }),
        )],
        ..Default::default()
    };
    let tree = poolster::generate(
        &api,
        config_profiles(SdkClientStyle::Namespaced, &packages).unwrap(),
    )
    .unwrap();
    assert!(
        tree.iter()
            .any(|(_, source)| source.contains("Unknown(serde_json::Value)"))
    );
    packages[0].language = "go".into();
    assert!(
        config_profiles(SdkClientStyle::Namespaced, &packages)
            .err()
            .unwrap()
            .to_string()
            .contains("open_unions")
    );
}

#[test]
fn presence_recipe_selects_explicit_model_abi_and_rejects_unsupported_targets() {
    let packages: Vec<PackageConfig> = serde_json::from_value(serde_json::json!([
        {"language":"java","path":"java","plugins":[{"name":"sdk","preserve_presence":true}]},
        {"language":"csharp","path":"csharp","plugins":[{"name":"sdk","preserve_presence":true}]}
    ]))
    .unwrap();
    let api = Api {
        name: "Presence".into(),
        ..Default::default()
    };
    let tree = poolster::generate(
        &api,
        config_profiles(SdkClientStyle::Namespaced, &packages).unwrap(),
    )
    .unwrap();
    for language in ["java", "csharp"] {
        assert!(
            tree.iter()
                .any(|(path, _)| path.starts_with(format!("./{language}"))
                    && path.to_string_lossy().contains("Presence")),
            "missing presence helper for {language}"
        );
    }
    let mut unsupported = packages;
    unsupported[0].language = "go".into();
    assert!(config_profiles(SdkClientStyle::Namespaced, &unsupported).is_err());
}
#[test]
fn every_sdk_recipe_can_select_native_operation_tests() {
    let languages = [
        "typescript",
        "rust",
        "go",
        "python",
        "php",
        "java",
        "csharp",
        "dotnet",
        "elixir",
        "ruby",
        "swift",
    ];
    let packages: Vec<PackageConfig> = serde_json::from_value(serde_json::Value::Array(languages.iter().map(|language| serde_json::json!({"language":language,"path":language,"plugins":[{"name":"sdk"},{"name":"operation-tests"}]})).collect())).unwrap();
    let api = Api {
        name: "Recipe".into(),
        ..Default::default()
    };
    let tree = poolster::generate(
        &api,
        config_profiles(SdkClientStyle::Namespaced, &packages).unwrap(),
    )
    .unwrap();
    for language in languages {
        assert!(
            tree.iter()
                .any(|(path, _)| path.starts_with(format!("./{language}"))
                    && (path.to_string_lossy().contains("operation")
                        || path.to_string_lossy().contains("Operation"))),
            "missing operation test output for {language}"
        );
    }
    let mut malformed = packages;
    let java = malformed
        .iter_mut()
        .find(|package| package.language == "java")
        .unwrap();
    java.plugins[1]
        .uses
        .insert("models".into(), "missing-provider".into());
    assert!(config_profiles(SdkClientStyle::Namespaced, &malformed).is_err());
}
#[test]
fn every_sdk_recipe_can_select_webhook_verification() {
    let languages = [
        "typescript",
        "rust",
        "go",
        "python",
        "php",
        "java",
        "csharp",
        "dotnet",
        "elixir",
        "ruby",
        "swift",
    ];
    let packages: Vec<PackageConfig> = serde_json::from_value(serde_json::Value::Array(languages.iter().map(|language|serde_json::json!({"language":language,"path":language,"plugins":[{"name":"sdk"},{"name":"webhooks"}]})).collect())).unwrap();
    let api = Api {
        name: "Webhook".into(),
        ..Default::default()
    };
    let tree = poolster::generate(
        &api,
        config_profiles(SdkClientStyle::Namespaced, &packages).unwrap(),
    )
    .unwrap();
    for language in languages {
        assert!(
            tree.iter().any(|(path, source)| (path.starts_with(language)
                || path.starts_with(format!("./{language}")))
                && source.contains("webhook-signature")),
            "no verifier emitted for {language}"
        );
    }
}
