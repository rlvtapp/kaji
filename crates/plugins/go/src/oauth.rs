use super::*;
use poolster_core::engine::{Meta, Plugin, PluginContext, Requirement};
pub struct OAuth {
    http_input: poolster_core::engine::HttpInput,
    meta: Meta,
    client: Option<poolster_core::engine::Handle<providers::Client>>,
}
pub fn oauth() -> OAuth {
    OAuth {
        http_input: Default::default(),
        meta: Meta::new(),
        client: None,
    }
}
impl OAuth {
    pub fn using_client(
        mut self,
        client: poolster_core::engine::Handle<providers::Client>,
    ) -> Self {
        self.client = Some(client);
        self
    }
    pub fn client_from(self, sdk: &crate::Sdk) -> Self {
        self.using_client(sdk.client())
    }
}
impl Plugin<Go> for OAuth {
    fn supports_native_input(&self) -> bool {
        self.http_input.is_explicit()
    }
    fn kind(&self) -> &'static str {
        "go-oauth"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        let mut requirements = self.http_input.requirements();
        requirements.extend(vec![Requirement::on(self.client)]);
        requirements
    }
    fn generate(&self, cx: &mut PluginContext<'_, Go>) -> Result<()> {
        self.http_input.with_context(cx, |cx| {
            let package =
                go_package_name(cx.settings.package_name.as_deref().unwrap_or(&cx.api.name));
            cx.files.emit(GeneratedFile::new(
                "oauth.go",
                include_str!("../templates/oauth.go.tmpl").replace("__PACKAGE__", &package),
            )?)
        })
    }
}

#[path = "oauth_input.rs"]
mod http_input;
