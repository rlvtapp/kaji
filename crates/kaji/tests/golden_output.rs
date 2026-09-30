//! Approved generated-output fingerprints for every maintained Kaji target.
//! TypeScript SDK operations resolve directly to their successful response body.
//!
//! The checked-in fixture protects the actual emitted files, rather than only
//! the generator API.  Update it intentionally after reviewing a generation
//! change; an unexpected byte-level change fails CI with a useful diff.

mod support;

use kaji::{ProfileSet, generate};

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
            .package(kaji::rust::package("rust").with(kaji::rust::sdk()))
            .package(kaji::ts::package("typescript-fetch").with(kaji::ts::sdk().fetch()))
            .package(kaji::ts::package("typescript-axios").with(kaji::ts::sdk().axios()))
            .package(kaji::go::package("go").with(kaji::go::sdk()))
            .package(kaji::python::package("python").with(kaji::python::sdk()))
            .package(kaji::php::package("php").with(kaji::php::sdk()))
            .package(kaji::java::package("java").with(kaji::java::sdk()))
            .package(kaji::dotnet::package("dotnet").with(kaji::dotnet::sdk()))
            .package(kaji::elixir::package("elixir").with(kaji::elixir::sdk()))
            .package(kaji::mock::package("mock-server").with(kaji::mock::server())),
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
    assert_eq!(
        actual, APPROVED_SNAPSHOT,
        "generated output changed; review it and intentionally update \
         crates/kaji/tests/fixtures/all-targets.snapshot"
    );
}

#[test]
fn generation_is_byte_deterministic_before_snapshot_comparison() {
    assert_eq!(snapshot(), snapshot());
}
