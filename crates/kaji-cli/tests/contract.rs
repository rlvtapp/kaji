use serde_json::Value;
use std::{
    path::Path,
    process::{Command, Output},
};
fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kaji"))
        .args(arguments)
        .output()
        .unwrap()
}
fn fixture(name: &str) -> std::path::PathBuf {
    let provider = match name {
        "events.yaml" => "asyncapi",
        "workflows.yaml" => "arazzo",
        "rpc/service.proto" => "protobuf",
        _ => panic!("unknown fixture {name}"),
    };
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../inputs/{provider}/tests/fixtures"))
        .join(name)
}

#[test]
fn lists_registered_providers_and_inspects_typed_graphql() {
    let plugins = run(&["contract", "plugins", "--format", "json"]);
    assert!(plugins.status.success());
    let plugins: Value = serde_json::from_slice(&plugins.stdout).unwrap();
    assert_eq!(plugins.as_array().unwrap().len(), 5);
    assert!(
        plugins
            .as_array()
            .unwrap()
            .iter()
            .any(|plugin| plugin["provider"] == "graphql.apollo")
    );
    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(source.path(), "type Query { hello: String }").unwrap();
    let inspected = run(&[
        "contract",
        "inspect",
        source.path().to_str().unwrap(),
        "--input-format",
        "graphql",
        "--provider",
        "graphql.apollo",
        "--format",
        "json",
    ]);
    assert!(
        inspected.status.success(),
        "{}",
        String::from_utf8_lossy(&inspected.stderr)
    );
    let report: Value = serde_json::from_slice(&inspected.stdout).unwrap();
    assert_eq!(report["provider"], "graphql.apollo");
    assert_eq!(report["summary"]["operations"][0]["kind"], "query");
}

#[test]
fn arazzo_reports_unloaded_sources_without_executing_steps() {
    let source = fixture("workflows.yaml");
    let inspected = run(&[
        "contract",
        "inspect",
        source.to_str().unwrap(),
        "--input-format",
        "arazzo",
        "--format",
        "json",
    ]);
    assert!(
        inspected.status.success(),
        "{}",
        String::from_utf8_lossy(&inspected.stderr)
    );
    let report: Value = serde_json::from_slice(&inspected.stdout).unwrap();
    assert_eq!(report["provider"], "arazzo.roas");
    assert_eq!(report["diagnostics"][0]["code"], "unresolved-source");
    assert_eq!(report["summary"]["operations"].as_array().unwrap().len(), 2);
}

#[test]
fn asyncapi_and_protobuf_use_their_registered_plugins() {
    for (file, format, provider) in [
        ("events.yaml", "asyncapi", "asyncapi.roas"),
        ("rpc/service.proto", "protobuf", "protobuf.protox"),
    ] {
        let source = fixture(file);
        let output = run(&[
            "contract",
            "inspect",
            source.to_str().unwrap(),
            "--input-format",
            format,
            "--format",
            "json",
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["provider"], provider);
        assert!(
            !report["summary"]["operations"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn invalid_contract_and_provider_selection_fail_without_json_success() {
    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(source.path(), "type Query { hello: Missing }").unwrap();
    let output = run(&[
        "contract",
        "inspect",
        source.path().to_str().unwrap(),
        "--input-format",
        "graphql",
        "--format",
        "json",
    ]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid GraphQL"));
    let output = run(&[
        "contract",
        "inspect",
        "unused",
        "--input-format",
        "graphql",
        "--provider",
        "protobuf.protox",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("reads protobuf"));
}

#[test]
fn argument_errors_do_not_emit_success_payloads() {
    for args in [
        vec!["contract"],
        vec!["contract", "unknown"],
        vec!["contract", "inspect"],
        vec!["contract", "inspect", "unused"],
        vec!["contract", "inspect", "unused", "--input-format"],
        vec!["contract", "inspect", "unused", "--provider"],
        vec!["contract", "plugins", "--format"],
        vec!["contract", "plugins", "--format", "xml"],
        vec!["contract", "plugins", "unused"],
        vec!["contract", "plugins", "--input-format", "graphql"],
        vec!["contract", "plugins", "--provider", "graphql.apollo"],
        vec!["contract", "inspect", "a", "b", "--input-format", "graphql"],
        vec!["contract", "inspect", "unused", "--wat"],
        vec![
            "contract",
            "inspect",
            "unused",
            "--input-format",
            "graphql",
            "--input-format",
            "graphql",
        ],
        vec![
            "contract",
            "inspect",
            "unused",
            "--provider",
            "graphql.apollo",
            "--provider",
            "graphql.apollo",
        ],
    ] {
        let output = run(&args);
        assert!(!output.status.success(), "accepted {args:?}");
        assert!(output.stdout.is_empty(), "success output for {args:?}");
        assert!(!output.stderr.is_empty(), "no diagnostic for {args:?}");
    }
}
#[test]
fn help_works_without_sources_or_loading() {
    for args in [
        vec!["contract", "--help"],
        vec!["contract", "-h"],
        vec!["contract", "inspect", "--help"],
        vec!["contract", "plugins", "--help"],
    ] {
        let output = run(&args);
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("Usage:"));
        assert!(output.stderr.is_empty());
    }
}
#[test]
fn protobuf_alias_routes_to_the_same_provider() {
    let source = fixture("rpc/service.proto");
    let output = run(&[
        "contract",
        "inspect",
        source.to_str().unwrap(),
        "--input-format",
        "proto",
        "--format",
        "json",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["provider"], "protobuf.protox");
    assert_eq!(report["summary"]["format"], "protobuf");
}
#[test]
fn human_inspection_sends_arazzo_diagnostics_to_stderr() {
    let source = fixture("workflows.yaml");
    let output = run(&[
        "contract",
        "inspect",
        source.to_str().unwrap(),
        "--input-format",
        "arazzo",
    ]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Provider: arazzo.roas"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unresolved-source"));
}
#[test]
fn paths_with_spaces_and_unicode_are_preserved_in_json() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("契約 with spaces.graphql");
    std::fs::write(&path, "type Query { hello: String }").unwrap();
    let output = run(&[
        "contract",
        "inspect",
        path.to_str().unwrap(),
        "--input-format",
        "graphql",
        "--format",
        "json",
    ]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["source"], path.to_str().unwrap());
    assert_eq!(report["diagnostics"], serde_json::json!([]));
}
#[test]
fn missing_source_reports_provider_and_path_without_success_output() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("missing.graphql");
    let output = run(&[
        "contract",
        "inspect",
        path.to_str().unwrap(),
        "--input-format",
        "graphql",
        "--format",
        "json",
    ]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains("graphql.apollo"));
    assert!(diagnostic.contains("missing.graphql"));
}
#[test]
fn unknown_format_and_provider_fail_before_reading_source() {
    for (option, value, expected) in [
        ("--input-format", "custom", "no input provider"),
        ("--provider", "graphql.missing", "unknown input provider"),
    ] {
        let mut args = vec![
            "contract",
            "inspect",
            "definitely-missing",
            "--format",
            "json",
        ];
        if option == "--provider" {
            args.extend(["--input-format", "graphql"]);
        }
        args.extend([option, value]);
        let output = run(&args);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains(expected));
    }
}
#[test]
fn provider_listing_is_stable_and_sorted_in_both_presentations() {
    let output = run(&["contract", "plugins", "--format", "json"]);
    let report: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    let ids: Vec<_> = report
        .iter()
        .map(|p| p["provider"].as_str().unwrap())
        .collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted);
    let human = run(&["contract", "plugins"]);
    assert!(human.status.success());
    let stdout = String::from_utf8_lossy(&human.stdout);
    assert_eq!(stdout.lines().count(), report.len());
    for plugin in report {
        assert!(stdout.contains(plugin["provider"].as_str().unwrap()));
    }
}
