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

#[test]
fn graphql_recipe_generates_collections_and_both_executable_clis() {
    let directory = tempfile::tempdir().unwrap();
    let path = recipe(
        directory.path(),
        serde_json::json!([
            {"language":"postman","path":"postman","plugins":[{"name":"collection","base_url":"http://localhost:4000/graphql"},{"name":"environment"}]},
            {"language":"rust-cli","path":"rust-command","name":"users-cli","plugins":[{"name":"cli","command_name":"users","base_url":"http://localhost:4000/graphql"}]},
            {"language":"typescript-cli","path":"js-command","plugins":[{"name":"cli","command_name":"users","base_url":"http://localhost:4000/graphql"}]}
        ]),
    );
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    generate_from_config(&path, ColorChoice::Never, true, false).unwrap();
    let root = directory.path().join("generated");
    assert!(root.join("postman/collection.json").exists());
    assert!(root.join("postman/environment.json").exists());
    assert!(root.join("rust-command/Cargo.toml").exists());
    assert!(root.join("js-command/package.json").exists());
}

#[test]
fn graphql_advanced_recipes_dispatch_all_sdk_languages_and_skip_unary_tools() {
    let directory = tempfile::tempdir().unwrap();
    let languages = [
        "typescript",
        "rust",
        "go",
        "python",
        "php",
        "java",
        "csharp",
        "ruby",
        "swift",
        "elixir",
    ];
    for incremental in [false, true] {
        let mut packages=languages.iter().map(|language|serde_json::json!({"language":language,"path":language,"name":if *language=="php"{"example/graphql-client"}else{"graphql_client"},"plugins":[{"name":"graphql","style":"raw","subscriptions":!incremental}]})).collect::<Vec<_>>();
        if incremental {
            packages.push(serde_json::json!({"language":"postman","path":"collection","plugins":[{"name":"collection"}]}));
        }
        let path = recipe(directory.path(), serde_json::json!(packages));
        std::fs::write(directory.path().join("schema.graphql"),"#import \"types.graphql\"\ndirective @defer(if:Boolean! = true,label:String) on FRAGMENT_SPREAD | INLINE_FRAGMENT type Query{user:User!} type Subscription{ticks:User!}").unwrap();
        std::fs::write(
            directory.path().join("types.graphql"),
            "type User{id:ID! name:String!}",
        )
        .unwrap();
        std::fs::write(
            directory.path().join("operations.graphql"),
            if incremental {
                "query Read{user{id ... @defer(label:\"details\"){name}}}"
            } else {
                "query Read{user{id name}} subscription Ticks{ticks{id name}}"
            },
        )
        .unwrap();
        let mut document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        document["input"]["options"]["graphql_incremental"] = incremental.into();
        std::fs::write(&path, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
        generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
        generate_from_config(&path, ColorChoice::Never, true, false).unwrap();
        for language in languages {
            assert!(
                directory.path().join("generated").join(language).is_dir(),
                "{language}"
            );
        }
        if incremental {
            assert!(!directory.path().join("generated/collection").exists());
        }
        let metadata = std::fs::read_to_string(
            directory
                .path()
                .join("generated/.poolster-native-generation.json"),
        )
        .unwrap();
        assert!(metadata.contains("types.graphql"));
    }
}

#[test]
fn graphql_symfony_recipe_generates_bundle_and_preserves_regeneration() {
    let directory = tempfile::tempdir().unwrap();
    let path = recipe(
        directory.path(),
        serde_json::json!([{"language":"symfony","path":"bundle","name":"acme/graphql","plugins":[{"name":"graphql","style":"flat"}]}]),
    );
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    let root = directory.path().join("generated/bundle");
    assert!(root.join("src/Client.php").exists());
    assert!(root.join("src/Symfony/AcmeGraphqlBundle.php").exists());
    let manifest = std::fs::read_to_string(root.join("composer.json")).unwrap();
    assert!(manifest.contains("symfony/http-client"));
    generate_from_config(&path, ColorChoice::Never, true, false).unwrap();
}
