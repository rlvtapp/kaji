use super::*;
use poolster_core::engine::{Meta, Plugin, PluginContext};
pub struct Webhooks {
    meta: Meta,
}
pub fn webhooks() -> Webhooks {
    Webhooks { meta: Meta::new() }
}
impl Plugin<Java> for Webhooks {
    fn kind(&self) -> &'static str {
        "java-webhooks"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, Java>) -> Result<()> {
        let namespace = java_package_name(
            cx.settings
                .package_name
                .as_deref()
                .unwrap_or(&format!("io.poolster.{}", package_segment(&cx.api.name))),
        );
        cx.files.emit(GeneratedFile::new(
            format!(
                "src/main/java/{}/StandardWebhooks.java",
                namespace.replace('.', "/")
            ),
            include_str!("../templates/webhooks.java.tmpl").replace("__PACKAGE__", &namespace),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use poolster_core::engine::Packages;
    #[test]
    fn opt_in_emits_standard_webhook_verifier() {
        let tree = Packages::new()
            .package(crate::package("sdk").with(crate::sdk()).with(webhooks()))
            .generate(&Api::default(), None)
            .unwrap();
        let source = tree
            .iter()
            .find(|(path, _)| path.ends_with("StandardWebhooks.java"))
            .unwrap()
            .1;
        assert!(source.contains("HmacSHA256") || source.contains("HMACSHA256"));
        assert!(source.contains("does not match a trusted key"));
    }
    #[test]
    #[ignore = "requires native JDK17; canonical Standard Webhooks verification probes"]
    fn native_webhook_verification_executes_canonical_vectors() {
        let vector: serde_json::Value =
            serde_json::from_str(include_str!("../../python/testdata/webhook-vectors.json"))
                .unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("StandardWebhooks.java"),
            include_str!("../templates/webhooks.java.tmpl").replace("__PACKAGE__", "probe"),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("Probe.java"),
            include_str!("../tests/fixtures/webhooks_probe.java")
                .replace(
                    "__BODY__",
                    &serde_json::to_string(&vector["raw_body"]).unwrap(),
                )
                .replace(
                    "__SECRET__",
                    &serde_json::to_string(&vector["secret"]).unwrap(),
                )
                .replace(
                    "__SIGNATURE__",
                    &serde_json::to_string(&vector["headers"]["webhook-signature"]).unwrap(),
                ),
        )
        .unwrap();
        let result = std::process::Command::new("javac")
            .args(["-d", ".", "StandardWebhooks.java", "Probe.java"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let result = std::process::Command::new("java")
            .args(["-cp", ".", "probe.Probe"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
