use crate::TypeScript;
use anyhow::Result;
use poolster_core::{
    GeneratedFile,
    engine::{Meta, Plugin, PluginContext, Requirement},
};
pub struct OAuth {
    meta: Meta,
    transport: Option<poolster_core::engine::Handle<crate::composition::Transport>>,
}
pub fn oauth() -> OAuth {
    OAuth {
        meta: Meta::new(),
        transport: None,
    }
}
impl OAuth {
    pub fn using_transport(
        mut self,
        transport: poolster_core::engine::Handle<crate::composition::Transport>,
    ) -> Self {
        self.transport = Some(transport);
        self
    }
    pub fn transport_from(self, sdk: &crate::Sdk) -> Self {
        self.using_transport(sdk.transport_handle())
    }
}
impl Plugin<TypeScript> for OAuth {
    fn kind(&self) -> &'static str {
        "typescript-oauth"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.transport)]
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
        let transport = cx.inputs.get::<crate::composition::Transport>()?;
        anyhow::ensure!(
            transport.module == std::path::Path::new(".poolster/client"),
            "OAuth requires the maintained TypeScript transport"
        );
        cx.workspace
            .declare("oauth", "OAuthClientCredentials", "typescript-oauth")?;
        cx.workspace
            .declare("oauth", "createOAuthClient", "typescript-oauth")?;
        cx.workspace.export("oauth")?;
        cx.files.emit(GeneratedFile::new(
            "oauth.ts",
            include_str!("../templates/oauth.ts.tmpl"),
        )?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires POOLSTER_TSC_JS and Node"]
    fn native_oauth_singleflight_replay_cancellation_and_redaction() {
        let api = poolster_core::Api {
            name: "OAuth".into(),
            schemas: vec![poolster_core::Schema::new(
                "Thing",
                poolster_core::SchemaValue::new(poolster_core::SchemaKind::String),
            )],
            operations: vec![poolster_core::Operation {
                id: "getThing".into(),
                method: poolster_core::HttpMethod::Get,
                path: "/thing".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let tree = poolster_core::engine::Packages::new()
            .package(crate::package("sdk").with(crate::sdk()).with(oauth()))
            .generate(&api, None)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        let cwd = dir.path().join("sdk");
        let compiler = std::env::var("POOLSTER_TSC_JS").unwrap();
        let output = std::process::Command::new("node")
            .args([&compiler, "-p", "tsconfig.json"])
            .current_dir(&cwd)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::write(
            cwd.join("probe.mjs"),
            include_str!("../tests/fixtures/oauth_probe.mjs"),
        )
        .unwrap();
        let output = std::process::Command::new("node")
            .arg("probe.mjs")
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
