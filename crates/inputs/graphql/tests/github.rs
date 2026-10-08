//! Offline regression coverage of GitHub's real published GraphQL schema.
//! Source, pinned revision, checksums and MIT license live alongside the fixture.
use poolster_core::input::InputPlugin;
use poolster_input_graphql::{GraphqlDocument, GraphqlInput, parse};

const GITHUB: &str = include_str!("fixtures/github/schema.graphql");

#[test]
fn full_github_schema_parses_validates_and_preserves_native_surface() {
    assert!(GITHUB.len() > 1_000_000);
    let document = parse(GITHUB).unwrap();
    let summary = document.summary();
    assert!(summary.types.len() > 1_000, "{} types", summary.types.len());
    assert!(
        summary.operations.len() > 200,
        "{} operations",
        summary.operations.len()
    );
    for name in ["Repository", "PullRequest", "Issue", "User", "DateTime"] {
        assert!(document.schema.types.contains_key(name), "missing {name}");
        assert!(summary.types.iter().any(|ty| ty == name));
    }
    for (name, kind) in [
        ("repository", "query"),
        ("viewer", "query"),
        ("createIssue", "mutation"),
        ("addComment", "mutation"),
    ] {
        assert!(
            summary
                .operations
                .iter()
                .any(|operation| operation.name == name && operation.kind == kind),
            "missing {kind}.{name}"
        );
    }
    assert!(summary.types.iter().all(|name| !name.starts_with("__")));
}

#[test]
fn full_github_schema_provider_publishes_same_native_contract() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/github/schema.graphql");
    let contract = GraphqlInput.load(&path).unwrap();
    let native = contract.get::<GraphqlDocument>().unwrap();
    assert_eq!(native.summary(), contract.summary);
    assert_eq!(contract.summary.format, "graphql");
}

#[test]
fn full_github_schema_rejects_broken_type_reference() {
    let broken = GITHUB.replacen("type Repository ", "type RemovedRepository ", 1);
    assert_ne!(broken, GITHUB, "fixture no longer contains expected type");
    let error = parse(&broken).unwrap_err().to_string();
    assert!(error.contains("Repository"));
}
