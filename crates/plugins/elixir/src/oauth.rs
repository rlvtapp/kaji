use super::*;
use poolster_core::engine::{Meta, Plugin, PluginContext};
pub struct OAuth {
    meta: Meta,
}
pub fn oauth() -> OAuth {
    OAuth { meta: Meta::new() }
}
impl Plugin<crate::Elixir> for OAuth {
    fn kind(&self) -> &'static str {
        "elixir-oauth"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, crate::Elixir>) -> Result<()> {
        let package = cx
            .settings
            .package_name
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(package_slug)
            .unwrap_or_else(|| format!("{}-sdk", package_slug(&cx.api.name)));
        let module = pascal_case(&package);
        let app = elixir_identifier(&package);
        cx.files.emit(GeneratedFile::new(
            format!("lib/{app}/oauth.ex"),
            include_str!("oauth.ex.txt").replace("__KAJI_MODULE__", &module),
        )?)
    }
}
#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires Elixir with Jason; KAJI_ELIXIR_NATIVE_PROJECT points to cached Mix project"]
    fn native_oauth_singleflight_and_origin() {
        let project =
            std::env::var("KAJI_ELIXIR_NATIVE_PROJECT").expect("cached Mix project required");
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("oauth.ex"),
            include_str!("oauth.ex.txt").replace("__KAJI_MODULE__", "ProbeSdk"),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("client.ex"),
            format!(
                "{}\n{}\n{}\n{}",
                crate::render_api_error(&poolster_core::Api::default(), "ProbeSdk"),
                crate::render_json("ProbeSdk"),
                include_str!("multipart.ex.txt").replace("__KAJI_MODULE__", "ProbeSdk"),
                crate::render_client("ProbeSdk")
            ),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("probe.exs"),
            include_str!("oauth_probe.exs"),
        )
        .unwrap();
        let output = std::process::Command::new("mix")
            .args(["run", "--no-compile", "--no-start", "-r"])
            .arg(dir.path().join("oauth.ex"))
            .arg("-r")
            .arg(dir.path().join("client.ex"))
            .arg(dir.path().join("probe.exs"))
            .current_dir(project)
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
