use super::*;

#[test]
fn retry_headers_preserve_zero_precedence_dates_and_bounds() {
    let tree = render_test_sdk(&contact_api(), "java", Some("com.poolster.email")).unwrap();
    let client = rendered_java(&tree);
    assert!(client.contains("firstValue(\"retry-after-ms\")"));
    assert!(client.contains("retryAfterMillisDelay(retryAfterMillis)"));
    assert!(client.contains("retryAllowed(method, headers, null)"));
    assert!(client.contains("entry.getValue() != null && !entry.getValue().isBlank()"));
    assert!(!client.contains("retryAllowed(method, headers)"));
    assert!(!client.contains("retryDelay(attempt, null)"));
    assert!(!client.contains("retryDelay(attempt, error.retryAfter())"));
    assert!(client.contains("if (delay < 0) delay = retryAfterDelay(retryAfter)"));
    assert!(client.contains("delay = Math.min(delay, cap)"));
    assert!(client.contains("DateTimeFormatter.RFC_1123_DATE_TIME"));
    assert!(client.contains("!Double.isFinite(milliseconds) || milliseconds < 0"));
    let error = super::api_exception("example");
    assert!(error.contains("this(statusCode, responseBody, retryAfter, null)"));
    assert!(error.contains("public String retryAfterMillis()"));
}

#[test]
#[ignore = "requires a JDK 17 toolchain; executes emitted retry parsers without dependencies"]
fn generated_retry_parsers_execute_with_jdk() {
    use std::process::Command;
    let client = super::render_client_base(&contact_api(), "example")
        .replace(
            include_str!("../../templates/call_options_methods.java.tmpl"),
            "",
        )
        .replace(
            include_str!("../../templates/sequential_json.java.tmpl"),
            "",
        );
    let start = client
        .find("    protected static long retryAfterMillisDelay(")
        .unwrap();
    let end = client.rfind("\n}").unwrap();
    let eligibility_start = client
        .find("    protected static boolean retryAllowed(")
        .unwrap();
    let eligibility_end = client
        .find("    protected static boolean retryableStatus(")
        .unwrap();
    let parsers = format!(
        "{}\n{}",
        &client[eligibility_start..eligibility_end],
        &client[start..end]
    );
    let probe = format!(
        "import java.util.Map;\nimport java.util.Locale;\npublic class ParserProbe {{\n{parsers}\n{}\n}}",
        r#"
    static void check(boolean value) { if (!value) throw new AssertionError(); }
    public static void main(String[] args) {
        var closed = new java.util.concurrent.atomic.AtomicBoolean();
        try (var events = ssePayloads(java.util.stream.Stream.of(": keepalive", "event: update", "id: 1", "data: first", "data:  second", "", "retry: 100", "data:", "", "data:last").onClose(() -> closed.set(true)))) {
            check(events.toList().equals(java.util.List.of("first\n second", "", "last")));
        }
        check(closed.get());
        check(!retryAllowed("POST", Map.of("Idempotency-Key", ""), null));
        check(!retryAllowed("PATCH", Map.of("X-Key", ""), "X-Key"));
        check(!retryAllowed("POST", Map.of("Idempotency-Key", " "), null));
        check(!retryAllowed("PATCH", Map.of("X-Key", "\t"), "X-Key"));
        check(retryAllowed("POST", Map.of("idempotency-key", "provided"), null));
        check(retryAllowed("PATCH", Map.of("x-key", "provided"), "X-Key"));
        check(retryAfterMillisDelay("0") == 0);
        check(retryAfterMillisDelay("125.5") == 125);
        check(retryAfterMillisDelay("NaN") == -1);
        check(retryAfterMillisDelay("Infinity") == -1);
        check(retryAfterMillisDelay("-1") == -1);
        check(retryAfterMillisDelay("invalid") == -1);
        check(retryAfterMillisDelay("1e300") == Long.MAX_VALUE);
        check(retryAfterDelay("0") == 0);
        check(retryAfterDelay("2") == 2000);
        check(retryAfterDelay("NaN") == -1);
        check(retryAfterDelay("invalid") == -1);
        check(retryAfterDelay("Thu, 01 Jan 1970 00:00:00 GMT") == 0);
        var future = java.time.ZonedDateTime.now(java.time.ZoneOffset.UTC).plusSeconds(5).format(java.time.format.DateTimeFormatter.RFC_1123_DATE_TIME);
        var delay = retryAfterDelay(future);
        check(delay >= 3000 && delay <= 5000);
    }
"#
    );
    let output = tempfile::tempdir().unwrap();
    std::fs::write(output.path().join("ParserProbe.java"), probe).unwrap();
    for (tool, args) in [
        ("javac", vec!["ParserProbe.java"]),
        ("java", vec!["-cp", ".", "ParserProbe"]),
    ] {
        let result = Command::new(tool)
            .args(args)
            .current_dir(output.path())
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
#[ignore = "requires Maven and a JDK 17 toolchain"]
fn generated_package_compiles_with_maven() {
    use std::process::Command;

    let mut source = contact_api();
    let mut page = source.operations[0].clone();
    page.id = "listItemPages".into();
    page.path = "/items".into();
    page.request_body = None;
    page.parameters = vec![OperationParameter {
        name: "page".into(),
        location: "query".into(),
        required: false,
        schema: Some(integer()),
        description: None,
        annotations: BTreeMap::new(),
    }];
    page.responses = vec![poolster_core::OperationResponse::json(
        "200",
        SchemaValue::new(SchemaKind::Array {
            items: Box::new(SchemaValue::new(SchemaKind::String)),
        }),
    )];
    page.annotations.insert("x-poolster-pagination".into(), serde_json::json!({"type":"page","inputs":[{"name":"page","type":"page"}],"outputs":{"results":"$"}}));
    source.operations.push(page);
    let mut keyed = source.operations[0].clone();
    keyed.id = "createKeyedItem".into();
    keyed.method = poolster_core::HttpMethod::Post;
    keyed.annotations.insert(
        "x-poolster-idempotency".into(),
        serde_json::json!({"header":"X-Request-Key","auto_generate":true}),
    );
    source.operations.push(keyed);
    let source = poolster_core::idempotency::prepare_api(&source, &Default::default()).unwrap();
    let root = tempfile::tempdir().unwrap();
    render_sdk(
        &source,
        "sdk",
        Some("com.poolster.email"),
        SdkClientStyle::Namespaced,
    )
    .unwrap()
    .write_to(root.path())
    .unwrap();
    let output = Command::new("mvn")
        .args([
            "--batch-mode",
            "--no-transfer-progress",
            "-q",
            "-DskipTests",
            "compile",
        ])
        .current_dir(root.path().join("sdk"))
        .output()
        .expect("Maven must be available when this test is selected");
    assert!(
        output.status.success(),
        "generated Java package failed to compile:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    generated_retry_parsers_execute_with_jdk();
}
