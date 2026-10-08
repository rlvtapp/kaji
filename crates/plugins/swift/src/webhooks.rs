use super::*;
use poolster_core::engine::{Meta, Plugin, PluginContext};
pub struct Webhooks {
    meta: Meta,
}
pub fn webhooks() -> Webhooks {
    Webhooks { meta: Meta::new() }
}
impl Plugin<Swift> for Webhooks {
    fn kind(&self) -> &'static str {
        "swift-webhooks"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, Swift>) -> Result<()> {
        let namespace = type_name(
            cx.settings
                .package_name
                .as_deref()
                .unwrap_or(&format!("{}-sdk", kebab_case(&cx.api.name))),
        );
        cx.files.emit(GeneratedFile::new(
            format!("Sources/{}/StandardWebhooks.swift", namespace),
            include_str!("../templates/webhooks.swift.tmpl").replace("__PACKAGE__", &namespace),
        )?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use poolster_core::engine::Packages;
    #[test]
    fn opt_in_adds_only_pinned_crypto_dependency() {
        let api = Api {
            name: "Test".into(),
            ..Default::default()
        };
        let profiles = || crate::package("sdk").name("WebhookSDK").with(crate::sdk());
        let plain = Packages::new()
            .package(profiles())
            .generate(&api, None)
            .unwrap();
        assert!(
            !plain
                .get("sdk/Package.swift")
                .unwrap()
                .contains("swift-crypto")
        );
        let tree = Packages::new()
            .package(profiles().with(webhooks()))
            .generate(&api, None)
            .unwrap();
        let manifest = tree.get("sdk/Package.swift").unwrap();
        assert!(manifest.contains("exact: \"3.12.3\""));
        assert!(
            manifest
                .contains("dependencies: [.product(name: \"Crypto\", package: \"swift-crypto\")]")
        );
    }
    #[test]
    #[ignore = "requires Swift toolchain; CryptoKit executes canonical webhook vectors"]
    fn native_webhook_verification_executes_canonical_vectors() {
        let dir = tempfile::tempdir().unwrap();
        let tree = Packages::new()
            .package(
                crate::package("sdk")
                    .name("WebhookSDK")
                    .with(crate::sdk())
                    .with(webhooks()),
            )
            .generate(&Api::default(), None)
            .unwrap();
        tree.write_to(dir.path()).unwrap();
        let manifest = std::process::Command::new("swift")
            .args(["package", "--disable-sandbox", "--package-path"])
            .arg(dir.path().join("sdk"))
            .arg("dump-package")
            .env("CLANG_MODULE_CACHE_PATH", dir.path().join("cache"))
            .env(
                "SWIFTPM_MODULECACHE_OVERRIDE",
                dir.path().join("swiftpm-cache"),
            )
            .output()
            .unwrap();
        assert!(
            manifest.status.success(),
            "{}",
            String::from_utf8_lossy(&manifest.stderr)
        );
        let source = dir.path().join("Webhooks.swift");
        std::fs::write(&source, include_str!("../templates/webhooks.swift.tmpl")).unwrap();
        let main = dir.path().join("Probe.swift");
        std::fs::write(
            &main,
            include_str!("../tests/fixtures/webhooks_probe.swift"),
        )
        .unwrap();
        let vector = dir.path().join("vector.json");
        std::fs::write(
            &vector,
            include_str!("../../python/testdata/webhook-vectors.json"),
        )
        .unwrap();
        let binary = dir.path().join("probe");
        let output = std::process::Command::new("swiftc")
            .args([
                "-swift-version",
                "6",
                "-warnings-as-errors",
                "-parse-as-library",
                "-module-cache-path",
            ])
            .arg(dir.path().join("cache"))
            .args([source, main])
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            std::process::Command::new(binary)
                .arg(vector)
                .status()
                .unwrap()
                .success()
        );
    }
}
