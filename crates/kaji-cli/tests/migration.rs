use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

fn run(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_kaji"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn fixture(root: &Path) {
    fs::write(root.join("openapi.yaml"), "openapi: 3.0.3\ninfo: {title: Migration, version: '1'}\npaths:\n  /users:\n    get:\n      operationId: listUsers\n      responses:\n        '200':\n          description: ok\n          content:\n            application/json:\n              schema: {$ref: './models.yaml#/Users'}\n").unwrap();
    fs::write(
        root.join("models.yaml"),
        "Users:\n  type: array\n  items: {type: string}\n",
    )
    .unwrap();
}
#[test]
#[ignore = "requires bundled Go compiler"]
fn imports_three_vendors_and_bundles_references_without_editing_sources() {
    for (filename, config) in [
        (
            "stainless.yml",
            "targets:\n  python:\n    package_name: migration_sdk\nresources:\n  users:\n    methods:\n      list: get /users\n",
        ),
        (
            "fern/generators.yml",
            "api:\n  specs:\n    - openapi: ../openapi.yaml\ngroups:\n  python:\n    generators:\n      - name: fern-python-sdk\n        version: '9.9.9'\n        output:\n          location: pypi\n          package-name: migration_sdk\n          token: 'DO-NOT-COPY-SECRET'\n",
        ),
        (
            ".speakeasy/workflow.yaml",
            "workflowVersion: 1.0.0\nsources:\n  api:\n    inputs:\n      - location: ./openapi.yaml\ntargets:\n  python:\n    target: python\n    source: api\n",
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        fixture(root.path());
        let path = root.path().join(filename);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, config).unwrap();
        let original = fs::read(root.path().join("openapi.yaml")).unwrap();
        let result = run(root.path(), &["migrate", ".", "--output", "converted"]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            fs::read(root.path().join("openapi.yaml")).unwrap(),
            original
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), config);
        let native: Value =
            serde_json::from_slice(&fs::read(root.path().join("converted/kaji.json")).unwrap())
                .unwrap();
        assert_eq!(native["packages"][0]["language"], "python");
        assert_eq!(native["openapi"]["version"], "0.1.0"); // Generator version is not an SDK version.
        let bundled = fs::read_to_string(root.path().join("converted/openapi.json")).unwrap();
        assert!(!bundled.contains("./models.yaml"));
        let report =
            fs::read_to_string(root.path().join("converted/migration-report.json")).unwrap();
        assert!(!report.contains("DO-NOT-COPY-SECRET"));
        let generated = run(
            root.path(),
            &["generate", "--config", "converted/kaji.json"],
        );
        assert!(
            generated.status.success(),
            "{}",
            String::from_utf8_lossy(&generated.stderr)
        );
        assert!(root.path().join("converted/generated").is_dir());
        let syntax = Command::new("python3")
            .env("PYTHONPYCACHEPREFIX", root.path().join("python-cache"))
            .args(["-m", "compileall", "-q"])
            .arg(root.path().join("converted/generated"))
            .output()
            .unwrap();
        assert!(
            syntax.status.success(),
            "{}",
            format!(
                "{}{}",
                String::from_utf8_lossy(&syntax.stdout),
                String::from_utf8_lossy(&syntax.stderr)
            )
        );
        let again = run(root.path(), &["migrate", ".", "--output", "converted"]);
        assert!(!again.status.success());
    }
}
#[test]
#[ignore = "requires bundled Go compiler"]
fn strict_rejects_unsupported_settings_before_writing_output() {
    let root = tempfile::tempdir().unwrap();
    fixture(root.path());
    fs::write(
        root.path().join("stainless.yml"),
        "targets: {python: {}}\npagination: {custom: {type: custom}}\n",
    )
    .unwrap();
    let result = run(
        root.path(),
        &["migrate", ".", "--strict", "--output", "converted"],
    );
    assert!(!result.status.success());
    assert!(!root.path().join("converted").exists());
    assert!(String::from_utf8_lossy(&result.stderr).contains("pagination"));
}
#[test]
#[ignore = "requires bundled Go compiler"]
fn direct_vendor_generation_and_automatic_project_detection() {
    let root = tempfile::tempdir().unwrap();
    fixture(root.path());
    fs::write(root.path().join("stainless.yml"),"targets: {python: {package_name: migration_sdk}}\nresources: {users: {methods: {list: 'get /users'}}}\n").unwrap();
    for args in [
        vec!["generate", "--config", "stainless.yml"],
        vec!["generate"],
    ] {
        let result = run(root.path(), &args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(root.path().join("kaji-generated").is_dir());
    }
    assert!(!root.path().join("kaji.json").exists());
    assert!(!root.path().join("kaji-migration").exists());
}
#[test]
#[ignore = "requires bundled Go compiler"]
fn annotations_are_recognized_during_normal_generation() {
    let root = tempfile::tempdir().unwrap();
    let spec = json!({"openapi":"3.0.3","info":{"title":"Migration","version":"1"},"paths":{"/users":{"get":{"operationId":"old","x-stainless-method":"users.list","responses":{"200":{"description":"ok","content":{"application/json":{"schema":{"type":"string"}}}}}}},"/hidden":{"get":{"operationId":"hidden","x-speakeasy-ignore":true,"responses":{"200":{"description":"ok"}}}}}});
    fs::write(
        root.path().join("openapi.json"),
        serde_json::to_vec(&spec).unwrap(),
    )
    .unwrap();
    let result = run(
        root.path(),
        &[
            "generate",
            "openapi.json",
            "--language",
            "python",
            "--output",
            "sdk",
        ],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    fn collect(path: &Path, output: &mut String) {
        for entry in fs::read_dir(path).unwrap().flatten() {
            if entry.path().is_dir() {
                collect(&entry.path(), output);
            } else if entry.path().extension().is_some_and(|e| e == "py") {
                output.push_str(&fs::read_to_string(entry.path()).unwrap());
            }
        }
    }
    let mut rendered = String::new();
    collect(&root.path().join("sdk"), &mut rendered);
    assert!(rendered.contains("users_list"));
    assert!(!rendered.contains("def hidden("));
}

#[test]
fn ambiguous_project_selection_fails_without_writing_files() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("stainless.yml"), "targets: {python: {}}\n").unwrap();
    fs::create_dir(root.path().join("fern")).unwrap();
    fs::write(root.path().join("fern/generators.yml"), "groups: {}\n").unwrap();
    let result = run(root.path(), &["migrate", "."]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("found 2"));
    assert!(!root.path().join("kaji-migration").exists());
}

#[test]
#[ignore = "requires bundled Go compiler"]
fn multi_target_migrations_report_custom_hooks_without_copying_credentials() {
    for (filename, config, diagnostic) in [
        (
            "stainless.yml",
            "targets:\n  python: {package_name: migrated_python, hooks: {token: DO-NOT-COPY-SECRET}}\n  go: {package_name: migrated_go}\nresources:\n  users:\n    methods:\n      list: get /users\nhooks: {command: 'touch DO-NOT-EXECUTE'}\n",
            "hooks",
        ),
        (
            "fern/generators.yml",
            "api:\n  specs:\n    - openapi: ../openapi.yaml\ngroups:\n  sdk:\n    generators:\n      - name: fern-python-sdk\n        output: {package-name: migrated_python, token: DO-NOT-COPY-SECRET}\n      - name: fern-go-sdk\n      - custom: {token: DO-NOT-COPY-SECRET}\n",
            "custom generator",
        ),
        (
            ".speakeasy/workflow.yaml",
            "workflowVersion: 1.0.0\nsources:\n  api:\n    inputs:\n      - location: ./openapi.yaml\ntargets:\n  python:\n    target: python\n    source: api\n    hooks: {token: DO-NOT-COPY-SECRET}\n  go:\n    target: go\n    source: api\n",
            "hooks",
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        fixture(root.path());
        let path = root.path().join(filename);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, config).unwrap();
        let before = fs::read(root.path().join("openapi.yaml")).unwrap();
        let result = run(root.path(), &["migrate", ".", "--output", "converted"]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let recipe = fs::read_to_string(root.path().join("converted/kaji.json")).unwrap();
        let native: Value = serde_json::from_str(&recipe).unwrap();
        assert_eq!(native["packages"].as_array().unwrap().len(), 2);
        let report =
            fs::read_to_string(root.path().join("converted/migration-report.json")).unwrap();
        assert!(report.contains(diagnostic));
        for output in [&report, &recipe] {
            assert!(!output.contains("DO-NOT-COPY-SECRET"));
        }
        assert!(!root.path().join("DO-NOT-EXECUTE").exists());
        assert_eq!(fs::read_to_string(&path).unwrap(), config);
        assert_eq!(fs::read(root.path().join("openapi.yaml")).unwrap(), before);
        let strict = run(
            root.path(),
            &["migrate", ".", "--strict", "--output", "strict-output"],
        );
        assert!(!strict.status.success());
        assert!(!root.path().join("strict-output").exists());
    }
}
