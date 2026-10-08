//! Extensible input providers publish native typed contracts for output consumers.
use std::any::{Any, TypeId};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::engine::Contract;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputOperation {
    pub name: String,
    pub kind: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputSummary {
    /// Extensible format identifier: community formats require no core enum change.
    pub format: String,
    pub title: String,
    pub version: Option<String>,
    pub types: Vec<String>,
    pub operations: Vec<InputOperation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputDiagnostic {
    pub code: String,
    pub message: String,
}

/// Native payloads use the same `Contract` trait as generator providers.
/// A parser may publish several contracts, including an `AdaptedApi` when its
/// semantics fit Poolster's HTTP model. Reading an absent capability fails explicitly.
pub struct InputContract {
    pub summary: InputSummary,
    pub diagnostics: Vec<InputDiagnostic>,
    contracts: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl InputContract {
    pub fn new(summary: InputSummary) -> Self {
        Self {
            summary,
            diagnostics: Vec::new(),
            contracts: HashMap::new(),
        }
    }

    pub fn publish<C: Contract>(&mut self, contract: C) -> Result<()> {
        ensure!(
            !self.contracts.contains_key(&TypeId::of::<C>()),
            "input contract {} was already published",
            C::NAME
        );
        self.contracts.insert(TypeId::of::<C>(), Box::new(contract));
        Ok(())
    }

    pub fn take<C: Contract>(&mut self) -> Result<C> {
        self.contracts
            .remove(&TypeId::of::<C>())
            .and_then(|contract| contract.downcast::<C>().ok())
            .map(|contract| *contract)
            .with_context(|| {
                format!(
                    "input provider did not publish required contract {}",
                    C::NAME
                )
            })
    }

    pub fn get<C: Contract>(&self) -> Result<&C> {
        self.contracts
            .get(&TypeId::of::<C>())
            .and_then(|contract| contract.downcast_ref())
            .with_context(|| {
                format!(
                    "input provider did not publish required contract {}",
                    C::NAME
                )
            })
    }
}

/// Provider-neutral source resolution options. Providers reject unsupported options.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InputOptions {
    pub operation_files: Vec<PathBuf>,
    pub import_roots: Vec<PathBuf>,
    pub broker: Option<serde_json::Value>,
    pub workflow_sources: BTreeMap<String, PathBuf>,
}

pub trait InputPlugin: Send + Sync {
    /// Unique provider identity, e.g. `graphql.apollo` or `openapi.roas`.
    fn id(&self) -> &str;
    /// Protocol/description format this plugin reads.
    fn format(&self) -> &str;
    fn load(&self, path: &Path) -> Result<InputContract>;
    fn load_with_options(&self, path: &Path, options: &InputOptions) -> Result<InputContract> {
        ensure!(
            options == &InputOptions::default(),
            "input provider {} does not support these options",
            self.id()
        );
        self.load(path)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InputPluginInfo {
    pub provider: String,
    pub format: String,
}

pub struct LoadedInput {
    pub provider: String,
    pub source: PathBuf,
    pub contract: InputContract,
}

#[derive(Default)]
pub struct InputRegistry {
    plugins: BTreeMap<String, Box<dyn InputPlugin>>,
}

fn valid_identifier(value: &str) -> bool {
    value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
}

impl InputRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<P: InputPlugin + 'static>(&mut self, plugin: P) -> Result<()> {
        ensure!(
            valid_identifier(plugin.id()),
            "invalid input provider identifier {:?}",
            plugin.id()
        );
        ensure!(
            valid_identifier(plugin.format()),
            "invalid input format identifier {:?}",
            plugin.format()
        );
        ensure!(
            !self.plugins.contains_key(plugin.id()),
            "duplicate input provider {:?}",
            plugin.id()
        );
        self.plugins
            .insert(plugin.id().to_owned(), Box::new(plugin));
        Ok(())
    }

    pub fn plugins(&self) -> Vec<InputPluginInfo> {
        self.plugins
            .values()
            .map(|plugin| InputPluginInfo {
                provider: plugin.id().to_owned(),
                format: plugin.format().to_owned(),
            })
            .collect()
    }

    /// Several providers may serve the same format; callers select the provider
    /// explicitly when automatic selection would be ambiguous.
    pub fn load(&self, format: &str, provider: Option<&str>, path: &Path) -> Result<LoadedInput> {
        self.load_with_options(format, provider, path, &InputOptions::default())
    }
    pub fn load_with_options(
        &self,
        format: &str,
        provider: Option<&str>,
        path: &Path,
        options: &InputOptions,
    ) -> Result<LoadedInput> {
        let plugin = if let Some(id) = provider {
            let plugin = self
                .plugins
                .get(id)
                .with_context(|| format!("unknown input provider {id:?}"))?;
            ensure!(
                plugin.format() == format,
                "input provider {id:?} reads {}, not {format}",
                plugin.format()
            );
            plugin.as_ref()
        } else {
            let matching: Vec<_> = self
                .plugins
                .values()
                .filter(|plugin| plugin.format() == format)
                .collect();
            ensure!(
                !matching.is_empty(),
                "no input provider registered for format {format:?}"
            );
            ensure!(
                matching.len() == 1,
                "multiple input providers for {format:?}: {}; select --provider explicitly",
                matching
                    .iter()
                    .map(|plugin| plugin.id())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            matching[0].as_ref()
        };
        let contract = plugin.load_with_options(path, options).with_context(|| {
            format!(
                "input provider {} failed reading {}",
                plugin.id(),
                path.display()
            )
        })?;
        ensure!(
            contract.summary.format == format,
            "input provider {} published format {:?}, expected {format:?}",
            plugin.id(),
            contract.summary.format
        );
        Ok(LoadedInput {
            provider: plugin.id().to_owned(),
            source: path.to_path_buf(),
            contract,
        })
    }
}

impl Contract for crate::AdaptedApi {
    const NAME: &'static str = "poolster.http-api";
}

/// Bridges a selected input contract into the ordinary provider/consumer graph.
/// Consumers declare `Requirement::on(provider.handle())` as for any renderer.
pub struct InputProvider<C: Contract> {
    registry: std::sync::Arc<InputRegistry>,
    format: String,
    provider: Option<String>,
    source: PathBuf,
    options: InputOptions,
    meta: crate::engine::Meta,
    marker: std::marker::PhantomData<fn() -> C>,
}

impl<C: Contract> InputProvider<C> {
    pub fn new(
        registry: std::sync::Arc<InputRegistry>,
        format: impl Into<String>,
        source: impl Into<PathBuf>,
    ) -> Self {
        Self {
            registry,
            format: format.into(),
            source: source.into(),
            provider: None,
            options: InputOptions::default(),
            meta: crate::engine::Meta::new(),
            marker: std::marker::PhantomData,
        }
    }
    pub fn using(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }
    pub fn with_options(mut self, options: InputOptions) -> Self {
        self.options = options;
        self
    }
    pub fn handle(&self) -> crate::engine::Handle<C> {
        self.meta.handle()
    }
}

impl<C: Contract, L: crate::engine::Language> crate::engine::Plugin<L> for InputProvider<C> {
    fn supports_native_input(&self) -> bool {
        true
    }
    fn kind(&self) -> &'static str {
        "input-provider"
    }
    fn meta(&self) -> &crate::engine::Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<crate::engine::Provision> {
        vec![crate::engine::Provision::of::<C>()]
    }
    fn generate(&self, cx: &mut crate::engine::PluginContext<'_, L>) -> Result<()> {
        let mut input = self
            .registry
            .load_with_options(
                &self.format,
                self.provider.as_deref(),
                &self.source,
                &self.options,
            )?
            .contract;
        cx.publish(input.take::<C>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Native(String);
    impl Contract for Native {
        const NAME: &'static str = "example.native";
    }
    #[derive(Debug)]
    struct Missing;
    impl Contract for Missing {
        const NAME: &'static str = "example.missing";
    }
    struct Provider(&'static str);
    impl InputPlugin for Provider {
        fn id(&self) -> &str {
            self.0
        }
        fn format(&self) -> &str {
            "custom"
        }
        fn load(&self, path: &Path) -> Result<InputContract> {
            let mut input = InputContract::new(InputSummary {
                format: "custom".into(),
                title: "Custom".into(),
                version: None,
                types: vec![],
                operations: vec![],
            });
            input.publish(Native(std::fs::read_to_string(path)?))?;
            Ok(input)
        }
    }

    #[test]
    fn community_provider_publishes_native_contract_without_core_enum_changes() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), "native contract").unwrap();
        let mut registry = InputRegistry::new();
        registry.register(Provider("custom.parser")).unwrap();
        let input = registry.load("custom", None, file.path()).unwrap();
        assert_eq!(input.contract.get::<Native>().unwrap().0, "native contract");
        assert_eq!(input.provider, "custom.parser");
        assert!(
            input
                .contract
                .get::<Missing>()
                .unwrap_err()
                .to_string()
                .contains("example.missing")
        );
    }

    #[test]
    fn replacements_need_explicit_selection_and_duplicate_ids_fail() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut registry = InputRegistry::new();
        registry.register(Provider("custom.first")).unwrap();
        registry.register(Provider("custom.second")).unwrap();
        assert!(
            registry
                .load("custom", None, file.path())
                .err()
                .unwrap()
                .to_string()
                .contains("multiple input providers")
        );
        assert!(
            registry
                .load("custom", Some("custom.second"), file.path())
                .is_ok()
        );
        assert!(registry.register(Provider("custom.first")).is_err());
        assert!(
            registry
                .load("other", Some("custom.first"), file.path())
                .is_err()
        );
        assert!(registry.load("other", None, file.path()).is_err());
        assert!(
            registry
                .load("custom", Some("unknown"), file.path())
                .is_err()
        );
    }

    #[test]
    fn registry_rejects_invalid_ids_and_false_format_claims() {
        struct Wrong;
        impl InputPlugin for Wrong {
            fn id(&self) -> &str {
                "custom.wrong"
            }
            fn format(&self) -> &str {
                "custom"
            }
            fn load(&self, _: &Path) -> Result<InputContract> {
                Ok(InputContract::new(InputSummary {
                    format: "other".into(),
                    title: String::new(),
                    version: None,
                    types: vec![],
                    operations: vec![],
                }))
            }
        }
        let mut registry = InputRegistry::new();
        assert!(registry.register(Provider("../invalid")).is_err());
        registry.register(Wrong).unwrap();
        assert!(
            registry
                .load("custom", None, Path::new("unused"))
                .err()
                .unwrap()
                .to_string()
                .contains("published format")
        );
    }

    #[test]
    fn duplicate_publications_preserve_first_contract() {
        let mut input = InputContract::new(InputSummary {
            format: "custom".into(),
            title: "Custom".into(),
            version: None,
            types: vec![],
            operations: vec![],
        });
        input.publish(Native("first".into())).unwrap();
        assert!(input.publish(Native("second".into())).is_err());
        assert_eq!(input.get::<Native>().unwrap().0, "first");
    }
}
