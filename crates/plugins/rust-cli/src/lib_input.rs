//! Explicit HTTP contract and block selection.
use super::*;

impl Cli {
    pub fn input(
        mut self,
        value: poolster_core::engine::Handle<poolster_core::AdaptedApi>,
    ) -> Self {
        self.http_input = self.http_input.input(value);
        self
    }
    pub fn input_models(
        mut self,
        value: poolster_core::engine::Handle<poolster_core::blocks::Blocks<poolster_core::Schema>>,
    ) -> Self {
        self.http_input = self.http_input.input_models(value);
        self
    }
    pub fn input_endpoints(
        mut self,
        value: poolster_core::engine::Handle<
            poolster_core::blocks::Blocks<poolster_core::Operation>,
        >,
    ) -> Self {
        self.http_input = self.http_input.input_endpoints(value);
        self
    }
}
