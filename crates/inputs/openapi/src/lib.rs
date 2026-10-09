//! OpenAPI compiler-artifact input boundary, preserving existing HTTP behavior.
pub mod blocks;
pub mod compiler;
pub mod contracts;
pub mod openapi_sidecar;
use anyhow::Result;
pub use openapi_sidecar::OpenApiSidecar;
use poolster_core::{
    AdaptedApi,
    input::{InputContract, InputOperation, InputPlugin, InputSummary},
};
use std::path::Path;

/// Publish the whole HTTP contract and independently consumable rich blocks.
pub fn publish(adapted: AdaptedApi, source: &str) -> Result<InputContract> {
    let mut input = InputContract::new(InputSummary {
        format: "openapi".into(),
        title: adapted.api.name.clone(),
        version: Some(adapted.api.version.clone()),
        types: adapted.api.schemas.iter().map(|s| s.name.clone()).collect(),
        operations: adapted
            .api
            .operations
            .iter()
            .map(|op| InputOperation {
                name: op.id.clone(),
                kind: "http".into(),
            })
            .collect(),
    });
    let reference = poolster_core::blocks::ContractReference::from_bytes(
        <AdaptedApi as poolster_core::engine::Contract>::NAME,
        source,
        &serde_json::to_vec(&adapted)?,
    );
    input.publish(blocks::models(&adapted, source).with_parent(reference.clone()))?;
    input.publish(blocks::endpoints(&adapted, source).with_parent(reference.clone()))?;
    input.publish_with_reference(adapted, reference)?;
    Ok(input)
}

/// Reads directories emitted by the existing embedded OpenAPI compiler.
/// Raw YAML/JSON compilation remains the responsibility of the existing CLI.
pub struct OpenApiInput;
impl InputPlugin for OpenApiInput {
    fn id(&self) -> &str {
        "openapi.compiler-artifacts"
    }
    fn format(&self) -> &str {
        "openapi"
    }
    fn load(&self, path: &Path) -> Result<InputContract> {
        let adapted = OpenApiSidecar::new(path, "Api", "0.0.0").load()?;
        publish(adapted, &path.to_string_lossy())
    }
}

/// Raw document provider using the caller's existing official Poolster compiler.
/// Register explicitly alongside artifact providers, then select its provider ID.
pub struct OpenApiCompilerInput {
    pub executable: std::path::PathBuf,
    pub name: String,
    pub version: String,
}
impl InputPlugin for OpenApiCompilerInput {
    fn id(&self) -> &str {
        "openapi.compiler"
    }
    fn format(&self) -> &str {
        "openapi"
    }
    fn load(&self, path: &Path) -> Result<InputContract> {
        let output = tempfile::tempdir()?;
        compiler::compile(&self.executable, path, output.path(), None)?;
        let adapted = OpenApiSidecar::new(output.path(), &self.name, &self.version).load()?;
        publish(adapted, &path.to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use poolster_core::{Api, Operation, Schema, SchemaKind, SchemaValue, blocks::Blocks};
    #[test]
    fn whole_and_blocks_preserve_rich_http_semantics() {
        let mut value = SchemaValue::new(SchemaKind::String);
        value.nullable = true;
        value.optional = true;
        value.constraints.insert("minLength".into(), 3.into());
        let adapted = AdaptedApi {
            api: Api {
                schemas: vec![Schema::new("Name", value)],
                operations: vec![Operation {
                    id: "getName".into(),
                    path: "/name".into(),
                    ..Default::default()
                }],
                ..Default::default()
            },
            ..Default::default()
        };
        let input = publish(adapted.clone(), "spec").unwrap();
        assert_eq!(input.get::<AdaptedApi>().unwrap(), &adapted);
        assert_eq!(
            input.get::<Blocks<Schema>>().unwrap().items[0].value,
            adapted.api.schemas[0]
        );
        assert_eq!(
            input.get::<Blocks<Operation>>().unwrap().items[0]
                .metadata
                .id
                .local,
            "GET /name"
        );
    }
}
