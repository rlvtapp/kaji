use std::fs;
use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_poolster"))
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
        "csharp",
        "elixir",
        "ruby",
        "swift",
    ] {
        assert!(output.join(target).is_dir(), "missing {target}");
    }
    let lock: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(output.join(".poolster/generation.lock.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(lock["version"], 1);
    assert_eq!(lock["generator"]["name"], "poolster");
    assert_eq!(lock["input"]["kind"], "artifacts");
    assert_eq!(
        lock["api"]["operations"],
        serde_json::json!(["GET /contacts"])
    );
    assert!(
        lock["input"]["artifacts_sha256"]
            .as_str()
            .is_some_and(|hash| hash.len() == 64)
    );
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

    let check = cli()
        .arg("generate")
        .arg("--artifacts")
        .arg(&artifacts)
        .arg("--output")
        .arg(&output)
        .args([
            "--language",
            "all",
            "--name",
            "CLI Contract",
            "--jobs",
            "2",
            "--check",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(
        report,
        serde_json::json!({"added":[],"modified":[],"removed":[]})
    );

    let update = cli()
        .args(["update", "--output"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        update.status.success(),
        "{}",
        String::from_utf8_lossy(&update.stderr)
    );
    assert!(String::from_utf8_lossy(&update.stdout).contains("1 unchanged"));

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
    let config = working.path().join("poolster.json");
    fs::write(
        &config,
        r#"{
  "openapi": {
    "artifacts": "artifacts",
    "name": "Contacts",
    "version": "1.2.3",
    "paths": { "include": ["/contacts*"] }
  },
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
    { "language": "csharp", "path": "csharp", "name": "acme-contacts", "plugins": [{ "name": "sdk" }] },
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
        "csharp/AcmeContacts.csproj",
        "docs/redoc.html",
        "docs/redocly.yaml",
        "docs/tools.json",
    ] {
        assert!(output.join(file).is_file(), "missing {file}");
    }
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output.join("web/package.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["dependencies"]["zod"], "^4.0.0", "{manifest}");
    assert_eq!(
        manifest["peerDependencies"]["@tanstack/react-query"],
        "^5.0.0"
    );
    assert_eq!(manifest["dependencies"]["msw"], "^2.0.0");
    assert_eq!(manifest["devDependencies"]["cypress"], "^15.0.0");
    let lock: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(output.join(".poolster/generation.lock.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(lock["paths"]["include"], serde_json::json!(["/contacts*"]));
    assert!(lock["input"]["config_sha256"].is_string());
    let check = cli()
        .args(["generate", "--config"])
        .arg(&config)
        .args(["--check", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    let changes: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(
        changes,
        serde_json::json!({"added":[], "modified":[], "removed":[]})
    );
    let owned = output.join("web/zod.ts");
    fs::write(&owned, "// local edit\n").unwrap();
    let drift = cli()
        .args(["generate", "--config"])
        .arg(&config)
        .args(["--check", "--json"])
        .output()
        .unwrap();
    assert!(!drift.status.success());
    let changes: serde_json::Value = serde_json::from_slice(&drift.stdout).unwrap();
    assert!(changes["modified"].as_array().unwrap().iter().any(|path| {
        path.as_str()
            .is_some_and(|path| std::path::Path::new(path).ends_with("web/zod.ts"))
    }));
    assert_eq!(fs::read_to_string(&owned).unwrap(), "// local edit\n");
}

#[test]
fn init_writes_a_json_recipe_without_overwriting_existing_work() {
    let working = tempfile::tempdir().unwrap();
    let config = working.path().join("nested/poolster.json");
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
        "https://raw.githubusercontent.com/rlvtapp/kaji/main/schemas/v1/poolster.schema.json"
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
        serde_json::from_str(include_str!("../../../schemas/v1/poolster.schema.json")).unwrap();
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert!(schema["properties"]["packages"].is_object());
    assert!(schema["$defs"]["plugin"].is_object());
}

#[test]
fn published_check_baseline_schema_is_valid_json_schema_document() {
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/v1/check-baseline.schema.json"
    ))
    .unwrap();
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(schema["properties"]["version"]["const"], 1);
    assert!(schema["properties"]["diagnostics"].is_object());
}

#[test]
fn named_provider_recipe_and_release_version_survive_regeneration() {
    let working = tempfile::tempdir().unwrap();
    let artifacts = working.path().join("artifacts");
    fs::create_dir_all(artifacts.join("operations")).unwrap();
    fs::write(artifacts.join("schemas.json"), r#"{"schemas":[]}"#).unwrap();
    fs::write(artifacts.join("security-schemes.json"), r#"{"schemes":[]}"#).unwrap();
    fs::write(
        artifacts.join("operations.json"),
        r#"{"GET /items":"get.json"}"#,
    )
    .unwrap();
    fs::write(
        artifacts.join("operations/get.json"),
        r#"{"path":"/items","method":"GET","operation_id":"listItems","responses":[]}"#,
    )
    .unwrap();
    fs::write(artifacts.join("operations-order.json"), r#"["GET /items"]"#).unwrap();
    let config = working.path().join("poolster.json");
    let mut recipe = serde_json::json!({
        "openapi": {"artifacts":"artifacts", "name":"Inventory", "version":"1.0.0"},
        "output": {"path":"generated"},
        "packages": [{"language":"typescript", "path":"web", "name":"@acme/inventory",
            "release": {"build":[{"program":"npm","args":["run","build"]}], "test":[{"program":"npm","args":["run","build"]}]},
            "plugins":[
                {"name":"models", "id":"domain", "output":"types"},
                {"name":"transport", "id":"wire", "output":"network"},
                {"name":"operations", "id":"api", "output":"calls", "uses":{"models":"domain","transport":"wire"}},
                {"name":"client", "uses":{"models":"domain","transport":"wire","operations":"api"}},
                {"name":"tanstack-react-query", "uses":{"operations":"api"}}
            ]}]
    });
    fs::write(&config, serde_json::to_vec_pretty(&recipe).unwrap()).unwrap();
    let generated = cli()
        .args(["generate", "--config"])
        .arg(&config)
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let root = working.path().join("generated/web");
    assert!(root.join("network.ts").exists());
    assert!(
        fs::read_to_string(root.join("calls/listItems.ts"))
            .unwrap()
            .contains("network")
    );
    let metadata_path = root.join(".poolster/package.json");
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&fs::read(&metadata_path).unwrap()).unwrap();
    metadata["version"] = "2.3.4".into();
    fs::write(
        &metadata_path,
        serde_json::to_vec_pretty(&metadata).unwrap(),
    )
    .unwrap();
    let regenerated = cli()
        .args(["generate", "--config"])
        .arg(&config)
        .output()
        .unwrap();
    assert!(
        regenerated.status.success(),
        "{}",
        String::from_utf8_lossy(&regenerated.stderr)
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("package.json")).unwrap()).unwrap();
    assert_eq!(manifest["version"], "2.3.4");
    let check = cli()
        .args(["generate", "--config"])
        .arg(&config)
        .arg("--check")
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    // A binding to a nonexistent instance must fail before output is rewritten.
    recipe["packages"][0]["plugins"][2]["uses"]["transport"] = "missing".into();
    fs::write(&config, serde_json::to_vec_pretty(&recipe).unwrap()).unwrap();
    let bad = cli()
        .args(["generate", "--config"])
        .arg(&config)
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("no transport provider"));
    assert_eq!(
        fs::read(root.join("package.json")).unwrap(),
        serde_json::to_vec_pretty(&manifest)
            .unwrap()
            .into_iter()
            .chain(*b"\n")
            .collect::<Vec<_>>()
    );
}

#[test]
fn source_customizations_are_package_scoped_survive_regeneration_and_detect_drift() {
    let working = tempfile::tempdir().unwrap();
    let artifacts = working.path().join("artifacts");
    fs::create_dir_all(&artifacts).unwrap();
    for (file, contents) in [
        ("schemas.json", r#"{"schemas":[]}"#),
        ("security-schemes.json", r#"{"schemes":[]}"#),
        ("operations.json", "{}"),
        ("operations-order.json", "[]"),
    ] {
        fs::write(artifacts.join(file), contents).unwrap();
    }
    fs::create_dir_all(artifacts.join("operations")).unwrap();
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
    fs::create_dir_all(working.path().join("overrides")).unwrap();
    let source = working.path().join("overrides/extra.ts");
    fs::write(&source, "export const customBehavior = 1;\n").unwrap();
    let config = working.path().join("poolster.json");
    let mut document = serde_json::json!({
        "openapi":{"artifacts":"artifacts","name":"Example","version":"1.0.0"},
        "output":{"path":"generated"},
        "packages":[
            {"language":"typescript","path":"web","plugins":[{"name":"sdk"}],
             "customizations":[{"mode":"add","path":"extra.ts","source":"overrides/extra.ts"}]},
            {"language":"go","path":"go","plugins":[{"name":"sdk"}]}
        ]
    });
    fs::write(&config, serde_json::to_string(&document).unwrap()).unwrap();
    let run = |check: bool| {
        let mut cmd = cli();
        cmd.args(["generate", "--config"]).arg(&config);
        if check {
            cmd.arg("--check");
        }
        cmd.output().unwrap()
    };
    for _ in 0..2 {
        let result = run(false);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let output = working.path().join("generated/web/extra.ts");
    assert_eq!(
        fs::read_to_string(&output).unwrap(),
        "export const customBehavior = 1;\n"
    );
    assert!(!working.path().join("generated/go/extra.ts").exists());
    assert!(run(true).status.success());
    fs::write(&source, "export const customBehavior = 2;\n").unwrap();
    assert!(!run(true).status.success());
    assert!(fs::read_to_string(&output).unwrap().contains("= 1"));
    assert!(run(false).status.success());
    let before = fs::read_to_string(&output).unwrap();
    document["packages"][0]["customizations"] = serde_json::json!([
        {"mode":"replace","path":"extra.ts","source":"overrides/extra.ts"}
    ]);
    fs::write(&config, serde_json::to_string(&document).unwrap()).unwrap();
    let result = run(false);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("target missing"));
    assert_eq!(fs::read_to_string(output).unwrap(), before);
}

#[test]
fn bundled_author_middleware_is_scoped_and_tracks_source_drift() {
    let working = tempfile::tempdir().unwrap();
    let artifacts = working.path().join("artifacts");
    fs::create_dir_all(artifacts.join("operations")).unwrap();
    for (file, text) in [
        ("schemas.json", r#"{"schemas":[]}"#),
        ("security-schemes.json", r#"{"schemes":[]}"#),
        ("operations.json", r#"{"GET /contacts":"get.json"}"#),
        ("operations-order.json", r#"["GET /contacts"]"#),
        (
            "operations/get.json",
            r#"{"path":"/contacts","method":"GET","operation_id":"listContacts","responses":[]}"#,
        ),
    ] {
        fs::write(artifacts.join(file), text).unwrap();
    }
    let source = working.path().join("policy.ts");
    fs::write(
        &source,
        "export const authorPolicy = async (request: any, next: any) => next(request);\n",
    )
    .unwrap();
    let config = working.path().join("poolster.json");
    let mut document = serde_json::json!({
        "openapi":{"artifacts":"artifacts","name":"Example","version":"1.0.0"},
        "output":{"path":"generated"},
        "packages":[
            {"language":"typescript","path":"web","plugins":[{"name":"sdk"}],
             "middleware":[{"source":"policy.ts","path":"middleware/policy.ts","symbol":"authorPolicy"}]},
            {"language":"go","path":"go","plugins":[{"name":"sdk"}]}
        ]
    });
    fs::write(&config, serde_json::to_string(&document).unwrap()).unwrap();
    let run = |check: bool| {
        let mut command = cli();
        command.args(["generate", "--config"]).arg(&config);
        if check {
            command.arg("--check");
        }
        command.output().unwrap()
    };
    let result = run(false);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let runtime = working.path().join("generated/web/.poolster/client.ts");
    assert!(
        fs::read_to_string(&runtime)
            .unwrap()
            .contains("[kajiBundledMiddleware0, ...(config.middleware ?? [])]")
    );
    let bundled = working.path().join("generated/web/middleware/policy.ts");
    assert_eq!(
        fs::read_to_string(&bundled).unwrap(),
        fs::read_to_string(&source).unwrap()
    );
    assert!(
        !working
            .path()
            .join("generated/go/middleware/policy.ts")
            .exists()
    );
    assert!(run(true).status.success());
    fs::write(&source,"export const authorPolicy = async (request: any, next: any) => next({ ...request, query: { tenant: 'author' } });\n").unwrap();
    assert!(!run(true).status.success());
    assert!(!fs::read_to_string(&bundled).unwrap().contains("tenant"));
    assert!(run(false).status.success());
    assert!(fs::read_to_string(&bundled).unwrap().contains("tenant"));
    document["packages"][0]["middleware"] = serde_json::json!([]);
    fs::write(&config, serde_json::to_string(&document).unwrap()).unwrap();
    assert!(run(false).status.success());
    assert!(!bundled.exists());
    assert!(
        !fs::read_to_string(runtime)
            .unwrap()
            .contains("kajiBundledMiddleware0")
    );
}

#[test]
fn postman_and_typed_terraform_share_a_recipe_and_preserve_environment() {
    use serde_json::json;
    let working = tempfile::tempdir().unwrap();
    let artifacts = working.path().join("artifacts");
    fs::create_dir_all(artifacts.join("operations")).unwrap();
    let body = json!({"type":"object", "additionalProperties":false, "required":["name"], "properties":{"name":{"type":"string"}}});
    let response = json!({"type":"object", "additionalProperties":false, "required":["id","name"], "properties":{"id":{"type":"string","readOnly":true},"name":{"type":"string"}}});
    fs::write(artifacts.join("schemas.json"), r#"{"schemas":[]}"#).unwrap();
    fs::write(artifacts.join("security-schemes.json"), r#"{"schemes":[]}"#).unwrap();
    let mut index = serde_json::Map::new();
    let mut order = Vec::new();
    for (id, method, path) in [
        ("createWidget", "POST", "/widgets"),
        ("getWidget", "GET", "/widgets/{id}"),
        ("updateWidget", "PATCH", "/widgets/{id}"),
        ("deleteWidget", "DELETE", "/widgets/{id}"),
    ] {
        let key = format!("{method} {path}");
        index.insert(key.clone(), json!(format!("{id}.json")));
        order.push(key);
        let mut operation = json!({"operation_id":id,"method":method,"path":path,"tags":["Widgets"],"servers":[{"url":"https://example.test"}],"responses":[]});
        if method == "POST" || method == "PATCH" {
            operation["request_body"] = json!({"required":true,"media_types":[{"content_type":"application/json","schema_definition":body}]});
        }
        if method != "POST" {
            operation["parameters"] =
                json!([{"name":"id","in":"path","required":true,"schema":{"type":"string"}}]);
        }
        if method != "DELETE" {
            operation["responses"] = json!([{"code":"200","content_type":"application/json","schema_definition":response}]);
        }
        fs::write(
            artifacts.join("operations").join(format!("{id}.json")),
            serde_json::to_vec(&operation).unwrap(),
        )
        .unwrap();
    }
    fs::write(
        artifacts.join("operations.json"),
        serde_json::to_vec(&index).unwrap(),
    )
    .unwrap();
    fs::write(
        artifacts.join("operations-order.json"),
        serde_json::to_vec(&order).unwrap(),
    )
    .unwrap();
    let config = working.path().join("poolster.json");
    fs::write(&config, serde_json::to_vec(&json!({"openapi":{"artifacts":"artifacts","name":"Widgets"},"output":{"path":"generated"},"packages":[{"language":"postman","path":"postman","plugins":[{"name":"collection","strict":true,"split_by_group":true},{"name":"environment"}]},{"language":"terraform","path":"terraform","plugins":[{"name":"provider","provider_name":"widgets","infer":false,"resources":[{"name":"widget","create":"createWidget","read":"getWidget","update":"updateWidget","delete":"deleteWidget"}]}]}]})).unwrap()).unwrap();
    let run = || {
        cli()
            .args(["generate", "--config"])
            .arg(&config)
            .output()
            .unwrap()
    };
    let result = run();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let output = working.path().join("generated");
    let collection: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("postman/collection.json")).unwrap()).unwrap();
    assert!(
        collection["info"]["schema"]
            .as_str()
            .unwrap()
            .contains("v2.1.0")
    );
    let catalog: serde_json::Value = serde_json::from_slice(
        &fs::read(output.join("terraform/.poolster/terraform-plan.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(catalog["resources"][0]["name"], "widget");
    assert!(output.join("terraform/go.mod").is_file());
    let split: serde_json::Value = serde_json::from_slice(
        &fs::read(output.join("postman/collections/group-0000.json")).unwrap(),
    )
    .unwrap();
    assert!(split["item"][0]["item"].is_array());
    assert_eq!(split["variable"], collection["variable"]);
    let environment = output.join("postman/environment.json");
    fs::write(&environment, "customer-owned environment").unwrap();
    assert!(run().status.success());
    assert_eq!(
        fs::read_to_string(&environment).unwrap(),
        "customer-owned environment"
    );
    let check = cli()
        .args(["generate", "--config"])
        .arg(&config)
        .args(["--check", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&check.stdout).unwrap(),
        json!({"added":[],"modified":[],"removed":[]})
    );
}

#[test]
fn per_language_repository_setup_and_install_dry_run_are_local() {
    let working = tempfile::tempdir().unwrap();
    for language in ["typescript", "python"] {
        let metadata = working
            .path()
            .join(format!("generated/{language}/.poolster/package.json"));
        fs::create_dir_all(metadata.parent().unwrap()).unwrap();
        fs::write(metadata, serde_json::to_vec(&serde_json::json!({
            "language":language,"name":format!("demo-{language}"),"version":"1.0.0",
            "publisher":{"registry":if language == "python" { "pypi" } else { "npm" },"release_type":"simple"}
        })).unwrap()).unwrap();
    }
    let output = cli()
        .current_dir(working.path())
        .args([
            "sdk",
            "init",
            "--root",
            "generated",
            "--repository-pattern",
            "acme/api-{lang}",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for language in ["typescript", "python"] {
        let setup = format!(".poolster/sdk-repository-setup/acme/api-{language}");
        assert!(
            working
                .path()
                .join(&setup)
                .join(".github/workflows/poolster-sdk-release.yml")
                .is_file()
        );
        let output = cli()
            .current_dir(working.path())
            .args([
                "sdk",
                "install",
                "--setup",
                &setup,
                "--repository",
                &format!("acme/api-{language}"),
                "--dry-run",
            ])
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("Would install"));
    }
    let output = cli()
        .current_dir(working.path())
        .args([
            "sdk",
            "init",
            "--root",
            "generated",
            "--repository",
            "acme/all",
            "--repository-pattern",
            "acme/api-{lang}",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
}
