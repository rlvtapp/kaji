use super::*;
use poolster_core::engine::{Meta, Plugin, PluginContext, Requirement};
pub struct OAuth {
    meta: Meta,
    client: Option<poolster_core::engine::Handle<providers::Client>>,
}
pub fn oauth() -> OAuth {
    OAuth {
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
    fn kind(&self) -> &'static str {
        "go-oauth"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.client)]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Go>) -> Result<()> {
        let package = go_package_name(cx.settings.package_name.as_deref().unwrap_or(&cx.api.name));
        cx.files.emit(GeneratedFile::new(
            "oauth.go",
            include_str!("oauth.go.txt").replace("__PACKAGE__", &package),
        )?)
    }
}
