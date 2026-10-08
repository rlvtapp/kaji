use kaji_input_capnproto as capnproto;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/rpc")
        .join(name)
}

#[test]
fn capnproto_inspects_imports_or_reports_compiler_requirement() {
    let available = std::process::Command::new("capnp")
        .arg("--version")
        .output();
    if available
        .as_ref()
        .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    {
        let error = capnproto::load(&fixture("service.capnp")).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("requires the official `capnp` compiler")
        );
        eprintln!(
            "SKIPPED external Cap'n Proto schema compilation: capnp is absent; requirement diagnostic verified"
        );
        return;
    }
    let document = capnproto::load(&fixture("service.capnp")).unwrap();
    let summary = document.summary();
    assert!(summary.types.iter().any(|name| name.ends_with(":Item")));
    assert!(summary.operations.iter().any(
        |operation| operation.name.ends_with("Store.get") && operation.kind == "capability_rpc"
    ));
    assert!(!document.schema_request.is_empty());
}

#[test]
fn missing_source_is_reported_before_invoking_compiler() {
    let error = capnproto::load(&fixture("absent.capnp")).unwrap_err();
    assert!(error.to_string().contains("reading Cap'n Proto input"));
    assert!(error.to_string().contains("absent.capnp"));
}

#[test]
fn provider_propagates_missing_source() {
    use kaji_core::input::InputPlugin;
    assert!(
        capnproto::CapnProtoInput
            .load(&fixture("absent.capnp"))
            .is_err()
    );
}

#[test]
fn official_capnproto_schema_and_rpc_descriptor_corpus() {
    let bytes = include_bytes!("fixtures/upstream/corpus.bin").to_vec();
    let document =
        capnproto::CapnProtoDocument::from_schema_request(bytes.clone(), "official-corpus".into())
            .unwrap();
    let summary = document.summary();
    eprintln!(
        "Official Cap'n Proto corpus: {} descriptor bytes, {} types, {} operations",
        bytes.len(),
        summary.types.len(),
        summary.operations.len()
    );
    assert_eq!(document.schema_request, bytes);
    assert!(summary.types.len() > 100);
    assert!(summary.operations.len() > 20);
    assert!(
        summary
            .types
            .iter()
            .any(|name| name.ends_with("schema.capnp:Node"))
    );
    assert!(
        summary
            .types
            .iter()
            .any(|name| name.ends_with("rpc.capnp:Message"))
    );
    assert!(
        summary
            .operations
            .iter()
            .any(|operation| operation.kind == "capability_streaming")
    );
    assert!(
        summary
            .operations
            .iter()
            .any(|operation| operation.kind == "capability_rpc")
    );
}

#[test]
fn official_source_corpus_compiles_when_compiler_is_available() {
    if std::process::Command::new("capnp")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!(
            "SKIPPED official Cap'n Proto source corpus compilation: capnp absent; retained official descriptor request is tested independently"
        );
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/upstream");
    for source in [
        "test.capnp",
        "schema.capnp",
        "rpc.capnp",
        "rpc-twoparty.capnp",
    ] {
        let document = capnproto::load_with_includes(
            &root.join("capnp").join(source),
            std::slice::from_ref(&root),
        )
        .unwrap();
        assert!(!document.summary().types.is_empty());
    }
}
