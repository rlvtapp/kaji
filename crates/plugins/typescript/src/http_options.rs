use super::*;

impl Sdk {
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
    pub fn fetch(mut self) -> Self {
        self.options.transport = SdkTransport::Fetch;
        self
    }
    pub fn axios(mut self) -> Self {
        self.options.transport = SdkTransport::Axios;
        self
    }
    pub fn client_name(mut self, name: impl Into<String>) -> Self {
        self.options.client_name = Some(name.into());
        self
    }
    pub fn flat(mut self) -> Self {
        self.client_style = Some(SdkClientStyle::Flat);
        self
    }
    pub fn namespaced(mut self) -> Self {
        self.client_style = Some(SdkClientStyle::Namespaced);
        self
    }
    pub fn raw(mut self) -> Self {
        self.options.surface = SdkSurface::Raw;
        self
    }
    pub fn group_by_tag(mut self, value: bool) -> Self {
        self.options.group_by_tag = value;
        self
    }
    pub fn model_options(mut self, options: ModelOptions) -> Self {
        self.options.model_options = options;
        self
    }
    /// Sets the default operation error behavior; callers may override per request.
    pub fn throw_on_error(mut self, value: bool) -> Self {
        self.options.throw_on_error = value;
        self
    }
}

impl Sdk {
    pub fn operations_handle(&self) -> Handle<composition::Operations> {
        self.meta.handle()
    }
    pub fn models_handle(&self) -> Handle<composition::Models> {
        self.meta.handle()
    }
    pub fn transport_handle(&self) -> Handle<composition::Transport> {
        self.meta.handle()
    }
}

impl Types {
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
    pub fn layout(mut self, layout: SourceLayout) -> Self {
        self.layout = Some(layout);
        self
    }
    /// Module name without `.ts`, relative to this package.
    pub fn output(mut self, module: impl Into<String>) -> Self {
        self.output = module.into();
        self
    }
    pub fn handle(&self) -> Handle<TsTypes> {
        self.meta.handle()
    }
}
