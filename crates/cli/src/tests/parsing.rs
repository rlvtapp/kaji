use super::*;

#[test]
fn selects_multiple_targets_without_duplicates() {
    let Action::Generate(options) =
        parse(arguments("generate api.yaml -o sdk -l go,rust -l go")).unwrap()
    else {
        panic!()
    };
    assert_eq!(options.languages, ["go", "rust"]);
    assert!(matches!(options.style, SdkClientStyle::Namespaced));
}

#[test]
fn accepts_csharp_and_rejects_removed_dotnet_selector() {
    let Action::Generate(options) = parse(arguments("generate api.yaml -o sdk -l csharp")).unwrap()
    else {
        panic!()
    };
    assert_eq!(options.languages, ["csharp"]);
    assert!(parse(arguments("generate api.yaml -o sdk -l dotnet")).is_err());
}

#[test]
fn parses_repeatable_direct_path_selectors() {
    let Action::Generate(options) = parse(arguments(
        "generate --artifacts cache -o sdk -l go --include-path /messages* --include-path /admin* --exclude-path /admin/audit*",
    ))
    .unwrap() else {
        panic!()
    };
    assert_eq!(options.path_selection.include, ["/messages*", "/admin*"]);
    assert_eq!(options.path_selection.exclude, ["/admin/audit*"]);
}

#[test]
fn validates_flags_sources_and_language_specific_options() {
    for invalid in [
        "generate api.yaml -o sdk",
        "generate -o sdk -l go",
        "generate api.yaml -l go",
        "generate api.yaml --artifacts cache -o sdk -l go",
        "generate api.yaml -o sdk -l go --typescript-surface raw",
        "generate api.yaml -o sdk -l go --client-style bad",
        "generate api.yaml -o sdk -l go --unknown nope",
        "generate api.yaml second.yaml -o sdk -l go",
        "generate --artifacts cache -o sdk -l go --openapi-compiler helper",
        "generate api.yaml -o sdk -l go --jobs 0",
        "generate api.yaml -o sdk -l go --jobs -1",
        "generate api.yaml -o sdk -l go --jobs many",
    ] {
        assert!(parse(arguments(invalid)).is_err(), "{invalid}");
    }
}

#[test]
fn supports_all_targets_and_artifact_reuse() {
    let Action::Generate(options) = parse(arguments(
        "generate --artifacts cache -o sdk -l all --client-style flat",
    ))
    .unwrap() else {
        panic!()
    };
    assert_eq!(options.languages.len(), SDK_LANGUAGES.len());
    assert_eq!(options.artifacts, Some(PathBuf::from("cache")));
}

#[test]
fn path_selectors_are_validated_and_exclusions_win() {
    let selection = PathSelection {
        include: vec!["/messages*".into(), "/admin/users*".into()],
        exclude: vec!["/admin/users/audit*".into()],
    };
    let api = Api {
        name: "Example".into(),
        version: "1".into(),
        operations: vec![
            Operation {
                path: "/messages/send".into(),
                ..Default::default()
            },
            Operation {
                path: "/admin/users".into(),
                ..Default::default()
            },
            Operation {
                path: "/admin/users/audit-log".into(),
                ..Default::default()
            },
            Operation {
                path: "/health".into(),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let sliced = slice_api_paths(api, &selection).unwrap();
    assert_eq!(
        sliced
            .operations
            .iter()
            .map(|operation| operation.path.as_str())
            .collect::<Vec<_>>(),
        ["/messages/send", "/admin/users"]
    );
    assert!(
        validate_path_selection(&PathSelection {
            include: vec!["messages".into()],
            ..Default::default()
        })
        .is_err()
    );
    assert!(
        slice_api_paths(
            Api {
                operations: vec![Operation {
                    path: "/health".into(),
                    ..Default::default()
                }],
                ..Default::default()
            },
            &PathSelection {
                include: vec!["/messages*".into()],
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn color_mode_is_global_and_does_not_force_direct_generation() {
    let Action::Generate(options) = parse(arguments("generate --color always")).unwrap() else {
        panic!()
    };
    assert_eq!(options.config, Some(PathBuf::from("poolster.json")));
    assert_eq!(options.color, ColorChoice::Always);
    assert!(ColorChoice::Always.enabled());
    assert!(!ColorChoice::Never.enabled());
}

#[test]
fn selects_the_generator_control_mcp_server() {
    assert!(matches!(
        parse(arguments("mcp generator")).unwrap(),
        Action::McpGenerator
    ));
    assert!(parse(arguments("mcp generator extra")).is_err());
}

#[test]
fn parses_native_mock_server_options() {
    let Action::MockServe(options) = parse(arguments(
        "mock serve openapi.yaml --port 4011 --openapi-compiler compiler",
    ))
    .unwrap() else {
        panic!()
    };
    assert_eq!(options.source, PathBuf::from("openapi.yaml"));
    assert_eq!(options.port, 4011);
    assert_eq!(options.compiler, Some(PathBuf::from("compiler")));
    assert!(parse(arguments("mock serve openapi.yaml --port 0")).is_err());
}

#[test]
fn parses_contract_check_options() {
    let Action::Check(options) =
        parse(arguments("check openapi.yaml --openapi-compiler compiler")).unwrap()
    else {
        panic!()
    };
    assert_eq!(options.source, PathBuf::from("openapi.yaml"));
    assert_eq!(options.compiler, Some(PathBuf::from("compiler")));
    assert!(parse(arguments("check one.yaml two.yaml")).is_err());
    assert!(parse(arguments("check openapi.yaml --wat")).is_err());
}

#[test]
fn parses_show_update_and_auth_commands() {
    let Action::Show(options) = parse(arguments(
        "show openapi.yaml --include-path /messages* --exclude-path /messages/audit* --json",
    ))
    .unwrap() else {
        panic!()
    };
    assert_eq!(options.source, PathBuf::from("openapi.yaml"));
    assert_eq!(options.paths.include, ["/messages*"]);
    assert_eq!(options.paths.exclude, ["/messages/audit*"]);
    assert_eq!(options.format, ShowFormat::Json);

    let Action::Update(options) = parse(arguments("update --output generated --force")).unwrap()
    else {
        panic!()
    };
    assert_eq!(options.output, PathBuf::from("generated"));
    assert!(options.force);

    let Action::Auth(Auth::Login { profile, token_env }) =
        parse(arguments("auth login github --token-env GITHUB_TOKEN")).unwrap()
    else {
        panic!()
    };
    assert_eq!(profile, "github");
    assert_eq!(token_env, "GITHUB_TOKEN");
    assert!(parse(arguments("auth logout github extra")).is_err());
}

#[test]
fn update_detects_unchanged_local_direct_input() {
    let temporary = tempfile::tempdir().unwrap();
    let source = temporary.path().join("openapi.yaml");
    std::fs::write(&source, "openapi: 3.1.0\n").unwrap();
    let replay = GenerationReplayLock {
        source: Some(source.to_string_lossy().into_owned()),
        artifacts: None,
        languages: vec!["go".into()],
        name: "API".into(),
        version: "1".into(),
        client_style: "namespaced".into(),
        typescript_transport: None,
        typescript_surface: "client".into(),
        typescript_client_name: None,
        go_jobs: None,
        compiler: None,
        paths: PathSelection::default(),
    };
    let input = UpdateInputLock {
        source_sha256: Some(sha256_file(&source).unwrap()),
        artifacts_sha256: "irrelevant".into(),
    };
    assert!(replay_input_is_unchanged(&replay, &input).unwrap());
    std::fs::write(&source, "openapi: 3.1.1\n").unwrap();
    assert!(!replay_input_is_unchanged(&replay, &input).unwrap());
}

#[test]
fn parses_openapi_directory_commands() {
    let Action::Discover(options) =
        parse(arguments("discover github --limit 5 --format json")).unwrap()
    else {
        panic!()
    };
    assert_eq!(options.query, "github");
    assert_eq!(options.limit, 5);
    assert_eq!(options.format, DiscoverFormat::Json);
    assert!(parse(arguments("discover --limit 0 github")).is_err());

    let Action::Download(options) = parse(arguments(
        "download github.com --version 1.1.4 --output contract.yaml",
    ))
    .unwrap() else {
        panic!()
    };
    assert_eq!(options.id, "github.com");
    assert_eq!(options.version.as_deref(), Some("1.1.4"));
    assert_eq!(options.output, PathBuf::from("contract.yaml"));
    assert!(parse(arguments("download github.com")).is_err());
}

#[test]
fn parses_machine_readable_gradual_check_options() {
    let Action::Check(options) = parse(arguments(
        "check openapi.yaml --json --severity missing-operation-id=warning --fail-on none --baseline old.json --write-baseline next.json --ignore ambiguous-path",
    ))
    .unwrap() else {
        panic!()
    };
    assert_eq!(options.format, CheckFormat::Json);
    assert_eq!(
        options.severity_overrides["missing-operation-id"],
        CheckSeverity::Warning
    );
    assert_eq!(options.fail_on, CheckFailureThreshold::None);
    assert_eq!(options.baseline, Some(PathBuf::from("old.json")));
    assert_eq!(options.write_baseline, Some(PathBuf::from("next.json")));
    assert!(options.ignored_rules.contains("ambiguous-path"));
    assert!(parse(arguments("check openapi.yaml --severity unknown=warning")).is_err());
    assert!(
        parse(arguments(
            "check openapi.yaml --severity missing-operation-id=notice"
        ))
        .is_err()
    );
}
