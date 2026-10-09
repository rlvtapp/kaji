use super::*;

fn recipe(root: &Path, packages: serde_json::Value) -> PathBuf {
    std::fs::write(root.join("schema.graphql"), "type Query { hello: String! }").unwrap();
    std::fs::write(root.join("operations.graphql"), "query Hello { hello }").unwrap();
    let path = root.join("poolster.json");
    std::fs::write(&path, serde_json::to_vec(&serde_json::json!({
            "input": {"format":"graphql", "provider":"graphql.apollo", "path":"schema.graphql", "options":{"operation_files":["operations.graphql"]}},
            "output":{"path":"generated"}, "packages":packages
        })).unwrap()).unwrap();
    path
}

#[test]
fn native_recipe_resolves_operation_paths_and_checks_regeneration() {
    let directory = tempfile::tempdir().unwrap();
    let path = recipe(
        directory.path(),
        serde_json::json!([{"language":"typescript","path":"sdk","name":"@example/graphql","plugins":[{"name":"graphql"}]}]),
    );
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    assert!(directory.path().join("generated/sdk/package.json").exists());
    generate_from_config(&path, ColorChoice::Never, true, false).unwrap();
    std::fs::write(
        directory.path().join("operations.graphql"),
        "query Renamed { hello }",
    )
    .unwrap();
    assert!(generate_from_config(&path, ColorChoice::Never, true, false).is_err());
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    generate_from_config(&path, ColorChoice::Never, true, false).unwrap();
}

#[test]
fn incompatible_outputs_warn_without_reading_schema_or_writing() {
    let directory = tempfile::tempdir().unwrap();
    let path = recipe(
        directory.path(),
        serde_json::json!([{"language":"go","path":"sdk","plugins":[{"name":"sdk"}]}]),
    );
    std::fs::remove_file(directory.path().join("schema.graphql")).unwrap();
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    assert!(!directory.path().join("generated").exists());
}

#[test]
fn mixed_recipe_preserves_skipped_owned_files_including_local_edits() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("generated");
    let mut previous = GeneratedTree::default();
    previous
        .insert(GeneratedFile::new("go/client.go", "old generated content").unwrap())
        .unwrap();
    previous.set_owner("go/client.go", "go-sdk").unwrap();
    previous.write_to(&output).unwrap();
    std::fs::write(output.join("go/client.go"), "local edits must survive").unwrap();
    let path = recipe(
        directory.path(),
        serde_json::json!([
            {"language":"typescript","path":"ts","plugins":[{"name":"graphql"}]},
            {"language":"go","path":"go","plugins":[{"name":"sdk"}]}
        ]),
    );
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    assert_eq!(
        std::fs::read_to_string(output.join("go/client.go")).unwrap(),
        "local edits must survive"
    );
    assert!(output.join("ts/package.json").exists());
    generate_from_config(&path, ColorChoice::Never, true, false).unwrap();
}

#[test]
fn empty_native_recipe_is_configuration_error_without_output() {
    let directory = tempfile::tempdir().unwrap();
    let path = recipe(directory.path(), serde_json::json!([]));
    let error = generate_from_config(&path, ColorChoice::Never, false, false).unwrap_err();
    assert!(format!("{error:#}").contains("at least one package"));
    assert!(!directory.path().join("generated").exists());
}

#[test]
fn unsupported_options_and_provider_selection_fail_without_export() {
    let directory = tempfile::tempdir().unwrap();
    let path = recipe(
        directory.path(),
        serde_json::json!([{"language":"typescript","path":"ts","plugins":[{"name":"graphql"}]}]),
    );
    let mut config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    config["input"]["provider"] = "graphql.missing".into();
    std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    let error = generate_from_config(&path, ColorChoice::Never, false, false).unwrap_err();
    assert!(format!("{error:#}").contains("unknown input provider"));
    config["input"]["provider"] = "graphql.apollo".into();
    config["input"]["options"]["broker"] = serde_json::json!({"kind":"nats"});
    std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    assert!(generate_from_config(&path, ColorChoice::Never, false, false).is_err());
    assert!(!directory.path().join("generated").exists());
}

#[test]
fn native_flags_parse_without_changing_openapi_default() {
    let Action::Generate(options) = parse("generate schema.graphql --input-format graphql --provider graphql.apollo --operation a.graphql --operation b.graphql -o out -l typescript".split_whitespace().map(OsString::from)).unwrap() else { panic!() };
    let input = options.native_input.unwrap();
    assert_eq!(input.format, "graphql");
    assert_eq!(input.options.operation_files.len(), 2);
    assert!(parse("generate schema.graphql --input-format graphql --operation op.graphql -o out -l rust --raw-sdk --client-style flat".split_whitespace().map(OsString::from)).is_err());
    let Action::Generate(options) = parse(
        "generate api.yaml -o out -l go"
            .split_whitespace()
            .map(OsString::from),
    )
    .unwrap() else {
        panic!()
    };
    assert!(options.native_input.is_none());
    assert!(options.source.is_some());
}

#[test]
fn arazzo_recipe_resolves_sources_and_tracks_regeneration() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("workflow.yaml"),
        include_str!("../../inputs/arazzo/tests/fixtures/runner/workflows.yaml"),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("shop.yaml"),
        include_str!("../../inputs/arazzo/tests/fixtures/runner/shop.openapi.yaml"),
    )
    .unwrap();
    let path = dir.path().join("poolster.json");
    std::fs::write(&path,serde_json::to_vec(&serde_json::json!({"input":{"format":"arazzo","path":"workflow.yaml","options":{"workflow_sources":{"shop":"shop.yaml"}}},"output":{"path":"generated"},"packages":[{"language":"typescript","path":"ts","plugins":[{"name":"workflow"}]}]})).unwrap()).unwrap();
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    generate_from_config(&path, ColorChoice::Never, true, false).unwrap();
    assert!(dir.path().join("generated/ts/workflows.ts").is_file());
    let mut openapi = std::fs::read_to_string(dir.path().join("shop.yaml")).unwrap();
    openapi.push_str("\nx-test-annotation: changed\n");
    std::fs::write(dir.path().join("shop.yaml"), openapi).unwrap();
    assert!(generate_from_config(&path, ColorChoice::Never, true, false).is_err());
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    generate_from_config(&path, ColorChoice::Never, true, false).unwrap();
}

#[test]
fn asyncapi_recipe_emits_kafka_models_and_skip_preserves_other_outputs() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("events.yaml"),
        include_str!("../../inputs/asyncapi/tests/fixtures/kafka-json.yaml"),
    )
    .unwrap();
    let path = dir.path().join("poolster.json");
    std::fs::write(&path,serde_json::to_vec(&serde_json::json!({"input":{"format":"asyncapi","path":"events.yaml","options":{"broker":{"kind":"kafka","brokers":["127.0.0.1:29092"],"client_id":"test"}}},"output":{"path":"generated"},"packages":[{"language":"typescript","path":"ts","plugins":[{"name":"asyncapi"}]}]})).unwrap()).unwrap();
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    assert!(dir.path().join("generated/ts/kafka-runtime.ts").is_file());
    generate_from_config(&path, ColorChoice::Never, true, false).unwrap();
}

#[test]
fn protobuf_native_flags_and_recipe_tool_paths_are_explicit() {
    let Action::Generate(options)=parse("generate api.proto --input-format protobuf -o out -l go --module example.test/api --import-root protos --protoc bin/protoc --protoc-gen-go bin/protoc-gen-go --protoc-gen-go-grpc bin/protoc-gen-go-grpc --go-package api.proto=example.test/api/rpc;rpc".split_whitespace().map(OsString::from)).unwrap() else {panic!()};
    assert_eq!(
        options.native_output.module.as_deref(),
        Some("example.test/api")
    );
    assert_eq!(
        options.native_output.go_packages["api.proto"],
        "example.test/api/rpc;rpc"
    );
    assert_eq!(
        options.native_input.as_ref().unwrap().options.import_roots,
        [std::env::current_dir().unwrap().join("protos")]
    );
    assert!(
        parse(
            "generate api.yaml -o out -l go --module example.test/api"
                .split_whitespace()
                .map(OsString::from)
        )
        .is_err()
    );
    let mut tools = GrpcToolchainConfig {
        protoc: Some("bin/protoc".into()),
        protoc_gen_go: Some("protoc-gen-go".into()),
        ..Default::default()
    };
    tools.resolve(Path::new("/recipe"));
    assert_eq!(tools.protoc, Some(PathBuf::from("/recipe/bin/protoc")));
    assert_eq!(tools.protoc_gen_go, Some(PathBuf::from("protoc-gen-go")));
}

#[path = "generation_graphql_tests.rs"]
mod graphql;
