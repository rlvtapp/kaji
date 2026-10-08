use super::*;
use poolster_core::engine::{Handle, Meta, Plugin, PluginContext, Requirement};
pub struct OAuth {
    meta: Meta,
    models: Option<Handle<crate::package::RubyModels>>,
}
pub fn oauth() -> OAuth {
    OAuth {
        meta: Meta::new(),
        models: None,
    }
}
impl OAuth {
    pub fn models_from(self, sdk: &crate::package::Sdk) -> Self {
        self.using_models(sdk.models())
    }
    pub fn using_models(mut self, models: Handle<crate::package::RubyModels>) -> Self {
        self.models = Some(models);
        self
    }
}
impl Plugin<crate::Ruby> for OAuth {
    fn kind(&self) -> &'static str {
        "ruby-oauth"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.models)]
    }
    fn generate(&self, cx: &mut PluginContext<'_, crate::Ruby>) -> Result<()> {
        let models = cx.inputs.get::<crate::package::RubyModels>()?;
        cx.files.emit(GeneratedFile::new(
            format!("lib/{}/oauth.rb", models.import),
            include_str!("../templates/oauth.rb.txt")
                .replace("__MODULE__", &models.module)
                .replace("__IMPORT__", &models.import),
        )?)?;
        cx.files.emit(GeneratedFile::new("OAUTH.md",format!("Require `{}/oauth`, then create `{}::OAuthClientCredentials.new(token_url: 'https://issuer.example/token', client_id: ENV.fetch('CLIENT_ID'), client_secret: ENV.fetch('CLIENT_SECRET'))` and pass `token_provider: provider` to the client. Client-credentials Basic authentication, bounded JSON response and thread-coordinated expiry refresh are supported. Safe calls can replay a rejected token once; unsafe unkeyed POST/PATCH cannot. The client's `cancelled:` callback cancels waiting and publication of a refreshed token; active synchronous transport interruption remains driver-owned. Errors and inspect omit secrets/payloads. No refresh-token, authorization-code, or PKCE flow is implied.",models.import,models.module))?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_oauth_singleflight_cancellation_expiry_and_bounded401() {
        let api = Api {
            name: "Probe".into(),
            operations: vec![
                Operation {
                    id: "deleteThing".into(),
                    method: poolster_core::HttpMethod::Delete,
                    path: "/thing".into(),
                    responses: vec![poolster_core::OperationResponse {
                        status: "204".into(),
                        description: None,
                        media_types: vec![],
                    }],
                    ..Default::default()
                },
                Operation {
                    id: "unsafeCreate".into(),
                    method: poolster_core::HttpMethod::Post,
                    path: "/thing".into(),
                    responses: vec![poolster_core::OperationResponse {
                        status: "204".into(),
                        description: None,
                        media_types: vec![],
                    }],
                    ..Default::default()
                },
                Operation {
                    id: "retryThing".into(),
                    method: poolster_core::HttpMethod::Post,
                    path: "/thing".into(),
                    annotations: BTreeMap::from([(
                        "x-poolster-idempotency".into(),
                        serde_json::json!({"header":"X-Once","auto_generate":true}),
                    )]),
                    responses: vec![poolster_core::OperationResponse {
                        status: "204".into(),
                        description: None,
                        media_types: vec![],
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let tree = poolster_core::engine::Packages::new()
            .package(crate::package("sdk").with(crate::sdk()).with(oauth()))
            .generate(&api, None)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        std::fs::write(
            dir.path().join("sdk/probe.rb"),
            include_str!("../tests/fixtures/oauth_probe.rb.txt"),
        )
        .unwrap();
        let output = std::process::Command::new("ruby")
            .args(["-Ilib", "probe.rb"])
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
