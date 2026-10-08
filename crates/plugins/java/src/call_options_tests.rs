use super::*;
#[test]
#[ignore = "requires JDK17+Maven; native scoped request headers and timeout"]
fn native_call_scopes_preserve_headers_timeouts_and_original_client() {
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
                .name("io.poolster.callscope")
                .with(crate::sdk())
                .with(crate::operation_tests()),
        )
        .generate(&api, None)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    let mut source = include_str!("operation_driver.java.txt")
        .replace("__PACKAGE__", "io.poolster.callscope")
        .replace(
            "__CASES__",
            include_str!("call_options_probe_main.java.txt"),
        );
    let start = source.find("        void assertRequest(").unwrap();
    let end = start
        + source[start..]
            .find("        public <T> HttpResponse<T> send(")
            .unwrap();
    source.replace_range(
        start..end,
        include_str!("call_options_probe_assert.java.txt"),
    );
    std::fs::write(
        dir.path()
            .join("sdk/src/test/java/io/kaji/callscope/PoolsterOperationTests.java"),
        source,
    )
    .unwrap();
    let output = std::process::Command::new("mvn")
        .args([
            "-q",
            "test-compile",
            "org.codehaus.mojo:exec-maven-plugin:3.5.0:java",
            "-Dexec.mainClass=io.poolster.callscope.PoolsterOperationTests",
            "-Dexec.classpathScope=test",
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
