use serde_json::json;
use std::{fs, process::Command};
fn checked(command: &mut Command) -> std::process::Output {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}
#[test]
#[ignore = "requires bundled Go compiler, TypeScript compiler and generated Rust dependencies"]
fn native_openapi32_query_compiles_and_executes_typed_sdk_bytes() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("api.json"),
        include_str!("fixtures/openapi32-query.json"),
    )
    .unwrap();
    fs::write(
        temp.path().join("poolster.json"),
        serde_json::to_vec(&json!({
            "openapi": {
                "input": "api.json",
                "name": "Query",
                "version": "1"
            },
            "output": {
                "path": "generated"
            },
            "packages": [
                {
                    "language": "typescript",
                    "path": "typescript",
                    "name": "query-sdk",
                    "plugins": [
                        {
                            "name": "sdk",
                            "transport": "fetch"
                        },
                        {
                            "name": "operation-tests"
                        }
                    ]
                },
                {
                    "language": "rust",
                    "path": "rust",
                    "name": "query-sdk",
                    "plugins": [
                        {
                            "name": "sdk"
                        },
                        {
                            "name": "operation-tests"
                        }
                    ]
                },
                {
                    "language": "ruby",
                    "path": "ruby",
                    "name": "query-sdk",
                    "plugins": [
                        {
                            "name": "sdk"
                        },
                        {
                            "name": "operation-tests"
                        }
                    ]
                },
                {
                    "language": "elixir",
                    "path": "elixir",
                    "name": "query-sdk",
                    "plugins": [
                        {
                            "name": "sdk"
                        },
                        {
                            "name": "operation-tests"
                        }
                    ]
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    checked(
        Command::new(env!("CARGO_BIN_EXE_kaji"))
            .args(["generate", "--config"])
            .arg(temp.path().join("poolster.json")),
    );
    let rust = temp.path().join("generated/rust");
    let source = fs::read_to_string(rust.join("src/client/operations/chunk_0001.rs")).unwrap();
    assert!(source.contains("QUERY"));
    assert!(
        source.contains("COPY"),
        "custom methods must retain their wire token"
    );
    assert!(
        source.contains("retry_allowed = true") || source.contains("retry_allowed: true"),
        "QUERY must remain replayable"
    );
    assert!(
        source.contains("retry_allowed = false") || source.contains("retry_allowed: false"),
        "POST must remain guarded"
    );
    let mut cargo = Command::new("cargo");
    if std::env::var("KAJI_RUNTIME_OFFLINE").as_deref() == Ok("1") {
        cargo.arg("--offline");
    }
    cargo.args(["test", "--lib"]).current_dir(&rust).env(
        "CARGO_TARGET_DIR",
        std::env::var_os("KAJI_RUNTIME_RUST_TARGET")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("kaji-runtime-contract-rust-target")),
    );
    let output = checked(&mut cargo);
    assert!(String::from_utf8_lossy(&output.stdout).contains("4 passed"));
    let ts = temp.path().join("generated/typescript");
    let compiler =
        std::env::var_os("KAJI_TSC_JS").expect("KAJI_TSC_JS must identify TypeScript's tsc.js");
    checked(
        Command::new("node")
            .arg(compiler)
            .args(["-p", "tsconfig.json"])
            .current_dir(&ts),
    );
    checked(
        Command::new("node")
            .arg("dist/tests/operation-tests.js")
            .current_dir(&ts),
    );
    checked(
        Command::new("ruby")
            .args(["-Ilib", "test/operation_tests.rb"])
            .current_dir(temp.path().join("generated/ruby")),
    );
}
