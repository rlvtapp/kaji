#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, process::Command};
    #[test]
    #[ignore = "needs Node, TypeScript and Axios; exercises real loopback HTTP drivers"]
    fn native_fetch_and_axios_per_call_controls_bound_retries_and_close_sockets() {
        let compiler = std::env::var_os("KAJI_TSC_JS").expect("set KAJI_TSC_JS");
        let dependencies = std::env::var_os("KAJI_TS_AXIOS_NODE_MODULES")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
                    "../../../examples/typescript-stack/generated/typescript/axios/node_modules",
                )
            });
        assert!(
            dependencies.join("axios").is_dir(),
            "set KAJI_TS_AXIOS_NODE_MODULES to an installed node_modules directory"
        );
        for transport in [
            crate::sdk::SdkTransport::Fetch,
            crate::sdk::SdkTransport::Axios,
        ] {
            let root = tempfile::tempdir().unwrap();
            std::os::unix::fs::symlink(&dependencies, root.path().join("node_modules")).unwrap();
            fs::write(
                root.path().join("runtime.ts"),
                crate::sdk::poolster_runtime(transport, None),
            )
            .unwrap();
            fs::write(
                root.path().join("probe.cjs"),
                include_str!("../tests/fixtures/request_control_probe.cjs"),
            )
            .unwrap();
            let output = Command::new("node")
                .arg(&compiler)
                .args([
                    "--module",
                    "commonjs",
                    "--target",
                    "ES2022",
                    "--lib",
                    "ES2022,DOM,DOM.Iterable",
                    "--strict",
                    "--skipLibCheck",
                    "--esModuleInterop",
                ])
                .arg("runtime.ts")
                .current_dir(root.path())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let mut command = Command::new("node");
            command
                .arg("probe.cjs")
                .current_dir(root.path())
                .env("NODE_PATH", &dependencies);
            if matches!(transport, crate::sdk::SdkTransport::Axios) {
                command.env("POOLSTER_CONTROL_AXIOS", "1");
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
