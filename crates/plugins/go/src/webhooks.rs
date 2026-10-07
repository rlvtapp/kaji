use super::*;
use kaji_core::engine::{Meta, Plugin, PluginContext};
pub struct Webhooks {
    meta: Meta,
}
pub fn webhooks() -> Webhooks {
    Webhooks { meta: Meta::new() }
}
impl Plugin<Go> for Webhooks {
    fn kind(&self) -> &'static str {
        "go-webhooks"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, Go>) -> Result<()> {
        let package = go_package_name(cx.settings.package_name.as_deref().unwrap_or(&cx.api.name));
        cx.files.emit(GeneratedFile::new(
            "webhooks.go",
            include_str!("webhooks.go.txt").replace("__PACKAGE__", &package),
        )?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_webhooks_and_oauth_execute_native_security_cases() {
        use kaji_core::engine::Packages;
        let api = Api {
            name: "Security".into(),
            ..Default::default()
        };
        let tree = Packages::new()
            .package(
                crate::package("sdk")
                    .name("example.com/security")
                    .with(crate::sdk())
                    .with(webhooks())
                    .with(crate::oauth()),
            )
            .generate(&api, None)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        std::fs::write(
            dir.path().join("sdk/security_test.go"),
            include_str!("security_test.go.txt")
                .replace("__PACKAGE__", &go_package_name("example.com/security")),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("sdk/vector.json"),
            include_str!("../../python/testdata/webhook-vectors.json"),
        )
        .unwrap();
        let output = std::process::Command::new("go")
            .args(["test", "-race", "./..."])
            .env("GOCACHE", "/private/tmp/kaji-go-cache")
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
}
