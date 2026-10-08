use super::*;
#[test]
#[ignore = "requires .NET8; native scoped headers and cancellation deadline"]
fn native_call_scopes_preserve_headers_and_cancel_timeout_without_mutation() {
    let api = Api {
        name: "Scope".into(),
        operations: vec![Operation {
            id: "getThing".into(),
            method: poolster_core::HttpMethod::Get,
            path: "/thing".into(),
            responses: vec![poolster_core::OperationResponse {
                status: "204".into(),
                description: None,
                media_types: vec![],
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let tree = poolster_core::engine::Packages::new()
        .package(
            crate::package("sdk")
                .name("Poolster.Scope")
                .with(crate::sdk())
                .with(crate::operation_tests()),
        )
        .generate(&api, None)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    std::fs::write(
        dir.path().join("sdk/tests/OperationTests/Program.cs"),
        include_str!("call_options_probe.cs.txt"),
    )
    .unwrap();
    let output = std::process::Command::new("dotnet")
        .args([
            "run",
            "--project",
            "tests/OperationTests/OperationTests.csproj",
        ])
        .current_dir(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
