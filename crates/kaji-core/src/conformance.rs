//! Generic engine semantics shared by every code-generation target.
//!
//! This is intentionally a small harness rather than a translation of Kaji's
//! TypeScript runtime. It establishes behaviour every Rust target must share:
//! dependency ordering, collision detection, deterministic output and a
//! machine-readable artifact inventory.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail};

use crate::{Api, CodegenPlugin, GeneratedManifest, GeneratedTree, GeneratorConfig};

pub struct PluginInvocation<'a> {
    pub plugin: &'a dyn CodegenPlugin,
    pub config: GeneratorConfig,
    /// Names of plugins which must run before this plugin.
    pub depends_on: Vec<String>,
}

impl<'a> PluginInvocation<'a> {
    pub fn new(plugin: &'a dyn CodegenPlugin) -> Self {
        Self {
            plugin,
            config: GeneratorConfig::default(),
            depends_on: Vec::new(),
        }
    }

    pub fn with_dependency(mut self, name: impl Into<String>) -> Self {
        self.depends_on.push(name.into());
        self
    }

    pub fn with_config(mut self, config: GeneratorConfig) -> Self {
        self.config = config;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationResult {
    pub tree: GeneratedTree,
    pub plugin_order: Vec<String>,
    pub manifest: GeneratedManifest,
}

/// Deterministic observer for the native generation lifecycle.
///
/// This is the Rust equivalent of the observable ordering behind Kaji's
/// `createKaji` lifecycle: plugins generate sequentially, then every plugin
/// receives a tree-transform turn in that same resolved order. A hook error is
/// a build error and stops the remaining lifecycle; no JavaScript runtime or
/// event emitter is involved.
pub trait GenerationLifecycle {
    fn generation_start(&mut self, _plugin_order: &[String]) -> Result<()> {
        Ok(())
    }

    fn plugin_start(&mut self, _plugin: &str) -> Result<()> {
        Ok(())
    }

    fn plugin_end(&mut self, _plugin: &str, _tree: &GeneratedTree) -> Result<()> {
        Ok(())
    }

    fn transform_start(&mut self, _plugin: &str) -> Result<()> {
        Ok(())
    }

    fn transform_end(&mut self, _plugin: &str, _tree: &GeneratedTree) -> Result<()> {
        Ok(())
    }

    fn generation_end(&mut self, _tree: &GeneratedTree) -> Result<()> {
        Ok(())
    }
}

struct NoopLifecycle;

impl GenerationLifecycle for NoopLifecycle {}

/// Topologically sorts declarations while retaining declaration order whenever
/// no dependency constrains two plugins.
pub fn ordered_plugins<'a>(
    plugins: &'a [PluginInvocation<'a>],
) -> Result<Vec<&'a PluginInvocation<'a>>> {
    let mut by_name = BTreeMap::new();
    for (index, invocation) in plugins.iter().enumerate() {
        if by_name.insert(invocation.plugin.name(), index).is_some() {
            bail!(
                "plugin '{}' was declared more than once",
                invocation.plugin.name()
            );
        }
    }

    for invocation in plugins {
        for dependency in &invocation.depends_on {
            if !by_name.contains_key(dependency.as_str()) {
                bail!(
                    "plugin '{}' requires plugin '{dependency}', but it was not declared",
                    invocation.plugin.name()
                );
            }
        }
    }

    let mut ordered = Vec::with_capacity(plugins.len());
    let mut complete = BTreeSet::new();
    let mut visiting = BTreeSet::new();

    fn visit<'a>(
        index: usize,
        plugins: &'a [PluginInvocation<'a>],
        by_name: &BTreeMap<&'static str, usize>,
        complete: &mut BTreeSet<usize>,
        visiting: &mut BTreeSet<usize>,
        ordered: &mut Vec<&'a PluginInvocation<'a>>,
    ) -> Result<()> {
        if complete.contains(&index) {
            return Ok(());
        }
        let invocation = &plugins[index];
        if !visiting.insert(index) {
            bail!(
                "plugin dependency cycle includes '{}'",
                invocation.plugin.name()
            );
        }
        for dependency in &invocation.depends_on {
            visit(
                by_name[dependency.as_str()],
                plugins,
                by_name,
                complete,
                visiting,
                ordered,
            )?;
        }
        visiting.remove(&index);
        complete.insert(index);
        ordered.push(invocation);
        Ok(())
    }

    for index in 0..plugins.len() {
        visit(
            index,
            plugins,
            &by_name,
            &mut complete,
            &mut visiting,
            &mut ordered,
        )?;
    }
    Ok(ordered)
}

pub fn generate_with_plan(api: &Api, plugins: &[PluginInvocation<'_>]) -> Result<GenerationResult> {
    let mut lifecycle = NoopLifecycle;
    generate_with_plan_and_lifecycle(api, plugins, &mut lifecycle)
}

/// Runs an ordered generation plan while reporting deterministic lifecycle
/// boundaries. The existing [`generate_with_plan`] API uses an internal no-op
/// lifecycle and therefore retains its original behaviour.
pub fn generate_with_plan_and_lifecycle(
    api: &Api,
    plugins: &[PluginInvocation<'_>],
    lifecycle: &mut dyn GenerationLifecycle,
) -> Result<GenerationResult> {
    let ordered = ordered_plugins(plugins)?;
    let plugin_order = ordered
        .iter()
        .map(|invocation| invocation.plugin.name().to_owned())
        .collect::<Vec<_>>();
    lifecycle
        .generation_start(&plugin_order)
        .context("generation lifecycle `generation_start` failed")?;

    let mut tree = GeneratedTree::default();
    for invocation in &ordered {
        let plugin_name = invocation.plugin.name();
        lifecycle.plugin_start(plugin_name).with_context(|| {
            format!("generation lifecycle `plugin_start` failed for '{plugin_name}'")
        })?;
        let files = invocation
            .plugin
            .generate(api, &invocation.config)
            .with_context(|| format!("plugin '{plugin_name}' generation failed"))?;
        for file in files {
            tree.insert(file)
                .with_context(|| format!("plugin '{plugin_name}' emitted an invalid file"))?;
        }
        lifecycle.plugin_end(plugin_name, &tree).with_context(|| {
            format!("generation lifecycle `plugin_end` failed for '{plugin_name}'")
        })?;
    }

    // Deliberately separate this from the generation loop. This is the same
    // all-files-visible transform phase used by tree-aware Kaji plugins such
    // as plugin-barrel.
    for invocation in &ordered {
        let plugin_name = invocation.plugin.name();
        lifecycle.transform_start(plugin_name).with_context(|| {
            format!("generation lifecycle `transform_start` failed for '{plugin_name}'")
        })?;
        invocation
            .plugin
            .transform_tree(&mut tree, &invocation.config)
            .with_context(|| format!("plugin '{plugin_name}' tree transform failed"))?;
        lifecycle
            .transform_end(plugin_name, &tree)
            .with_context(|| {
                format!("generation lifecycle `transform_end` failed for '{plugin_name}'")
            })?;
    }

    lifecycle
        .generation_end(&tree)
        .context("generation lifecycle `generation_end` failed")?;
    let manifest = GeneratedManifest::from_tree(&tree);
    Ok(GenerationResult {
        tree,
        plugin_order,
        manifest,
    })
}
