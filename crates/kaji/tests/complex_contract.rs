//! Synthetic, checked-in OpenAPI 3.1 regression corpus. This is deliberately
//! not a claim of compatibility with a third-party API or all JSON Schema.
use kaji::swift::PackageExt;
use kaji::{ProfileSet, csharp, java, swift};
use kaji_core::adapter::openapi_sidecar::OpenApiSidecar;
use std::{path::PathBuf, process::Command};

const SPEC: &str = include_str!("fixtures/complex-contract.openapi.json");

#[test]
fn fixture_is_explicitly_versioned_and_contains_complex_cases() {
    let fingerprint = SPEC.bytes().fold(14695981039346656037_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(1099511628211)
    });
    assert_eq!(
        fingerprint, 17660724544073469139,
        "fixture changes require intentional coverage review"
    );
    let spec: serde_json::Value = serde_json::from_str(SPEC).unwrap();
    assert_eq!(spec["openapi"], "3.1.0");
    let schemas = &spec["components"]["schemas"];
    assert_eq!(
        schemas["Node"]["properties"]["child"]["$ref"],
        "#/components/schemas/Node"
    );
    assert!(schemas["Choice"]["oneOf"].is_array());
    assert!(schemas["LooseChoice"]["anyOf"].is_array());
    assert!(schemas["Node"]["properties"]["雪"].is_object());
    assert!(schemas["Collision"]["properties"]["display_name"].is_object());
}

#[test]
#[ignore = "requires built Kaji OpenAPI compiler and Swift 6"]
fn real_openapi_complex_models_compile_and_round_trip_with_explicit_collision_errors() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let compiler = std::env::var_os("KAJI_OPENAPI_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/debug/kaji-openapi"));
    let temp = tempfile::tempdir().unwrap();
    let artifacts = temp.path().join("ast");
    let result = Command::new(compiler)
        .arg("--out")
        .arg(&artifacts)
        .arg(root.join("crates/kaji/tests/fixtures/complex-contract.openapi.json"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let mut adapted = OpenApiSidecar::new(artifacts, "complex", "1.0.0")
        .load()
        .unwrap();
    // Colliding names fail with a useful diagnostic rather than invalid source.
    for profiles in [
        ProfileSet::new("sdk").package(swift::package("swift").with(swift::sdk())),
        ProfileSet::new("sdk").package(java::package("java").with(java::sdk())),
        ProfileSet::new("sdk").package(csharp::package("csharp").with(csharp::sdk())),
    ] {
        let error = kaji::generate_with_security_catalog(
            &adapted.api,
            profiles,
            Some(&adapted.security_schemes),
        )
        .unwrap_err();
        let message = format!("{error:#}");
        assert!(
            message.contains("collide as native identifier"),
            "{message}"
        );
    }
    adapted
        .api
        .schemas
        .retain(|schema| schema.name != "Collision");
    let tree = kaji::generate_with_security_catalog(
        &adapted.api,
        ProfileSet::new("sdk").package(
            swift::package("swift")
                .name("ComplexSDK")
                .with(swift::sdk()),
        ),
        Some(&adapted.security_schemes),
    )
    .unwrap();
    tree.write_to(temp.path()).unwrap();
    let models = temp.path().join("sdk/swift/Sources/ComplexSDK/Models");
    let main = temp.path().join("main.swift");
    std::fs::write(&main, r##"
import Foundation
@main struct Probe {
    static func main() throws {
        let input = Data(#"{"id":"雪","nullable":null,"optional":null,"display-name":"café","雪":"値","choice":42,"mode":"known","child":{"id":"nested","nullable":null},"future":{"nested":true}}"#.utf8)
        let node = try JSONDecoder().decode(Node.self, from: input)
        let output = try JSONEncoder().encode(node)
        let original = try JSONSerialization.jsonObject(with: input) as! NSDictionary
        let roundtrip = try JSONSerialization.jsonObject(with: output) as! NSDictionary
        precondition(original == roundtrip)
        let absent = try JSONDecoder().decode(Node.self, from: Data(#"{"id":"x","nullable":null}"#.utf8))
        let absentObject = try JSONSerialization.jsonObject(with: JSONEncoder().encode(absent)) as! [String:Any]
        precondition(absentObject["optional"] == nil && absentObject["nullable"] is NSNull)
        // Swift currently has closed native enums: unknown wire cases reject.
        do { _ = try JSONDecoder().decode(Mode.self, from: Data(#""future""#.utf8)); preconditionFailure() }
        catch is DecodingError {}
        // Unions retain their JSON representation through JSONValue aliases.
        let choice = try JSONDecoder().decode(Choice.self, from: Data(#""future""#.utf8))
        precondition(choice == .string("future"))
    }
}
"##).unwrap();
    let mut sources = std::fs::read_dir(models)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    sources.push(
        temp.path()
            .join("sdk/swift/Sources/ComplexSDK/JSONValue.swift"),
    );
    sources.push(main);
    let executable = temp.path().join("probe");
    let output = Command::new("swiftc")
        .args([
            "-swift-version",
            "6",
            "-warnings-as-errors",
            "-parse-as-library",
            "-module-cache-path",
        ])
        .arg(temp.path().join("cache"))
        .args(&sources)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(Command::new(executable).status().unwrap().success());
}
