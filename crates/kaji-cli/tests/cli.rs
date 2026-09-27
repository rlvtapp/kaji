use std::fs;
use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_kaji"))
}

#[test]
fn help_version_and_language_discovery_work_without_a_compiler() {
    for args in [&["--help"][..], &["--version"][..], &["languages"][..]] {
        let output = cli().args(args).output().unwrap();
        assert!(output.status.success());
        assert!(!output.stdout.is_empty());
    }
}

#[test]
fn bad_arguments_have_a_distinct_exit_code() {
    let output = cli()
        .args(["generate", "--language", "unknown"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown target"));
}

#[test]
fn generates_all_languages_from_artifacts_and_preserves_custom_files() {
    let working = tempfile::tempdir().unwrap();
    let artifacts = working.path().join("artifacts with spaces");
    let output = working.path().join("sdk with spaces");
    fs::create_dir_all(artifacts.join("operations")).unwrap();
    fs::write(artifacts.join("schemas.json"), r#"{"schemas":[]}"#).unwrap();
    fs::write(artifacts.join("security-schemes.json"), r#"{"schemes":[]}"#).unwrap();
    fs::write(
        artifacts.join("operations.json"),
        r#"{"GET /contacts":"get.json"}"#,
    )
    .unwrap();
    fs::write(
        artifacts.join("operations-order.json"),
        r#"["GET /contacts"]"#,
    )
    .unwrap();
    fs::write(
        artifacts.join("operations/get.json"),
        r#"{"path":"/contacts","method":"GET","operation_id":"listContacts","responses":[]}"#,
    )
    .unwrap();
    let run = || {
        cli()
            .arg("generate")
            .arg("--artifacts")
            .arg(&artifacts)
            .arg("--output")
            .arg(&output)
            .args(["--language", "all", "--name", "CLI Contract", "--jobs", "2"])
            .output()
            .unwrap()
    };
    let result = run();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    for target in [
        "go",
        "rust",
        "typescript",
        "python",
        "php",
        "java",
        "dotnet",
        "elixir",
    ] {
        assert!(output.join(target).is_dir(), "missing {target}");
    }
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("Generation completed"));
    assert!(stderr.contains("Files"));
    assert!(
        !stderr.contains("\u{1b}["),
        "captured output must stay plain"
    );
    let custom = output.join("typescript/custom/index.ts");
    assert!(custom.is_file());
    fs::write(&custom, "// user customization\n").unwrap();
    let result = run();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        fs::read_to_string(custom).unwrap(),
        "// user customization\n"
    );

    let axios_output = working.path().join("axios sdk");
    let axios = cli()
        .arg("generate")
        .arg("--artifacts")
        .arg(&artifacts)
        .arg("--output")
        .arg(&axios_output)
        .args([
            "--language",
            "typescript",
            "--typescript-transport",
            "axios",
        ])
        .output()
        .unwrap();
    assert!(
        axios.status.success(),
        "{}",
        String::from_utf8_lossy(&axios.stderr)
    );
    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(axios_output.join("typescript/package.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["peerDependencies"]["axios"], "^1.0.0");
}

#[cfg(unix)]
#[test]
fn compiler_failure_is_reported_without_writing_output() {
    use std::os::unix::fs::PermissionsExt;
    let working = tempfile::tempdir().unwrap();
    let helper = working.path().join("failing compiler");
    let source = working.path().join("source.yaml");
    let output = working.path().join("output");
    fs::write(&source, "openapi: 3.0.3\n").unwrap();
    fs::write(&helper, "#!/bin/sh\nexit 42\n").unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
    let result = cli()
        .arg("generate")
        .arg(source)
        .arg("--output")
        .arg(&output)
        .args(["--language", "go", "--openapi-compiler"])
        .arg(helper)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(!output.exists());
    assert!(String::from_utf8_lossy(&result.stderr).contains("OpenAPI compiler failed"));
}

#[test]
fn json_config_generates_sdks_and_all_selected_artifacts() {
    let working = tempfile::tempdir().unwrap();
    let artifacts = working.path().join("artifacts");
    fs::create_dir_all(artifacts.join("operations")).unwrap();
    fs::write(artifacts.join("schemas.json"), r#"{"schemas":[]}"#).unwrap();
    fs::write(artifacts.join("security-schemes.json"), r#"{"schemes":[]}"#).unwrap();
    fs::write(
        artifacts.join("operations.json"),
        r#"{"GET /contacts":"get.json"}"#,
    )
    .unwrap();
    fs::write(
        artifacts.join("operations-order.json"),
        r#"["GET /contacts"]"#,
    )
    .unwrap();
    fs::write(
        artifacts.join("operations/get.json"),
        r#"{"path":"/contacts","method":"GET","operation_id":"listContacts","responses":[]}"#,
    )
    .unwrap();
    let config = working.path().join("kaji.json");
    fs::write(
        &config,
        r#"{
  "openapi": { "artifacts": "artifacts", "name": "Contacts", "version": "1.2.3" },
  "output": { "path": "generated" },
  "packages": [
    {
      "language": "typescript",
      "path": "web",
      "name": "@acme/contacts",
      "plugins": [
        { "name": "sdk", "transport": "fetch", "client_name": "Contacts" },
        { "name": "zod" },
        { "name": "tanstack-react-query" },
        { "name": "tanstack-vue-query" },
        { "name": "swr" },
        { "name": "faker" },
        { "name": "msw" },
        { "name": "cypress" }
      ]
    },
    { "language": "go", "path": "go", "plugins": [{ "name": "sdk", "jobs": 2 }] },
    {
      "language": "artifacts",
      "path": "docs",
      "plugins": [
        { "name": "redoc", "openapi_spec": "../openapi.yaml" },
        { "name": "mcp" }
      ]
    }
  ]
}"#,
    )
    .unwrap();

    let result = cli()
        .args(["generate", "--config"])
        .arg(&config)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let output = working.path().join("generated");
    for file in [
        "web/package.json",
        "web/zod.ts",
        "web/react-query.ts",
        "web/vue-query.ts",
        "web/swr.ts",
        "web/faker.ts",
        "web/msw.ts",
        "web/api.cy.ts",
        "go/go.mod",
        "docs/redoc.html",
        "docs/redocly.yaml",
        "docs/tools.json",
    ] {
        assert!(output.join(file).is_file(), "missing {file}");
    }
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output.join("web/package.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["dependencies"]["zod"], "^3.0.0", "{manifest}");
    assert_eq!(manifest["dependencies"]["@tanstack/react-query"], "^5.0.0");
    assert_eq!(manifest["dependencies"]["msw"], "^2.0.0");
}

#[test]
fn init_writes_a_json_recipe_without_overwriting_existing_work() {
    let working = tempfile::tempdir().unwrap();
    let config = working.path().join("nested/kaji.json");
    let first = cli()
        .args(["init", "--config"])
        .arg(&config)
        .args(["--input", "contract/openapi.json", "--output", "output"])
        .output()
        .unwrap();
    assert!(first.status.success());
    let document: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&config).unwrap()).unwrap();
    assert_eq!(
        document["$schema"],
        "https://raw.githubusercontent.com/rlvtapp/kaji/main/schemas/v1/kaji.schema.json"
    );
    assert_eq!(document["openapi"]["input"], "contract/openapi.json");
    assert_eq!(document["packages"][0]["plugins"][1]["name"], "zod");
    assert_eq!(
        document["packages"][0]["plugins"][2]["name"],
        "tanstack-react-query"
    );
    let second = cli()
        .args(["init", "--config"])
        .arg(&config)
        .output()
        .unwrap();
    assert_eq!(second.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&second.stderr).contains("refusing to overwrite"));
}

#[test]
fn published_config_schema_is_valid_json_schema_document() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../schemas/v1/kaji.schema.json")).unwrap();
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert!(schema["properties"]["packages"].is_object());
    assert!(schema["$defs"]["plugin"].is_object());
}
