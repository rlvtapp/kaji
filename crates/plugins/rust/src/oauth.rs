use crate::Rust;
use anyhow::Result;
use kaji_core::{
    GeneratedFile,
    engine::{Meta, Plugin, PluginContext, Requirement},
};
pub struct OAuth {
    meta: Meta,
    transport: Option<kaji_core::engine::Handle<crate::composition::Transport>>,
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
        transport: kaji_core::engine::Handle<crate::composition::Transport>,
    ) -> Self {
        self.transport = Some(transport);
        self
    }
}
impl Plugin<Rust> for OAuth {
    fn kind(&self) -> &'static str {
        "rust-oauth"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.transport)]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Rust>) -> Result<()> {
        let transport = cx.inputs.get::<crate::composition::Transport>()?;
        anyhow::ensure!(
            transport.module == "crate::transport",
            "OAuth requires the maintained Rust transport"
        );
        cx.workspace.oauth = true;
        cx.files.emit(GeneratedFile::new(
            "src/oauth.rs",
            include_str!("oauth.rs.txt"),
        )?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires cached generated Cargo dependencies"]
    fn native_oauth_singleflight_replay_and_dropped_refresh_are_safe() {
        let api = kaji_core::Api {
            name: "OAuth".into(),
            operations: vec![
                kaji_core::Operation {
                    id: "getThing".into(),
                    method: kaji_core::HttpMethod::Get,
                    path: "/thing".into(),
                    security: vec![kaji_core::SecurityRequirement {
                        schemes: [("oauth".into(), Vec::new())].into_iter().collect(),
                    }],
                    ..Default::default()
                },
                kaji_core::Operation {
                    id: "unsafeThing".into(),
                    method: kaji_core::HttpMethod::Post,
                    path: "/thing".into(),
                    security: vec![kaji_core::SecurityRequirement {
                        schemes: [("oauth".into(), Vec::new())].into_iter().collect(),
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let tree = kaji_core::engine::Packages::new()
            .package(
                crate::package("sdk")
                    .with(crate::sdk())
                    .with(crate::operation_tests())
                    .with(oauth()),
            )
            .generate(&api, None)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        let cwd = dir.path().join("sdk");
        let path = cwd.join("src/oauth.rs");
        let source = std::fs::read_to_string(&path).unwrap() + include_str!("oauth_probe.rs.txt");
        std::fs::write(path, source).unwrap();
        let mut command = crate::native_cargo();
        let output = command
            .args(["test", "--quiet"])
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
