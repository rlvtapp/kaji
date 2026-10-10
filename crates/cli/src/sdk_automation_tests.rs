use super::*;

#[test]
fn periodic_generation_is_explicit_and_validated() {
    for valid in ["17 3 * * *", "*/15 1-4 * * 1,3,5"] {
        validate_schedule(valid).unwrap();
    }
    for invalid in [
        "* * *",
        "60 3 * * *",
        "0 24 * * *",
        "0 0 0 * *",
        "*/0 * * * *",
        "0 0 * * 7",
        "0 0 * * MON",
    ] {
        assert!(validate_schedule(invalid).is_err(), "{invalid}");
    }
    let settings = WorkflowSettings::from_options(
        &parse(
            [
                "init",
                "--schedule",
                "17 3 * * *",
                "--base",
                "develop",
                "--bump",
                "minor",
            ]
            .map(OsString::from),
        )
        .unwrap(),
    )
    .unwrap();
    let mut metadata = PackageMetadata::new("demo");
    metadata.language = "typescript".into();
    metadata.version = "1.0.0".into();
    let files = scaffold(
        "generated",
        "poolster.json",
        None,
        "0.4.0",
        &[LocatedPackage {
            path: ".".into(),
            metadata,
        }],
        &settings,
    )
    .unwrap();
    let workflow: Value = serde_json::from_str(
        files[Path::new(".github/workflows/poolster-sdks.yml")]
            .strip_prefix(WORKFLOW_MARKER)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(workflow["on"]["schedule"][0]["cron"], "17 3 * * *");
    assert!(workflow["on"]["pull_request_target"].is_null());
    assert_eq!(workflow["on"]["push"]["branches"][0], "develop");
    let generation = workflow["jobs"]["generate"]["steps"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()["run"]
        .as_str()
        .unwrap();
    assert!(generation.contains("--base 'develop' --bump 'minor'"));
}

#[test]
fn connect_vendors_reviewable_relay_with_destination_scoped_auth() {
    let options = parse(
        [
            "connect",
            "--repository",
            "acme/sdks",
            "--spec",
            "api/openapi.yaml",
            "--target",
            "spec/openapi.yaml",
        ]
        .map(OsString::from),
    )
    .unwrap();
    let files = connect_scaffold(&options).unwrap();
    assert!(files.contains_key(Path::new(".github/actions/poolster-spec-sync/sync.mjs")));
    let workflow: Value = serde_json::from_str(
        files[Path::new(".github/workflows/poolster-spec-sync.yml")]
            .strip_prefix(WORKFLOW_MARKER)
            .unwrap(),
    )
    .unwrap();
    let steps = workflow["jobs"]["sync"]["steps"].as_array().unwrap();
    let app = steps.iter().find(|step| step["id"] == "app").unwrap();
    assert_eq!(app["with"]["owner"], "acme");
    assert_eq!(app["with"]["repositories"], "sdks");
    assert_eq!(
        steps.last().unwrap()["with"]["target-path"],
        "spec/openapi.yaml"
    );
    assert_eq!(workflow["permissions"]["contents"], "read");
    assert!(workflow["on"]["pull_request"].is_null());
    let invalid = parse(
        [
            "connect",
            "--repository",
            "acme/sdks",
            "--spec",
            "api.yaml",
            "--target",
            ".github/workflows/pwn.yml",
        ]
        .map(OsString::from),
    )
    .unwrap();
    assert!(connect_scaffold(&invalid).is_err());
    let remote = parse(
        [
            "connect",
            "--repository",
            "acme/sdks",
            "--spec",
            "api.yaml",
            "--target",
            "spec.yaml",
            "--auth",
            "broker",
            "--broker-url",
            "https://broker.example",
            "--actions",
            "remote",
            "--action-ref",
            "acme/poolster@commit123",
        ]
        .map(OsString::from),
    )
    .unwrap();
    let files = connect_scaffold(&remote).unwrap();
    assert_eq!(files.len(), 1);
    let workflow: Value = serde_json::from_str(
        files[Path::new(".github/workflows/poolster-spec-sync.yml")]
            .strip_prefix(WORKFLOW_MARKER)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(workflow["permissions"]["id-token"], "write");
    assert_eq!(
        workflow["jobs"]["sync"]["steps"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["uses"],
        "acme/poolster/packages/internal/spec-sync@commit123"
    );
}

#[test]
fn parses_only_command_specific_options() {
    assert!(parse(["init", "--phase", "publish"].map(OsString::from)).is_err());
    assert!(parse(["run", "--root", "generated", "--phase", "test"].map(OsString::from)).is_ok());
    assert!(parse(["pr", "--repository", "a/b", "--dry-run"].map(OsString::from)).is_ok());
}

#[test]
fn metadata_scaffolding_supports_custom_languages_and_literal_paths() {
    let mut metadata = PackageMetadata::new("custom-sdk");
    metadata.language = "custom".into();
    metadata.version = "1.0.0".into();
    metadata.build = vec![PackageCommand::new("custom-compiler", ["build"])];
    metadata.test = vec![PackageCommand::new("custom-compiler", ["test"])];
    let files = scaffold(
        "generated with spaces",
        "poolster.json",
        None,
        "0.4.0",
        &[LocatedPackage {
            path: "custom".into(),
            metadata,
        }],
        &WorkflowSettings::default(),
    )
    .unwrap();
    let ci = files
        .get(Path::new(".github/workflows/poolster-sdk-ci.yml"))
        .unwrap();
    let parsed: Value = serde_json::from_str(ci.strip_prefix(WORKFLOW_MARKER).unwrap()).unwrap();
    assert_eq!(
        parsed.pointer("/jobs/sdk/strategy/matrix/include/0/language"),
        Some(&json!("custom"))
    );
    assert!(!files.contains_key(Path::new("release-please-config.json")));
    assert!(
        scaffold(
            "../outside",
            "poolster.json",
            None,
            "0.4.0",
            &[],
            &WorkflowSettings::default()
        )
        .is_err()
    );
    assert!(repository("owner/repo; touch file").is_err());
    assert_eq!(shell_word("it's $(literal)"), "'it'\\''s $(literal)'");
}

#[test]
fn discovery_validates_metadata_and_skips_symlink_directories() {
    let dir = tempfile::tempdir().unwrap();
    let package = dir.path().join("sdk/.poolster");
    fs::create_dir_all(&package).unwrap();
    let mut value = PackageMetadata::new("sdk");
    value.language = "custom".into();
    value.version = "1.0.0".into();
    fs::write(package.join("package.json"), value.to_json().unwrap()).unwrap();
    assert_eq!(packages(dir.path()).unwrap()[0].path, "sdk");
    value.schema_version = 99;
    fs::write(
        package.join("package.json"),
        serde_json::to_string(&value).unwrap(),
    )
    .unwrap();
    assert!(packages(dir.path()).is_err());
}

#[test]
fn inventories_store_portable_paths_and_normalize_native_owned_keys() {
    let path = Path::new("sdk").join(".poolster").join("package.json");
    let files = BTreeMap::from([(path.clone(), "contents".into())]);
    let stored = serde_json::to_value(portable_inventory(&files).unwrap()).unwrap();
    assert_eq!(stored["sdk/.poolster/package.json"], "contents");
    let fingerprint = json!({"sha256":"unchanged", "owner":"test"});
    let mut inventory = json!({"version":1,"files":{path.to_str().unwrap():fingerprint.clone()}});
    normalize_owned_paths(&mut inventory).unwrap();
    assert_eq!(
        inventory["files"]["sdk/.poolster/package.json"],
        fingerprint
    );
    assert!(normalize_owned_paths(&mut json!({"files":{"../escape":{}}})).is_err());
    assert!(
        portable_inventory(&BTreeMap::from([(
            PathBuf::from("../escape"),
            "unsafe".into()
        )]))
        .is_err()
    );
}

#[test]
fn setup_paths_accept_native_nested_joins_without_accepting_escape() {
    let root = tempfile::tempdir().unwrap();
    let relative = Path::new(".poolster")
        .join("sdk-repository-setup")
        .join("acme")
        .join("api-typescript")
        .join(".github")
        .join("workflows")
        .join("release.yml");
    assert_eq!(
        setup_path(root.path(), &relative).unwrap(),
        root.path().join(&relative)
    );
    assert!(setup_path(root.path(), Path::new("../outside")).is_err());
    assert!(setup_path(root.path(), root.path()).is_err());
    assert!(relative_path("sdk\\nested").is_err());
    assert!(setup_path(root.path(), Path::new(".")).is_ok());
}

#[test]
fn git_tree_paths_use_repository_separators_and_reject_escape() {
    let path = Path::new(".")
        .join("sdk")
        .join(".poolster")
        .join("package.json");
    assert_eq!(git_tree_path(&path).unwrap(), "sdk/.poolster/package.json");
    assert_eq!(
        git_tree_path(&Path::new("rust").join("Cargo.toml")).unwrap(),
        "rust/Cargo.toml"
    );
    assert_eq!(
        git_tree_path(Path::new("configs/../schema.yaml")).unwrap(),
        "schema.yaml"
    );
    assert!(git_tree_path(Path::new("../outside")).is_err());
    assert!(git_tree_path(Path::new("")).is_err());
}

#[test]
fn package_arguments_are_passed_without_shell_expansion() {
    let dir = tempfile::tempdir().unwrap();
    let mut metadata = PackageMetadata::new("literal");
    metadata.language = "custom".into();
    metadata.version = "1".into();
    metadata.test = vec![PackageCommand::new(
        "git",
        ["rev-parse", "--sq-quote", "$(touch SHOULD_NOT_EXIST)"],
    )];
    run_commands(
        dir.path(),
        &LocatedPackage {
            path: ".".into(),
            metadata,
        },
        "test",
    )
    .unwrap();
    assert!(!dir.path().join("SHOULD_NOT_EXIST").exists());
}

#[test]
fn setup_is_reviewable_and_preserves_release_versions_and_local_edits() {
    let directory = tempfile::tempdir().unwrap();
    let files = BTreeMap::from([
        (
            PathBuf::from(".github/workflows/ci.yml"),
            format!("{WORKFLOW_MARKER}one"),
        ),
        (
            PathBuf::from(".release-please-manifest.json"),
            "{\"sdk\":\"1.0.0\"}".into(),
        ),
    ]);
    write_scaffold(directory.path(), files.clone(), true).unwrap();
    assert!(!directory.path().join(".github").exists());
    write_scaffold(directory.path(), files.clone(), false).unwrap();
    fs::write(
        directory.path().join(".release-please-manifest.json"),
        "{\"sdk\":\"2.0.0\"}",
    )
    .unwrap();
    let mut update = files;
    update.insert(
        ".github/workflows/ci.yml".into(),
        format!("{WORKFLOW_MARKER}two"),
    );
    write_scaffold(directory.path(), update.clone(), false).unwrap();
    assert!(
        fs::read_to_string(directory.path().join(".release-please-manifest.json"))
            .unwrap()
            .contains("2.0.0")
    );
    fs::write(
        directory.path().join(".github/workflows/ci.yml"),
        "handwritten customization",
    )
    .unwrap();
    assert!(write_scaffold(directory.path(), update, false).is_err());
    assert_eq!(
        fs::read_to_string(directory.path().join(".github/workflows/ci.yml")).unwrap(),
        "handwritten customization"
    );
}

fn git_init(root: &Path) {
    captured(root, "git", &["init", "-b", "main"]).unwrap();
    captured(root, "git", &["config", "user.name", "Test"]).unwrap();
    captured(
        root,
        "git",
        &["config", "user.email", "test@example.invalid"],
    )
    .unwrap();
}
fn git_commit(root: &Path, message: &str) {
    captured(root, "git", &["add", "."]).unwrap();
    captured(root, "git", &["commit", "-m", message]).unwrap();
}
#[test]
fn release_matrix_reads_immutable_tag_and_rejects_unsafe_retry_inputs() {
    let root = tempfile::tempdir().unwrap();
    git_init(root.path());
    let path = root.path().join("sdk/.poolster");
    fs::create_dir_all(&path).unwrap();
    let mut metadata = PackageMetadata::new("@acme/sdk");
    metadata.language = "typescript".into();
    metadata.version = "1.2.3".into();
    metadata.publisher = Some(poolster_core::release::PackagePublisher {
        registry: "npm".into(),
        release_type: "node".into(),
        commands: vec![],
        extra_files: vec![],
    });
    fs::write(path.join("package.json"), metadata.to_json().unwrap()).unwrap();
    git_commit(root.path(), "release");
    captured(root.path(), "git", &["tag", "sdk-v1.2.3"]).unwrap();
    metadata.version = "9.9.9".into();
    fs::write(path.join("package.json"), metadata.to_json().unwrap()).unwrap();
    git_commit(root.path(), "later main");
    let matrix = release_matrix(
        root.path(),
        &json!({"paths_released":"[\"sdk\"]","sdk--tag_name":"sdk-v1.2.3"}),
        "",
        "",
    )
    .unwrap();
    assert_eq!(matrix["include"][0]["tag"], "sdk-v1.2.3");
    assert_eq!(matrix["include"][0]["standard"], true);
    assert!(release_matrix(root.path(), &json!({}), "sdk", "").is_err());
    assert!(release_matrix(root.path(), &json!({}), "../escape", "sdk-v1.2.3").is_err());
    assert!(release_matrix(root.path(), &json!({}), "sdk", "HEAD:other").is_err());
    assert_eq!(
        release_matrix(root.path(), &json!({}), "", "").unwrap()["include"],
        json!([])
    );
}

#[test]
fn all_sdk_languages_receive_readable_checks_and_gated_publish_sources() {
    let languages = [
        "typescript",
        "python",
        "go",
        "rust",
        "java",
        "csharp",
        "swift",
        "php",
        "ruby",
        "elixir",
    ];
    let packages = languages
        .iter()
        .map(|language| {
            let mut metadata = PackageMetadata::new(format!("example-{language}"));
            metadata.language = (*language).into();
            metadata.version = "1.2.3".into();
            metadata.build = vec![PackageCommand::new("native-build", [*language])];
            metadata.test = vec![PackageCommand::new("native-test", [*language])];
            metadata.publisher = Some(poolster_core::release::PackagePublisher {
                registry: match *language {
                    "typescript" => "npm",
                    "python" => "pypi",
                    "rust" => "crates.io",
                    "go" => "go",
                    _ => "custom",
                }
                .into(),
                release_type: "simple".into(),
                commands: if ["typescript", "python", "rust", "go"].contains(language) {
                    vec![]
                } else {
                    vec![PackageCommand::new("reviewed-publish", [*language])]
                },
                extra_files: vec![],
            });
            LocatedPackage {
                path: (*language).into(),
                metadata,
            }
        })
        .collect::<Vec<_>>();
    let files = routed_scaffold(
        "generated",
        "poolster.json",
        "example/sdk-{lang}",
        "0.5.0",
        &packages,
        &WorkflowSettings::default(),
    )
    .unwrap();
    for language in languages {
        let prefix = format!(".poolster/sdk-repository-setup/example/sdk-{language}");
        for helper in [
            "check/action.yml",
            "check/check.mjs",
            "publish/action.yml",
            "publish/publish.mjs",
        ] {
            assert!(
                files.contains_key(&PathBuf::from(format!(
                    "{prefix}/.github/actions/poolster-{helper}"
                ))),
                "{language}/{helper}"
            );
        }
        let release: Value = serde_json::from_str(
            files[&PathBuf::from(format!(
                "{prefix}/.github/workflows/poolster-sdk-release.yml"
            ))]
                .strip_prefix(WORKFLOW_MARKER)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(release["permissions"]["contents"], "read");
        assert!(release["jobs"]["check"]["permissions"]["id-token"].is_null());
        assert_eq!(
            release["jobs"]["publish"]["permissions"]["id-token"],
            "write"
        );
        assert!(
            release["jobs"]["publish"]["needs"]
                .as_array()
                .unwrap()
                .contains(&json!("check"))
        );
        assert_eq!(
            release["jobs"]["check"]["steps"][0]["with"]["ref"],
            "${{ matrix.tag }}"
        );
        assert!(release["on"]["pull_request_target"].is_null());
        let ci: Value = serde_json::from_str(
            files[&PathBuf::from(format!("{prefix}/.github/workflows/poolster-sdk-ci.yml"))]
                .strip_prefix(WORKFLOW_MARKER)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            ci["jobs"]["sdk"]["strategy"]["matrix"]["include"][0]["language"],
            language
        );
        assert_eq!(
            ci["jobs"]["sdk"]["strategy"]["matrix"]["include"][0]["runner"],
            if language == "swift" {
                "macos-14"
            } else {
                "ubuntu-latest"
            }
        );
    }
}

#[test]
fn repository_routing_isolates_workflows_packages_and_app_scope() {
    let packages = [
        ("typescript", "web"),
        ("typescript", "admin"),
        ("python", "python"),
    ]
    .into_iter()
    .map(|(language, path)| {
        let mut metadata = PackageMetadata::new(path);
        metadata.language = language.into();
        metadata.version = "1.2.3".into();
        metadata.publisher = Some(poolster_core::release::PackagePublisher {
            registry: if language == "python" { "pypi" } else { "npm" }.into(),
            release_type: "simple".into(),
            commands: vec![],
            extra_files: vec![],
        });
        LocatedPackage {
            path: path.into(),
            metadata,
        }
    })
    .collect::<Vec<_>>();
    let files = routed_scaffold(
        "generated",
        "poolster.json",
        "acme/api-{lang}",
        "0.4.0",
        &packages,
        &WorkflowSettings::default(),
    )
    .unwrap();
    let source: Value = serde_json::from_str(
        files[Path::new(".github/workflows/poolster-sdks.yml")]
            .strip_prefix(WORKFLOW_MARKER)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(source["jobs"].as_object().unwrap().len(), 2);
    for (language, expected) in [
        ("typescript", vec!["generated/admin", "generated/web"]),
        ("python", vec!["generated/python"]),
    ] {
        let job = &source["jobs"][format!("generate_{language}")];
        let steps = job["steps"].as_array().unwrap();
        assert!(
            steps.last().unwrap()["run"]
                .as_str()
                .unwrap()
                .contains(&format!("--language '{language}'"))
        );
        assert_eq!(
            steps.iter().find(|step| step["id"] == "app").unwrap()["with"]["repositories"],
            format!("api-{language}")
        );
        let prefix = format!(".poolster/sdk-repository-setup/acme/api-{language}");
        let config: Value = serde_json::from_str(
            &files[&PathBuf::from(format!("{prefix}/release-please-config.json"))],
        )
        .unwrap();
        assert_eq!(
            config["packages"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            expected
        );
        assert!(files.contains_key(&PathBuf::from(format!(
            "{prefix}/.github/workflows/poolster-sdk-ci.yml"
        ))));
        assert!(files.contains_key(&PathBuf::from(format!(
            "{prefix}/.github/workflows/poolster-sdk-release.yml"
        ))));
        assert!(files.contains_key(&PathBuf::from(format!(
            "{prefix}/.github/actions/poolster-publish/publish.mjs"
        ))));
    }
    for invalid in [
        "acme/api",
        "acme/{lang}-{lang}",
        "acme/{other}-{lang}",
        "../{lang}",
    ] {
        assert!(
            routed_scaffold(
                "generated",
                "poolster.json",
                invalid,
                "0.4.0",
                &packages,
                &WorkflowSettings::default()
            )
            .is_err()
        );
    }
}

#[test]
fn release_setup_vendors_editable_actions_and_scopes_app_tokens() {
    let mut metadata = PackageMetadata::new("demo");
    metadata.language = "go".into();
    metadata.version = "1.0.0".into();
    metadata.publisher = Some(poolster_core::release::PackagePublisher {
        registry: "go".into(),
        release_type: "go".into(),
        commands: vec![],
        extra_files: vec![],
    });
    let packages = [LocatedPackage {
        path: "go".into(),
        metadata,
    }];
    let setup = scaffold(
        "generated",
        "poolster.json",
        None,
        "0.4.0",
        &packages,
        &WorkflowSettings::default(),
    )
    .unwrap();
    assert!(setup.contains_key(Path::new(".github/actions/poolster-check/check.mjs")));
    assert!(setup.contains_key(Path::new(".github/actions/poolster-publish/publish.mjs")));
    let config: Value =
        serde_json::from_str(&setup[Path::new("release-please-config.json")]).unwrap();
    assert_eq!(config["packages"]["generated/go"]["tag-separator"], "/");
    let release: Value = serde_json::from_str(
        setup[Path::new(".github/workflows/poolster-sdk-release.yml")]
            .strip_prefix(WORKFLOW_MARKER)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        release["jobs"]["publish"]["steps"][0]["with"]["ref"],
        "${{ matrix.tag }}"
    );
    assert!(release["jobs"]["check"]["permissions"].is_null());
    assert_eq!(
        release["jobs"]["publish"]["permissions"]["id-token"],
        "write"
    );
    let external = scaffold(
        "generated",
        "poolster.json",
        Some("acme/sdks"),
        "0.4.0",
        &packages,
        &WorkflowSettings::default(),
    )
    .unwrap();
    assert!(external.contains_key(Path::new(
        ".poolster/sdk-repository-setup/.github/workflows/poolster-sdk-release.yml"
    )));
    assert!(!external.contains_key(Path::new(".github/workflows/poolster-sdk-release.yml")));
    let source: Value = serde_json::from_str(
        external[Path::new(".github/workflows/poolster-sdks.yml")]
            .strip_prefix(WORKFLOW_MARKER)
            .unwrap(),
    )
    .unwrap();
    let step = source["jobs"]["generate"]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["id"] == "app")
        .unwrap();
    assert_eq!(step["with"]["owner"], "acme");
    assert_eq!(step["with"]["repositories"], "sdks");
}

#[test]
fn app_manifest_and_broker_setup_do_not_request_workflow_write() {
    let options = parse(["app", "--dry-run"].map(OsString::from)).unwrap();
    let manifest: Value = serde_json::from_str(&app_manifest(&options).unwrap()).unwrap();
    assert_eq!(manifest["public"], false);
    assert!(manifest["default_permissions"]["workflows"].is_null());
    let invalid = parse(
        [
            "init",
            "--auth",
            "broker",
            "--broker-url",
            "http://insecure.example",
        ]
        .map(OsString::from),
    )
    .unwrap();
    assert!(WorkflowSettings::from_options(&invalid).is_err());
    let valid = parse(
        [
            "sync",
            "--auth",
            "broker",
            "--broker-url",
            "https://broker.example",
        ]
        .map(OsString::from),
    )
    .unwrap();
    let settings = WorkflowSettings::from_options(&valid).unwrap();
    assert!(settings.token_step(None).is_some());
}
#[test]
fn existing_pr_branch_keeps_handwritten_commits_and_reports_revision() {
    let remote = tempfile::tempdir().unwrap();
    git_init(remote.path());
    let mut old = GeneratedTree::default();
    old.insert(GeneratedFile::new("generated/model.rs", "old").unwrap())
        .unwrap();
    old.write_to(remote.path()).unwrap();
    git_commit(remote.path(), "base");
    captured(
        remote.path(),
        "git",
        &["switch", "-c", "codex/poolster-sdks"],
    )
    .unwrap();
    fs::write(remote.path().join("README.md"), "handwritten PR addition").unwrap();
    git_commit(remote.path(), "manual PR work");
    let revision = captured(remote.path(), "git", &["rev-parse", "HEAD"]).unwrap();
    captured(remote.path(), "git", &["switch", "main"]).unwrap();
    let checkout = tempfile::tempdir().unwrap();
    let path = checkout.path().join("checkout");
    captured(
        checkout.path(),
        "git",
        &[
            "clone",
            "--branch",
            "main",
            remote.path().to_str().unwrap(),
            path.to_str().unwrap(),
        ],
    )
    .unwrap();
    let config = checkout.path().join("gitconfig");
    fs::write(&config, "").unwrap();
    assert_eq!(
        prepare_pr_branch(&path, "codex/poolster-sdks", true, &config)
            .unwrap()
            .unwrap(),
        revision.trim()
    );
    assert_eq!(
        fs::read_to_string(path.join("README.md")).unwrap(),
        "handwritten PR addition"
    );
    assert_eq!(
        captured(&path, "git", &["rev-parse", "HEAD"]).unwrap(),
        revision
    );
    // No push occurs; the fetched revision is the parent of future updates.
}
#[test]
fn transfer_preserves_released_versions_and_adopts_only_proven_version_edits() {
    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    git_init(target.path());
    let mut metadata = PackageMetadata::new("demo");
    metadata.language = "rust".into();
    metadata.version = "0.1.0".into();
    let mut old = GeneratedTree::default();
    old.insert(
        GeneratedFile::new("rust/.poolster/package.json", metadata.to_json().unwrap()).unwrap(),
    )
    .unwrap();
    old.insert(
        GeneratedFile::new(
            "rust/Cargo.toml",
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n",
        )
        .unwrap(),
    )
    .unwrap();
    old.write_to(target.path()).unwrap();
    git_commit(target.path(), "generated");
    metadata.version = "2.3.4".into();
    fs::write(
        target.path().join("rust/.poolster/package.json"),
        metadata.to_json().unwrap(),
    )
    .unwrap();
    fs::write(
        target.path().join("rust/Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"2.3.4\"\n\n[dependencies]\nserde = \"1\"\n",
    )
    .unwrap();
    git_commit(target.path(), "release version");
    metadata.version = "0.1.0".into();
    let mut fresh = GeneratedTree::default();
    fresh
        .insert(
            GeneratedFile::new("rust/.poolster/package.json", metadata.to_json().unwrap()).unwrap(),
        )
        .unwrap();
    fresh.insert(GeneratedFile::new("rust/Cargo.toml","[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"2\"\n").unwrap()).unwrap();
    fresh.write_to(source.path()).unwrap();
    transfer_owned_output(source.path(), target.path()).unwrap();
    let result = fs::read_to_string(target.path().join("rust/Cargo.toml")).unwrap();
    assert!(result.contains("2.3.4"));
    assert!(result.contains("serde = \"2\""));
    let result: Value = serde_json::from_slice(
        &fs::read(target.path().join("rust/.poolster/package.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(result["version"], "2.3.4");
    fs::write(target.path().join("rust/Cargo.toml"), result.to_string()).unwrap();
    assert!(transfer_owned_output(source.path(), target.path()).is_err());
}
#[cfg(unix)]
#[test]
fn git_auth_setup_is_scoped_and_remote_detection_needs_no_source_api() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let mock = root.path().join("mock-gh");
    let config = root.path().join("gitconfig");
    fs::write(&mock,"#!/bin/sh\n[ \"$1 $2 $3 $4\" = \"auth setup-git --hostname github.com\" ] || exit 2\nprintf '[credential \"https://github.com\"]\n helper = mock-helper\n' > \"$GIT_CONFIG_GLOBAL\"\n").unwrap();
    fs::set_permissions(&mock, fs::Permissions::from_mode(0o755)).unwrap();
    configured_capture(
        root.path(),
        mock.to_str().unwrap(),
        &["auth", "setup-git", "--hostname", "github.com"],
        &config,
    )
    .unwrap();
    assert!(fs::read_to_string(config).unwrap().contains("mock-helper"));
    assert_eq!(
        github_repository_from_remote("git@github.com:owner/source.git"),
        Some("owner/source".into())
    );
    assert_eq!(
        github_repository_from_remote("https://github.com/owner/source.git"),
        Some("owner/source".into())
    );
    assert!(github_repository_from_remote("https://token@github.com/owner/source.git").is_none());
}

#[test]
fn repository_transfer_keeps_handwritten_files_and_removes_only_owned_stale_files() {
    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let mut old = GeneratedTree::default();
    old.insert(GeneratedFile::new("old.rs", "old generated").unwrap())
        .unwrap();
    old.write_to(target.path()).unwrap();
    fs::write(target.path().join("README.md"), "handwritten").unwrap();
    let mut new = GeneratedTree::default();
    new.insert(GeneratedFile::new("new.rs", "new generated").unwrap())
        .unwrap();
    new.set_owner("new.rs", "custom-provider").unwrap();
    new.write_to(source.path()).unwrap();
    transfer_owned_output(source.path(), target.path()).unwrap();
    assert!(!target.path().join("old.rs").exists());
    assert_eq!(
        fs::read_to_string(target.path().join("README.md")).unwrap(),
        "handwritten"
    );
    assert_eq!(
        fs::read_to_string(target.path().join("new.rs")).unwrap(),
        "new generated"
    );
    fs::write(target.path().join("new.rs"), "manual patch").unwrap();
    assert!(transfer_owned_output(source.path(), target.path()).is_err());
}

#[cfg(unix)]
#[test]
fn setup_refuses_symlink_directories_before_writing() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join(".github")).unwrap();
    let files = BTreeMap::from([(
        PathBuf::from(".github/workflows/ci.yml"),
        "generated".into(),
    )]);
    assert!(write_scaffold(root.path(), files, false).is_err());
    assert!(!outside.path().join("workflows").exists());
}
#[test]
fn language_selection_keeps_variants_paths_and_validates_metadata() {
    let mut recipe = json!({"output":{"path":"sdks/rust"},"packages":[
        {"language":"rust","path":"sync","release":{}},
        {"language":"typescript","path":"ts","release":{}},
        {"language":"rust","path":"async","release":{"language":"rust"}}
    ]});
    select_recipe_language(&mut recipe, "rust").unwrap();
    assert_eq!(recipe["packages"].as_array().unwrap().len(), 2);
    assert_eq!(recipe["packages"][1]["path"], "async");
    assert_eq!(recipe["output"]["path"], "sdks/rust");
    assert!(select_recipe_language(&mut recipe.clone(), "go").is_err());
    recipe["packages"][0]["release"]["language"] = json!("go");
    assert!(select_recipe_language(&mut recipe, "rust").is_err());
    let mut missing = json!({"packages":[{"language":"rust"}]});
    assert!(select_recipe_language(&mut missing, "rust").is_err());
}

#[test]
fn language_destination_guard_rejects_foreign_owned_sdk_without_mutation() {
    let root = tempfile::tempdir().unwrap();
    git_init(root.path());
    let mut tree = GeneratedTree::default();
    for language in ["rust", "go"] {
        let mut metadata = PackageMetadata::new(format!("demo-{language}"));
        metadata.language = language.into();
        metadata.version = "3.2.1".into();
        tree.insert(
            GeneratedFile::new(
                format!("{language}/.poolster/package.json"),
                metadata.to_json().unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    }
    tree.write_to(root.path()).unwrap();
    git_commit(root.path(), "mixed output");
    assert!(
        ensure_owned_language(root.path(), "rust")
            .unwrap_err()
            .to_string()
            .contains("another SDK language")
    );
    assert!(
        captured(root.path(), "git", &["status", "--porcelain"])
            .unwrap()
            .trim()
            .is_empty()
    );
    let single = tempfile::tempdir().unwrap();
    let mut metadata = PackageMetadata::new("demo");
    metadata.language = "rust".into();
    metadata.version = "3.2.1".into();
    let mut tree = GeneratedTree::default();
    tree.insert(
        GeneratedFile::new(
            "variant/.poolster/package.json",
            metadata.to_json().unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    tree.write_to(single.path()).unwrap();
    ensure_owned_language(single.path(), "rust").unwrap();
    let generated = tempfile::tempdir().unwrap();
    transfer_owned_output(single.path(), generated.path()).unwrap();
    ensure_owned_language(generated.path(), "rust").unwrap();
    let preserved: PackageMetadata = serde_json::from_slice(
        &fs::read(generated.path().join("variant/.poolster/package.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(preserved.version, "3.2.1");
}
#[test]
fn api_release_notes_are_structured_deterministic_and_projected() {
    let input = json!([
        {"id":"endpoint-removed","level":"ERR","operation":"DELETE","path":"/notes/{id}","operationId":"deleteNote","text":"Operation removed","source":"PRIVATE","args":["SECRET"]},
        {"id":"endpoint-added","level":"INFO","operation":"GET","path":"/notes","text":"Operation added"},
        {"id":"endpoint-added","level":"INFO","operation":"GET","path":"/notes","text":"Operation added"}
    ]);
    let entries = api_change_entries(&input).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].id, "endpoint-added");
    let projected = serde_json::to_string(&entries).unwrap();
    assert!(!projected.contains("PRIVATE") && !projected.contains("SECRET"));
    assert!(api_change_markdown(&entries).contains("DELETE /notes/{id}"));
    assert!(api_change_entries(&json!({})).is_err());
    assert!(api_change_entries(&json!([{"text":"unknown"}])).is_err());
}
#[cfg(unix)]
#[test]
fn structured_diff_executes_mock_oasdiff_without_shell_or_path_changes() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let binary = root.path().join("mock-oasdiff");
    fs::write(&binary, r#"#!/bin/sh
if [ "$1" = "breaking" ]; then exit 1; fi
if [ "$3" = "json" ]; then
  printf '%s' '[{"id":"endpoint-removed","level":3,"operation":"GET","path":"/old","operationId":"old","text":"Operation removed"}]'
  exit 0
fi
exit 1
"#).unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    let result = spec_diff_using(
        binary.to_str().unwrap(),
        "$(literal).json",
        "new.json",
        None,
    )
    .unwrap();
    assert_eq!(result.bump, "major");
    assert_eq!(result.entries[0].operation_id.as_deref(), Some("old"));
    assert!(result.changelog.contains("GET /old"));
    assert!(!root.path().join("literal").exists());
}
#[test]
fn nested_api_notes_preserve_selected_bump_and_remove_source_directives() {
    let mut entries=api_change_entries(&json!([{"id":"removed","level":"ERR","operation":"GET","path":"/notes","text":"line one\nfeat!: INJECT\nBEGIN_NESTED_COMMIT Release-As: 99.0.0 END_COMMIT_OVERRIDE","comment":"BREAKING CHANGE: INJECT"}])).unwrap();
    entries[0].disclaimers.push("END_NESTED_COMMIT".into());
    let diff = SpecDiff {
        bump: "patch".into(),
        changelog: String::new(),
        entries,
    };
    let message = release_commit_message("fix(sdk): update generated SDKs", &diff);
    assert!(message.starts_with("fix(sdk): update generated SDKs\n"));
    assert_eq!(message.matches("BEGIN_NESTED_COMMIT").count(), 1);
    assert_eq!(message.matches("END_NESTED_COMMIT").count(), 1);
    assert!(
        !message.contains("END_COMMIT_OVERRIDE")
            && !message.contains("Release-As:")
            && !message.contains("BREAKING CHANGE:")
    );
    assert_eq!(
        message
            .lines()
            .filter(|line| line.starts_with("fix(api):"))
            .count(),
        1
    );
    assert!(!message.lines().any(|line| line.starts_with("feat!")));
    let markdown = api_change_markdown(&diff.entries);
    assert!(!markdown.contains("BEGIN_NESTED_COMMIT") && !markdown.contains("END_COMMIT_OVERRIDE"));
    let oversized=std::iter::repeat_with(|| api_change_entries(&json!([{"id":"large","level":"INFO","text":"x".repeat(10_000),"comment":"y".repeat(10_000)}])).unwrap().remove(0)).take(101).collect();
    let bounded = release_commit_message(
        "fix(sdk): update generated SDKs",
        &SpecDiff {
            bump: "patch".into(),
            changelog: String::new(),
            entries: oversized,
        },
    );
    assert!(bounded.len() < 20_200);
    assert_eq!(
        bounded.matches("BEGIN_NESTED_COMMIT").count(),
        bounded.matches("END_NESTED_COMMIT").count()
    );

    let root = tempfile::tempdir().unwrap();
    git_init(root.path());
    fs::write(root.path().join("sdk.txt"), "generated").unwrap();
    captured(root.path(), "git", &["add", "sdk.txt"]).unwrap();
    let temporary = tempfile::tempdir().unwrap();
    commit_from_file(root.path(), &temporary.path().join("message.txt"), &message).unwrap();
    assert_eq!(
        captured(root.path(), "git", &["log", "-1", "--format=%B"])
            .unwrap()
            .trim(),
        message.trim()
    );
    assert!(
        captured(root.path(), "git", &["status", "--porcelain"])
            .unwrap()
            .trim()
            .is_empty()
    );
}
