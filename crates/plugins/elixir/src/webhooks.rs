use super::*;
use poolster_core::engine::{Meta, Plugin, PluginContext};
/// Optional raw-body Standard Webhooks v1 verifier. Native requirements: Elixir 1.15+, OTP 25+ with crypto.
pub struct Webhooks {
    meta: Meta,
}
pub fn webhooks() -> Webhooks {
    Webhooks { meta: Meta::new() }
}
impl Plugin<crate::Elixir> for Webhooks {
    fn kind(&self) -> &'static str {
        "elixir-webhooks"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, crate::Elixir>) -> Result<()> {
        let package = cx
            .settings
            .package_name
            .as_deref()
            .filter(|name| !name.trim().is_empty())
            .map(package_slug)
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| format!("{}-sdk", package_slug(&cx.api.name)));
        let namespace = pascal_case(&package);
        let app = elixir_identifier(&package);
        cx.files.emit(GeneratedFile::new(
            format!("lib/{app}/webhooks.ex"),
            include_str!("../templates/webhooks.ex.tmpl").replace("__MODULE__", &namespace),
        )?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn generate() -> (tempfile::TempDir, String) {
        use poolster_core::engine::Packages;
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
        let source =
            std::fs::read_to_string(dir.path().join("sdk/lib/security_sdk/webhooks.ex")).unwrap();
        (dir, source)
    }
    #[test]
    fn verifier_uses_selected_package_and_constant_time_backend() {
        let (_, source) = generate();
        assert!(source.contains("defmodule SecuritySdk.Webhooks"));
        assert!(source.contains(":crypto.hash_equals"));
        assert!(source.contains("Map.has_key?"));
        assert!(!source.contains("__MODULE__"));
    }
    #[test]
    #[ignore = "Requires Elixir 1.15+, OTP 25+ with crypto; executes generated code, no remote services"]
    fn generated_webhook_verifier_executes_native_security_cases() {
        let (dir, _) = generate();
        let root = dir.path().join("sdk");
        std::fs::write(
            root.join("probe.exs"),
            include_str!("../tests/fixtures/webhooks_probe.exs"),
        )
        .unwrap();
        let output = std::process::Command::new("elixir")
            .args(["-r", "lib/security_sdk/webhooks.ex", "probe.exs"])
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
