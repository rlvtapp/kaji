use crate::Rust;
use anyhow::Result;
use poolster_core::{
    GeneratedFile,
    engine::{Meta, Plugin, PluginContext, Requirement},
};
pub struct OAuth {
    http_input: poolster_core::engine::HttpInput,
    meta: Meta,
    transport: Option<poolster_core::engine::Handle<crate::composition::Transport>>,
}
pub fn oauth() -> OAuth {
    OAuth {
        http_input: Default::default(),
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
}
impl Plugin<Rust> for OAuth {
    fn supports_native_input(&self) -> bool {
        self.http_input.is_explicit()
    }
    fn kind(&self) -> &'static str {
        "rust-oauth"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        let mut requirements = self.http_input.requirements();
        requirements.extend(vec![Requirement::on(self.transport)]);
        requirements
    }
    fn generate(&self, cx: &mut PluginContext<'_, Rust>) -> Result<()> {
        self.http_input.with_context(cx, |cx| {
            let transport = cx.inputs.get::<crate::composition::Transport>()?;
            anyhow::ensure!(
                transport.module == "crate::transport",
                "OAuth requires the maintained Rust transport"
            );
            cx.workspace.oauth = true;
            cx.files.emit(GeneratedFile::new(
                "src/oauth.rs",
                include_str!("../templates/oauth.rs.tmpl"),
            )?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires cached generated Cargo dependencies"]
    fn native_oauth_singleflight_replay_and_dropped_refresh_are_safe() {
        let api = poolster_core::Api {
            name: "OAuth".into(),
            operations: vec![
                poolster_core::Operation {
                    id: "getThing".into(),
                    method: poolster_core::HttpMethod::Get,
                    path: "/thing".into(),
                    security: vec![poolster_core::SecurityRequirement {
                        schemes: [("oauth".into(), Vec::new())].into_iter().collect(),
                    }],
                    ..Default::default()
                },
                poolster_core::Operation {
                    id: "unsafeThing".into(),
                    method: poolster_core::HttpMethod::Post,
                    path: "/thing".into(),
                    security: vec![poolster_core::SecurityRequirement {
                        schemes: [("oauth".into(), Vec::new())].into_iter().collect(),
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let tree = poolster_core::engine::Packages::new()
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
        let source = std::fs::read_to_string(&path).unwrap()
            + include_str!("../tests/fixtures/oauth_probe.rs");
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

#[path = "oauth_input.rs"]
mod http_input;
