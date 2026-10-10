//! Approved generated-output fingerprints for every maintained Poolster target.
//! TypeScript SDK operations resolve directly to their successful response body.
//!
//! The checked-in fixture protects the actual emitted files, rather than only
//! the generator API.  Update it intentionally after reviewing a generation
//! change; an unexpected byte-level change fails CI with a useful diff.

mod support;

use poolster::{ProfileSet, generate};

const APPROVED_SNAPSHOT: &str = include_str!("fixtures/all-targets.snapshot");

fn fingerprint(contents: &str) -> u64 {
    // FNV-1a is tiny, deterministic and stable across Rust toolchain updates.
    contents
        .as_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}

fn snapshot() -> String {
    let tree = generate(
        &support::sdk_contract_api(),
        ProfileSet::new("sdk")
            .package(poolster::rust::package("rust").with(poolster::rust::sdk()))
            .package(poolster::ts::package("typescript-fetch").with(poolster::ts::sdk().fetch()))
            .package(poolster::ts::package("typescript-axios").with(poolster::ts::sdk().axios()))
            .package(poolster::go::package("go").with(poolster::go::sdk()))
            .package(poolster::python::package("python").with(poolster::python::sdk()))
            .package(poolster::php::package("php").with(poolster::php::sdk()))
            .package(poolster::java::package("java").with(poolster::java::sdk()))
            .package(poolster::csharp::package("csharp").with(poolster::csharp::sdk()))
            .package(poolster::elixir::package("elixir").with(poolster::elixir::sdk()))
            .package(poolster::mock::package("mock-server").with(poolster::mock::server())),
    )
    .expect("the maintained target set must generate");

    tree.iter()
        .map(|(path, contents)| {
            format!(
                "{}\t{}\t{:016x}",
                path.display(),
                contents.len(),
                fingerprint(contents)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[test]
fn every_maintained_target_matches_the_approved_output_snapshot() {
    let actual = snapshot();
    if std::env::var_os("POOLSTER_UPDATE_GOLDEN_SNAPSHOT").is_some() {
        std::fs::write("tests/fixtures/all-targets.snapshot", &actual)
            .expect("update approved generated-output snapshot");
        return;
    }
    assert_eq!(
        actual, APPROVED_SNAPSHOT,
        "generated output changed; review it and intentionally update \
         crates/facade/tests/fixtures/all-targets.snapshot"
    );
}

#[test]
fn generation_is_byte_deterministic_before_snapshot_comparison() {
    assert_eq!(snapshot(), snapshot());
}
