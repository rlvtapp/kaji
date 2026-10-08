#[cfg(test)]
mod tests {
    use crate::*;
    fn api() -> Api {
        Api {
            name: "Streams".into(),
            version: "1.0.0".into(),
            operations: vec![Operation {
                id: "getEvents".into(),
                method: poolster_core::HttpMethod::Get,
                path: "/events".into(),
                responses: vec![poolster_core::OperationResponse {
                    status: "200".into(),
                    description: None,
                    media_types: vec![poolster_core::OperationMediaType {
                        content_type: "text/event-stream".into(),
                        schema: Some(SchemaValue::new(SchemaKind::String)),
                    }],
                }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }
    #[test]
    fn emits_incremental_lazy_sequence_and_explicit_transport_capability() {
        let tree = render_sdk(&api(), "sdk", Some("Streams"), SdkClientStyle::Namespaced).unwrap();
        let operations = tree.get("sdk/Sources/Streams/Operations.swift").unwrap();
        assert!(operations.contains("func getEvents() -> PoolsterEventSequence"));
        assert!(!operations.contains("async throws -> String"));
        let runtime = tree
            .get("sdk/Sources/Streams/PoolsterClient.swift")
            .unwrap();
        assert!(runtime.contains("as? any PoolsterStreamingTransport"));
        assert!(runtime.contains("session.delegate == nil"));
        let stream = tree.get("sdk/Sources/Streams/Streaming.swift").unwrap();
        assert!(stream.contains("bufferingOldest(32)"));
        assert!(stream.contains("Incomplete final events are discarded"));
    }
    #[test]
    #[ignore = "Requires Swift6; native chunk framing, middleware, cancellation and bounded-memory probe"]
    fn native_sse_frames_incrementally_preserves_policy_and_cancels() {
        compile_and_run(include_str!("sse_probe.swift.txt"));
    }
    #[test]
    #[ignore = "Requires Swift6+Python3 and loopback sockets; incremental URLSession stream/cancellation/redirect probe"]
    fn native_urlsession_sse_delivers_before_eof_and_cancels_socket() {
        use std::io::BufRead;
        let root = tempfile::tempdir().unwrap();
        let server = root.path().join("server.py");
        std::fs::write(&server, include_str!("sse_loopback_server.py")).unwrap();
        let mut child = std::process::Command::new("python3")
            .arg(server)
            .arg(root.path())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut port = String::new();
        std::io::BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut port)
            .unwrap();
        assert!(
            port.trim().parse::<u16>().is_ok(),
            "Loopback server failed to bind"
        );
        let script = include_str!("sse_loopback_probe.swift.txt")
            .replace("__PORT__", port.trim())
            .replace("__ROOT__", root.path().to_str().unwrap());
        let result = std::panic::catch_unwind(|| compile_and_run(&script));
        let _ = child.kill();
        let _ = child.wait();
        if let Err(error) = result {
            std::panic::resume_unwind(error)
        }
    }
    fn compile_and_run(script: &str) {
        let root = tempfile::tempdir().unwrap();
        render_sdk(&api(), "sdk", Some("Streams"), SdkClientStyle::Namespaced)
            .unwrap()
            .write_to(root.path())
            .unwrap();
        let main = root.path().join("main.swift");
        std::fs::write(&main, script).unwrap();
        fn sources(path: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    sources(&path, out)
                } else if path.extension().is_some_and(|ext| ext == "swift") {
                    out.push(path)
                }
            }
        }
        let mut files = vec![];
        sources(&root.path().join("sdk/Sources"), &mut files);
        let output = std::process::Command::new("swiftc")
            .args([
                "-parse-as-library",
                "-swift-version",
                "6",
                "-warnings-as-errors",
                "-module-cache-path",
            ])
            .arg(root.path().join("cache"))
            .args(files)
            .arg(main)
            .arg("-o")
            .arg(root.path().join("probe"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = std::process::Command::new(root.path().join("probe"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
