//! Faker auxiliary rendering.
use super::*;

/// Emits deterministic-looking Faker factories for every reusable schema.
#[derive(Default)]
pub struct TypeScriptFaker;

impl TypeScriptFaker {
    pub(crate) fn generate_with_models(
        &self,
        api: &Api,
        config: &ArtifactOptions,
        options: &crate::ModelOptions,
    ) -> Result<Vec<GeneratedFile>> {
        self.generate(&crate::json::artifact_api(api, options), config)
    }

    pub fn generate(&self, api: &Api, config: &ArtifactOptions) -> Result<Vec<GeneratedFile>> {
        crate::auxiliary_layout::faker(&crate::auxiliary_layout::prepare(api), config)
    }
}
