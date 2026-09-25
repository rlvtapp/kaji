use std::collections::BTreeMap;

use anyhow::Result;

use crate::{Api, GeneratedFile, GeneratedTree};

/// Per-plugin configuration after config-file parsing and defaulting.
pub type GeneratorConfig = BTreeMap<String, String>;

/// A target-specific code generator.
///
/// Built-in generators are Rust implementations. A future WASM boundary can
/// expose the same input and output shapes to third-party plugins.
pub trait CodegenPlugin: Send + Sync {
    fn name(&self) -> &'static str;

    fn generate(&self, api: &Api, config: &GeneratorConfig) -> Result<Vec<GeneratedFile>>;

    /// Runs after every plugin has emitted its ordinary files. This supports
    /// deterministic tree-aware generators such as Kaji-style barrels without
    /// coupling normal generators to one another.
    fn transform_tree(&self, _tree: &mut GeneratedTree, _config: &GeneratorConfig) -> Result<()> {
        Ok(())
    }
}

/// Runs plugins in declaration order and rejects conflicting output paths.
pub fn generate(
    api: &Api,
    plugins: &[(&dyn CodegenPlugin, GeneratorConfig)],
) -> Result<GeneratedTree> {
    let mut output = GeneratedTree::default();
    for (plugin, config) in plugins {
        for file in plugin.generate(api, config)? {
            output.insert(file)?;
        }
    }
    for (plugin, config) in plugins {
        plugin.transform_tree(&mut output, config)?;
    }
    Ok(output)
}
