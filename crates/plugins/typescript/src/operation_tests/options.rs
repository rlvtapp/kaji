use super::*;

impl OperationTests {
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
    pub fn using_models(mut self, handle: Handle<Models>) -> Self {
        self.models = Some(handle);
        self
    }
    pub fn using_operations(mut self, handle: Handle<Operations>) -> Self {
        self.operations = Some(handle);
        self
    }
    pub fn using_transport(mut self, handle: Handle<Transport>) -> Self {
        self.transport = Some(handle);
        self
    }
    pub fn sample_options(mut self, options: poolster_core::samples::SampleOptions) -> Self {
        self.options = options;
        self
    }
    pub fn max_operations(mut self, limit: usize) -> Self {
        self.max_operations = limit;
        self
    }
}
