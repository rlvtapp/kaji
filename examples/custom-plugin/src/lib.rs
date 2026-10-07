//! Minimal provider/consumer composition without a language SDK dependency.
use anyhow::Result;
use kaji_core::{
    Api, GeneratedFile, GeneratedTree,
    engine::{
        Contract, Handle, Language, Meta, Package, Packages, Plugin, PluginContext, Provision,
        Requirement,
    },
};

pub struct Documentation;
impl Language for Documentation {
    const NAME: &'static str = "documentation";
    type Settings = ();
    type Workspace = ();
}
#[derive(Clone)]
pub struct Names(pub Vec<String>);
impl Contract for Names {
    const NAME: &'static str = "example.names";
}
pub struct Source {
    meta: Meta,
    prefix: String,
}
impl Source {
    pub fn new(prefix: &str) -> Self {
        Self {
            meta: Meta::new(),
            prefix: prefix.into(),
        }
    }
    pub fn handle(&self) -> Handle<Names> {
        self.meta.handle()
    }
}
impl Plugin<Documentation> for Source {
    fn kind(&self) -> &'static str {
        "name-provider"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<Names>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Documentation>) -> Result<()> {
        cx.publish(Names(
            cx.api
                .operations
                .iter()
                .map(|op| format!("{}{}", self.prefix, op.id))
                .collect(),
        ))
    }
}
/// A substitute owns the same contract; the consumer does not inspect provider kinds.
pub struct FixedSource {
    pub meta: Meta,
}
impl Plugin<Documentation> for FixedSource {
    fn kind(&self) -> &'static str {
        "fixed-name-provider"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<Names>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Documentation>) -> Result<()> {
        cx.publish(Names(vec!["replacement".into()]))
    }
}
pub struct Inventory {
    meta: Meta,
    source: Handle<Names>,
}
impl Inventory {
    pub fn new(source: Handle<Names>) -> Self {
        Self {
            meta: Meta::new(),
            source,
        }
    }
}
impl Plugin<Documentation> for Inventory {
    fn kind(&self) -> &'static str {
        "inventory-consumer"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(Some(self.source))]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Documentation>) -> Result<()> {
        cx.files.emit(GeneratedFile::new(
            "operations.txt",
            cx.inputs.get::<Names>()?.0.join("\n"),
        )?)
    }
}
pub fn generate(api: &Api) -> Result<GeneratedTree> {
    let ordinary = Source::new("api:");
    let replacement = FixedSource { meta: Meta::new() };
    let selected = replacement.meta.handle();
    Packages::new()
        .package(
            Package::<Documentation>::new("docs")
                .with(Inventory::new(selected))
                .with(ordinary)
                .with(replacement)
                .with(kaji_core::api_reference::api_reference()),
        )
        .generate(api, None)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_substitute_drives_consumer_and_reference_emits() {
        let tree = generate(&Api::default()).unwrap();
        assert_eq!(tree.get("docs/operations.txt"), Some("replacement"));
        assert!(tree.get("docs/API_REFERENCE.md").is_some());
    }
}
