use super::*;

impl Query {
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
    /// Select original operation IDs without changing the operation provider.
    pub fn include_operations(mut self, ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.include = Some(ids.into_iter().map(Into::into).collect());
        self
    }
    pub fn operation_kind(mut self, id: impl Into<String>, kind: QueryKind) -> Self {
        self.kinds.insert(id.into(), kind);
        self
    }
    pub fn operation_name(mut self, id: impl Into<String>, name: impl Into<String>) -> Self {
        self.names.insert(id.into(), name.into());
        self
    }
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.meta = self.meta.label(label);
        self
    }
    pub fn using_operations(mut self, handle: Handle<Operations>) -> Self {
        self.provider = Some(handle);
        self
    }
    /// Bound generated helper modules; zero is rejected during generation.
    pub fn max_operations_per_file(mut self, count: usize) -> Self {
        self.operations_per_file = Some(count);
        self
    }
    /// Preserve one aggregate helper module for callers that explicitly prefer it.
    pub fn single_file(mut self) -> Self {
        self.operations_per_file = None;
        self.layout = Some(poolster_core::SourceLayout::SingleFile);
        self
    }
    pub fn output(mut self, module: impl Into<String>) -> Self {
        self.output = module.into();
        self
    }
}
