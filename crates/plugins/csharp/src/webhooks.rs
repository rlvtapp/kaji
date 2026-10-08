use super::*;
use poolster_core::engine::{Meta, Plugin, PluginContext};
pub struct Webhooks {
    meta: Meta,
}
pub fn webhooks() -> Webhooks {
    Webhooks { meta: Meta::new() }
}
impl Plugin<CSharp> for Webhooks {
    fn kind(&self) -> &'static str {
        "csharp-webhooks"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, CSharp>) -> Result<()> {
        let namespace = dotnet_namespace(
            cx.settings
                .package_name
                .as_deref()
                .unwrap_or(&format!("{}-sdk", kebab_case(&cx.api.name))),
        );
        cx.files.emit(GeneratedFile::new(
            "StandardWebhooks.cs",
            include_str!("../templates/webhooks.cs.tmpl").replace("__PACKAGE__", &namespace),
        )?)
    }
}

impl Plugin<crate::DotNet> for Webhooks {
    fn kind(&self) -> &'static str {
        "csharp-webhooks"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, crate::DotNet>) -> Result<()> {
        let namespace = dotnet_namespace(
            cx.settings
                .package_name
                .as_deref()
                .unwrap_or(&format!("{}-sdk", kebab_case(&cx.api.name))),
        );
        cx.files.emit(GeneratedFile::new(
            "StandardWebhooks.cs",
            include_str!("../templates/webhooks.cs.tmpl").replace("__PACKAGE__", &namespace),
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
            .find(|(path, _)| path.ends_with("StandardWebhooks.cs"))
            .unwrap()
            .1;
        assert!(source.contains("HmacSHA256") || source.contains("HMACSHA256"));
        assert!(source.contains("does not match a trusted key"));
    }
    #[test]
    #[ignore = "requires native .NET8; canonical Standard Webhooks verification probes"]
    fn native_webhook_verification_executes_canonical_vectors() {
        let vector: serde_json::Value =
            serde_json::from_str(include_str!("../../python/testdata/webhook-vectors.json"))
                .unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("StandardWebhooks.cs"),
            include_str!("../templates/webhooks.cs.tmpl").replace("__PACKAGE__", "ProbeSDK"),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("Probe.cs"),
            include_str!("../tests/fixtures/webhooks_probe.cs")
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
        std::fs::write(dir.path().join("Probe.csproj"),"<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><TargetFramework>net8.0</TargetFramework><OutputType>Exe</OutputType><Nullable>enable</Nullable></PropertyGroup></Project>").unwrap();
        let result = std::process::Command::new("dotnet")
            .args(["run", "--project", "Probe.csproj", "--verbosity", "quiet"])
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
