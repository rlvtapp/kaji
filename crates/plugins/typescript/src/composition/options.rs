use super::*;

impl Provider {
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
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.meta = self.meta.label(label);
        self
    }
    pub fn output(mut self, module: impl Into<String>) -> Self {
        self.output = module.into();
        self
    }
    pub fn model_options(mut self, options: ModelOptions) -> Self {
        self.config.model_options = options;
        self
    }
    pub fn axios(mut self) -> Self {
        self.config.transport = sdk::SdkTransport::Axios;
        self
    }
    pub fn client_name(mut self, name: impl Into<String>) -> Self {
        self.config.client_name = Some(name.into());
        self
    }
    pub fn namespaced(mut self) -> Self {
        self.config.client_style = poolster_core::SdkClientStyle::Namespaced;
        self
    }
    pub fn throw_on_error(mut self, value: bool) -> Self {
        self.config.throw_on_error = value;
        self
    }
    pub fn using_models(mut self, handle: Handle<Models>) -> Self {
        self.models = Some(handle);
        self
    }
    pub fn using_transport(mut self, handle: Handle<Transport>) -> Self {
        self.transport = Some(handle);
        self
    }
    pub fn using_operations(mut self, handle: Handle<Operations>) -> Self {
        self.operations = Some(handle);
        self
    }
    pub fn models_handle(&self) -> Handle<Models> {
        self.meta.handle()
    }
    pub fn transport_handle(&self) -> Handle<Transport> {
        self.meta.handle()
    }
    pub fn operations_handle(&self) -> Handle<Operations> {
        self.meta.handle()
    }
    pub fn client_handle(&self) -> Handle<Client> {
        self.meta.handle()
    }
}

impl Auxiliary {
    pub fn using_graphql(self, handle: Option<Handle<crate::GraphqlClient>>) -> Self {
        self.graphql(handle)
    }
    /// Consume the actual generated GraphQL client capability.
    pub fn graphql(mut self, handle: Option<Handle<crate::GraphqlClient>>) -> Self {
        self.graphql = true;
        self.graphql_client = handle;
        self
    }
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
    pub fn layout(mut self, layout: poolster_core::SourceLayout) -> Self {
        self.layout = Some(layout);
        self
    }
    pub fn fixture_options(mut self, options: crate::FixtureOptions) -> Self {
        self.fixture_options = options;
        self
    }
    pub fn cypress_options(mut self, options: crate::CypressOptions) -> Self {
        self.cypress_options = options;
        self
    }
    /// Splits large auxiliary modules at declaration boundaries.
    pub fn max_file_bytes(mut self, bytes: usize) -> Self {
        self.max_file_bytes = bytes;
        self
    }
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.meta = self.meta.label(label);
        self
    }
    pub fn output(mut self, module: impl Into<String>) -> Self {
        self.output = module.into();
        self
    }
    pub fn using_models(mut self, handle: Handle<Models>) -> Self {
        self.models = Some(handle);
        self
    }
    pub fn using_operations(mut self, handle: Handle<Operations>) -> Self {
        self.operations = Some(handle);
        self
    }
}
