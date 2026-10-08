use super::*;
use kaji_core::engine::{Meta, Plugin, PluginContext};
/// Optional raw-body Standard Webhooks v1 verifier. Native requirements: PHP 8.2+ with hash and JSON extensions.
pub struct Webhooks {
    meta: Meta,
}
pub fn webhooks() -> Webhooks {
    Webhooks { meta: Meta::new() }
}
impl Plugin<crate::Php> for Webhooks {
    fn kind(&self) -> &'static str {
        "php-webhooks"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, crate::Php>) -> Result<()> {
        let package = cx
            .settings
            .package_name
            .clone()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| format!("kaji/{}-sdk", package_slug(&cx.api.name)));
        let namespace = namespace_for_package(&package);
        cx.files.emit(GeneratedFile::new(
            "src/Webhooks.php",
            include_str!("../templates/webhooks.php.tmpl").replace("__NAMESPACE__", &namespace),
        )?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn generate() -> (tempfile::TempDir, String) {
        use kaji_core::engine::Packages;
        let tree = Packages::new()
            .package(crate::package("sdk").name("security-sdk").with(webhooks()))
            .generate(
                &Api {
                    name: "Other".into(),
                    ..Default::default()
                },
                None,
            )
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        let source = std::fs::read_to_string(dir.path().join("sdk/src/Webhooks.php")).unwrap();
        (dir, source)
    }
    #[test]
    fn verifier_uses_selected_package_and_constant_time_backend() {
        let (_, source) = generate();
        assert!(source.contains("namespace Security\\Sdk;"));
        assert!(source.contains("hash_equals"));
        assert!(source.contains("array_key_exists"));
        assert!(!source.contains("__NAMESPACE__"));
    }
    #[test]
    #[ignore = "Requires PHP 8.2+ with hash and JSON extensions; executes generated code, no remote services"]
    fn generated_webhook_verifier_executes_native_security_cases() {
        let (dir, _) = generate();
        let root = dir.path().join("sdk");
        std::fs::write(root.join("probe.php"), include_str!("../tests/fixtures/webhooks_probe.php")).unwrap();
        let output = std::process::Command::new("php")
            .arg("probe.php")
            .current_dir(root)
            .output()
            .expect("Required native toolchain unavailable");
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
