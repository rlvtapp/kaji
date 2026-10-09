use super::*;
use poolster_core::engine::{Meta, Plugin, PluginContext};
pub struct OAuth {
    meta: Meta,
    pub(crate) http: poolster_core::engine::HttpInput,
}
pub fn oauth() -> OAuth {
    OAuth {
        meta: Meta::new(),
        http: Default::default(),
    }
}
impl Plugin<crate::Php> for OAuth {
    fn kind(&self) -> &'static str {
        "php-oauth"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, crate::Php>) -> Result<()> {
        self.http.run(cx, |cx| {
            let package = cx
                .settings
                .package_name
                .clone()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| format!("poolster/{}-sdk", package_slug(&cx.api.name)));
            let namespace = namespace_for_package(&package);
            cx.files.emit(GeneratedFile::new(
                "src/OAuthClient.php",
                include_str!("../templates/oauth.php.tmpl").replace("__NAMESPACE__", &namespace),
            )?)
        })
    }
    fn requires(&self) -> Vec<poolster_core::engine::Requirement> {
        self.http.requirements()
    }

    fn supports_native_input(&self) -> bool {
        self.http.is_explicit()
    }
}

impl OAuth {
    /// Select the authoritative HTTP contract produced by an input or transform.
    pub fn input(
        mut self,
        input: poolster_core::engine::Handle<poolster_core::AdaptedApi>,
    ) -> Self {
        self.http = self.http.input(input);
        self
    }
    /// Consume complete model blocks from the selected HTTP contract revision.
    pub fn input_models(
        mut self,
        models: poolster_core::engine::Handle<poolster_core::blocks::Blocks<poolster_core::Schema>>,
    ) -> Self {
        self.http = self.http.input_models(models);
        self
    }
    /// Consume complete endpoint blocks from the selected HTTP contract revision.
    pub fn input_endpoints(
        mut self,
        endpoints: poolster_core::engine::Handle<
            poolster_core::blocks::Blocks<poolster_core::Operation>,
        >,
    ) -> Self {
        self.http = self.http.input_endpoints(endpoints);
        self
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires PHP 8.2 with PSR-7/18 dependencies via POOLSTER_PHP_AUTOLOAD"]
    fn native_oauth_cache_origin_and_redaction() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("OAuthClient.php"),
            include_str!("../templates/oauth.php.tmpl").replace("__NAMESPACE__", "ProbeSdk"),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("Client.php"),
            crate::render_client(
                &poolster_core::Api::default(),
                "ProbeSdk",
                poolster_core::SdkClientStyle::Flat,
            ),
        )
        .unwrap();
        let api = poolster_core::Api {
            name: "ProbeScoped".into(),
            operations: vec![poolster_core::Operation {
                id: "getPet".into(),
                method: poolster_core::HttpMethod::Get,
                path: "/pets".into(),
                responses: vec![poolster_core::OperationResponse {
                    status: "204".into(),
                    description: None,
                    media_types: vec![],
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        crate::render_sdk(
            &api,
            ".",
            Some("probescoped"),
            poolster_core::SdkClientStyle::Namespaced,
        )
        .unwrap()
        .write_to(dir.path().join("scoped"))
        .unwrap();
        std::fs::write(
            dir.path().join("probe.php"),
            include_str!("../tests/fixtures/oauth_probe.php"),
        )
        .unwrap();
        let output = std::process::Command::new("php")
            .args(["-d", "zend.assertions=1", "-d", "assert.exception=1"])
            .arg(dir.path().join("probe.php"))
            .arg(dir.path().join("OAuthClient.php"))
            .arg(std::env::var("POOLSTER_PHP_AUTOLOAD").expect("PSR autoload path required"))
            .arg(dir.path().join("Client.php"))
            .arg(dir.path().join("scoped"))
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
