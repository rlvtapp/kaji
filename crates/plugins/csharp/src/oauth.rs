use super::*;
use poolster_core::engine::{Meta, Plugin, PluginContext};
pub struct OAuth {
    http_input: poolster_core::engine::HttpInput,
    meta: Meta,
}
pub fn oauth() -> OAuth {
    OAuth {
        http_input: Default::default(),
        meta: Meta::new(),
    }
}
macro_rules! implementation {
    ($target:ty) => {
        impl Plugin<$target> for OAuth {
            fn supports_native_input(&self) -> bool {
                self.http_input.is_explicit()
            }
            fn requires(&self) -> Vec<poolster_core::engine::Requirement> {
                self.http_input.requirements()
            }

            fn kind(&self) -> &'static str {
                "csharp-oauth"
            }
            fn meta(&self) -> &Meta {
                &self.meta
            }
            fn generate(&self, cx: &mut PluginContext<'_, $target>) -> Result<()> {
                self.http_input.with_context(cx, |cx| {
                    let namespace = dotnet_namespace(
                        cx.settings
                            .package_name
                            .as_deref()
                            .unwrap_or(&format!("{}-sdk", kebab_case(&cx.api.name))),
                    );
                    cx.files.emit(GeneratedFile::new(
                        "OAuthClientCredentials.cs",
                        include_str!("../templates/oauth.cs.tmpl")
                            .replace("__PACKAGE__", &namespace),
                    )?)
                })
            }
        }
    };
}
implementation!(CSharp);

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires native .NET8"]
    fn native_oauth_cache_singleflight_replay_origin_and_cancellation() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("OAuth.cs"),
            include_str!("../templates/oauth.cs.tmpl").replace("__PACKAGE__", "ProbeSDK"),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("Probe.cs"),
            include_str!("../tests/fixtures/oauth_probe.cs"),
        )
        .unwrap();
        std::fs::write(dir.path().join("Probe.csproj"), "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net8.0</TargetFramework><Nullable>enable</Nullable><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>").unwrap();
        let output = std::process::Command::new("dotnet")
            .args(["run", "--project", "Probe.csproj"])
            .current_dir(dir.path())
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

#[path = "oauth_input.rs"]
mod http_input;
