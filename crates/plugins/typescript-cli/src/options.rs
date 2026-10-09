use super::*;

impl Cli {
    /// Select an owned HTTP input contract rather than the package's legacy API.
    pub fn input(
        mut self,
        handle: poolster_core::engine::Handle<poolster_core::AdaptedApi>,
    ) -> Self {
        self.http_input = self.http_input.input(handle);
        self
    }
    /// Select complete schema blocks derived from the selected HTTP input.
    pub fn input_models(
        mut self,
        handle: poolster_core::engine::Handle<poolster_core::blocks::Blocks<poolster_core::Schema>>,
    ) -> Self {
        self.http_input = self.http_input.input_models(handle);
        self
    }
    /// Select complete endpoint blocks derived from the selected HTTP input.
    pub fn input_endpoints(
        mut self,
        handle: poolster_core::engine::Handle<
            poolster_core::blocks::Blocks<poolster_core::Operation>,
        >,
    ) -> Self {
        self.http_input = self.http_input.input_endpoints(handle);
        self
    }
    pub fn command_name(mut self, value: impl Into<String>) -> Self {
        self.command_name = Some(value.into());
        self
    }
    pub fn base_url(mut self, value: impl Into<String>) -> Self {
        self.base_url = Some(value.into());
        self
    }
    pub fn oauth(mut self, value: OAuthConfig) -> Self {
        self.oauth = Some(value);
        self
    }
}

impl OAuthConfig {
    pub fn client_id(mut self, value: impl Into<String>) -> Self {
        self.client_id = value.into();
        self
    }
    pub fn security_scheme(mut self, value: impl Into<String>) -> Self {
        self.security_scheme = Some(value.into());
        self
    }
    pub fn scopes(mut self, values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.scopes = values.into_iter().map(Into::into).collect();
        self
    }
    pub fn preferred_flow(mut self, value: impl Into<String>) -> Self {
        self.preferred_flow = Some(value.into());
        self
    }
    pub fn authorization_url(mut self, value: impl Into<String>) -> Self {
        self.authorization_url = Some(value.into());
        self
    }
    pub fn device_authorization_url(mut self, value: impl Into<String>) -> Self {
        self.device_authorization_url = Some(value.into());
        self
    }
    pub fn token_url(mut self, value: impl Into<String>) -> Self {
        self.token_url = Some(value.into());
        self
    }
    pub fn redirect_uri(mut self, value: impl Into<String>) -> Self {
        self.redirect_uri = Some(value.into());
        self
    }
}
