use super::*;
use poolster_core::engine::{Meta, Plugin, PluginContext};
pub struct Webhooks {
    http_input: poolster_core::engine::HttpInput,
    meta: Meta,
}
pub fn webhooks() -> Webhooks {
    Webhooks {
        http_input: Default::default(),
        meta: Meta::new(),
    }
}
impl Plugin<Go> for Webhooks {
    fn supports_native_input(&self) -> bool {
        self.http_input.is_explicit()
    }
    fn requires(&self) -> Vec<poolster_core::engine::Requirement> {
        self.http_input.requirements()
    }

    fn kind(&self) -> &'static str {
        "go-webhooks"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, Go>) -> Result<()> {
        self.http_input.with_context(cx, |cx| {
            let package =
                go_package_name(cx.settings.package_name.as_deref().unwrap_or(&cx.api.name));
            cx.files.emit(GeneratedFile::new(
                "webhooks.go",
                include_str!("../templates/webhooks.go.tmpl").replace("__PACKAGE__", &package),
            )?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_webhooks_and_oauth_execute_native_security_cases() {
        use poolster_core::engine::Packages;
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
            include_str!("../tests/fixtures/security_test.go")
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
            .env(
                "GOCACHE",
                std::env::var_os("GOCACHE").unwrap_or_else(|| {
                    std::env::temp_dir()
                        .join("poolster-go-cache")
                        .into_os_string()
                }),
            )
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

#[path = "webhooks_input.rs"]
mod http_input;
