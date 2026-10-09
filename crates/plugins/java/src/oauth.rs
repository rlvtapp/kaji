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
impl Plugin<Java> for OAuth {
    fn supports_native_input(&self) -> bool {
        self.http_input.is_explicit()
    }
    fn requires(&self) -> Vec<poolster_core::engine::Requirement> {
        self.http_input.requirements()
    }

    fn kind(&self) -> &'static str {
        "java-oauth"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, Java>) -> Result<()> {
        self.http_input.with_context(cx, |cx| {
            let namespace = java_package_name(
                cx.settings
                    .package_name
                    .as_deref()
                    .unwrap_or(&format!("io.poolster.{}", package_segment(&cx.api.name))),
            );
            for (name, source) in [
                (
                    "OAuthClientCredentials",
                    include_str!("../templates/oauth.java.tmpl"),
                ),
                (
                    "OAuthHttpClient",
                    include_str!("../templates/oauth_http.java.tmpl"),
                ),
            ] {
                cx.files.emit(GeneratedFile::new(
                    format!(
                        "src/main/java/{}/{}.java",
                        namespace.replace('.', "/"),
                        name
                    ),
                    source.replace("__PACKAGE__", &namespace),
                )?)?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires native JDK17 and Maven"]
    fn native_oauth_cache_singleflight_replay_origin_and_interruption() {
        let api = Api {
            name: "OAuth".into(),
            ..Default::default()
        };
        let tree = poolster_core::engine::Packages::new()
            .package(
                crate::package("sdk")
                    .name("io.poolster.oauth")
                    .with(crate::sdk())
                    .with(oauth()),
            )
            .generate(&api, None)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        let cwd = dir.path().join("sdk");
        std::fs::create_dir_all(cwd.join("src/test/java/io/poolster/oauth")).unwrap();
        std::fs::write(
            cwd.join("src/test/java/io/poolster/oauth/OAuthProbe.java"),
            include_str!("../tests/fixtures/oauth_probe.java"),
        )
        .unwrap();
        let output = std::process::Command::new("mvn")
            .args([
                "-q",
                "test-compile",
                "org.codehaus.mojo:exec-maven-plugin:3.5.0:java",
                "-Dexec.mainClass=io.poolster.oauth.OAuthProbe",
                "-Dexec.classpathScope=test",
            ])
            .current_dir(cwd)
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
