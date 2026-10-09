//! Native GraphQL entrypoint configuration and package integration.
use super::*;

#[test]
fn graphql_recipe_scalar_mapping_reaches_generated_selection() {
    let directory = tempfile::tempdir().unwrap();
    let path = recipe(
        directory.path(),
        serde_json::json!([{"language":"typescript","path":"sdk","plugins":[{"name":"graphql","scalars":{"DateTime":{"input":"string","output":"string"}}}]}]),
    );
    std::fs::write(
        directory.path().join("schema.graphql"),
        "scalar DateTime\ntype Query { joinedAt: DateTime! }",
    )
    .unwrap();
    std::fs::write(
        directory.path().join("operations.graphql"),
        "query Joined { joinedAt }",
    )
    .unwrap();
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    let generated = std::fs::read_to_string(
        directory
            .path()
            .join("generated/sdk/graphql/models/Joined.ts"),
    )
    .unwrap();
    assert!(generated.contains("\"joinedAt\": (string)"));
    generate_from_config(&path, ColorChoice::Never, true, false).unwrap();
}

#[test]
fn graphql_rust_recipe_generates_without_http_adaptation() {
    let directory = tempfile::tempdir().unwrap();
    let path = recipe(
        directory.path(),
        serde_json::json!([{"language":"rust","path":"client","name":"graphql_client","plugins":[{"name":"graphql"}]}]),
    );
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    assert!(
        directory
            .path()
            .join("generated/client/Cargo.toml")
            .exists()
    );
    generate_from_config(&path, ColorChoice::Never, true, false).unwrap();
}

#[test]
fn graphql_rust_recipe_scalar_mappings_are_language_specific() {
    let directory = tempfile::tempdir().unwrap();
    let path = recipe(
        directory.path(),
        serde_json::json!([{"language":"rust","path":"client","name":"graphql_client","plugins":[{"name":"graphql","scalars":{"DateTime":{"input":"String","output":"String"}}}]}]),
    );
    std::fs::write(
        directory.path().join("schema.graphql"),
        "scalar DateTime\ntype Query { joinedAt: DateTime! }",
    )
    .unwrap();
    std::fs::write(
        directory.path().join("operations.graphql"),
        "query Joined { joinedAt }",
    )
    .unwrap();
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    generate_from_config(&path, ColorChoice::Never, true, false).unwrap();
    let mut config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    config["packages"][0]["plugins"][0]["scalars"]["DateTime"]["output"] = "string".into();
    std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    assert!(generate_from_config(&path, ColorChoice::Never, false, false).is_err());
}

#[test]
fn direct_graphql_generates_rust_and_mixed_languages_without_skips() {
    for (languages, flags) in [
        ("rust", vec![]),
        ("typescript,rust", vec![]),
        ("rust", vec!["--raw-sdk"]),
        ("typescript,rust", vec!["--client-style", "flat"]),
    ] {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("schema.graphql"),
            "type Query { hello: String! }",
        )
        .unwrap();
        std::fs::write(
            directory.path().join("operations.graphql"),
            "query Hello { hello }",
        )
        .unwrap();
        let mut args = vec![
            OsString::from("generate"),
            directory.path().join("schema.graphql").into_os_string(),
            OsString::from("--input-format"),
            OsString::from("graphql"),
            OsString::from("--operation"),
            directory.path().join("operations.graphql").into_os_string(),
            OsString::from("-o"),
            directory.path().join("generated").into_os_string(),
            OsString::from("-l"),
            OsString::from(languages),
        ];
        args.extend(flags.into_iter().map(OsString::from));
        let Action::Generate(options) = parse(args.into_iter()).unwrap() else {
            panic!()
        };
        assert!(skipped_outputs(&options, options.native_input.as_ref().unwrap()).is_empty());
        generate(*options).unwrap();
        assert!(directory.path().join("generated/rust/Cargo.toml").exists());
        if languages.contains("typescript") {
            assert!(
                directory
                    .path()
                    .join("generated/typescript/package.json")
                    .exists()
            );
        }
    }
}

#[test]
fn graphql_recipe_styles_groups_and_raw_conflicts_are_explicit() {
    let directory = tempfile::tempdir().unwrap();
    for style in ["raw", "flat", "idiomatic", "namespaced"] {
        let path = recipe(
            directory.path(),
            serde_json::json!([
            {"language":"typescript","path":"ts","plugins":[{"name":"graphql","style":style}]},
            {"language":"rust","path":"rust","name":"graphql_client","plugins":[{"name":"graphql","style":style}]}]),
        );
        generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
        let source =
            std::fs::read_to_string(directory.path().join("generated/ts/graphql-client.ts"))
                .unwrap_or_default();
        assert_eq!(
            source.contains("export function createClient"),
            style != "raw"
        );
    }
    let path = recipe(
        directory.path(),
        serde_json::json!([{"language":"typescript","path":"ts","plugins":[{"name":"graphql","style":"flat","raw":true}]}]),
    );
    assert!(
        generate_from_config(&path, ColorChoice::Never, false, false)
            .unwrap_err()
            .to_string()
            .contains("mutually exclusive")
    );
    let path = recipe(
        directory.path(),
        serde_json::json!([{"language":"typescript","path":"ts","plugins":[{"name":"graphql","groups":{"greeting":{"read":"Hello"}}}]}]),
    );
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    let source =
        std::fs::read_to_string(directory.path().join("generated/ts/graphql-client.ts")).unwrap();
    assert!(source.contains("greeting"));
}

#[test]
fn graphql_recipe_selects_contract_scoped_options_with_sdk_alias() {
    let directory = tempfile::tempdir().unwrap();
    let path = recipe(
        directory.path(),
        serde_json::json!([{"language":"typescript","path":"ts","plugins":[{"name":"sdk","contracts":{"http":{"style":"flat"},"graphql":{"style":"grouped","groups":{"greeting":{"read":"Hello"}}}}}]}]),
    );
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    let source =
        std::fs::read_to_string(directory.path().join("generated/ts/graphql-client.ts")).unwrap();
    assert!(source.contains("greeting"));
    let mut config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    config["packages"][0]["plugins"][0]["style"] = "flat".into();
    std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    assert!(
        generate_from_config(&path, ColorChoice::Never, false, false)
            .unwrap_err()
            .to_string()
            .contains("conflicting")
    );
}

#[test]
fn graphql_recipe_ecosystem_generates_with_cypress_mutation_optin() {
    let directory = tempfile::tempdir().unwrap();
    let path = recipe(
        directory.path(),
        serde_json::json!([{"language":"typescript","path":"ts","plugins":[{"name":"sdk","contracts":{"graphql":{"style":"flat"}}},{"name":"react-query"},{"name":"vue-query"},{"name":"swr"},{"name":"zod"},{"name":"faker","fixture_options":{"seed":42}},{"name":"msw"},{"name":"cypress","cypress_options":{"include_mutations":true,"base_url":"http://localhost:4000/graphql"}}]}]),
    );
    std::fs::write(
        directory.path().join("schema.graphql"),
        "type Query { hello: String! } type Mutation { rename(name: String!): String! }",
    )
    .unwrap();
    std::fs::write(
        directory.path().join("operations.graphql"),
        "query Hello { hello } mutation Rename($name:String!) { rename(name:$name) }",
    )
    .unwrap();
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    generate_from_config(&path, ColorChoice::Never, true, false).unwrap();
    assert!(directory.path().join("generated/ts/package.json").exists());
}
