//! Portable Postman collection generation; never executes requests or writes remote workspaces.
use anyhow::{Result, bail};
use kaji_core::engine::{
    Contract, Handle, Language, Meta, Package, Plugin, PluginContext, Provision, Requirement,
};
use kaji_core::{Api, GeneratedFile, SchemaKind, SchemaValue};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
mod render;
#[cfg(test)]
mod tests;

pub struct Postman;
#[derive(Default)]
pub struct Settings {
    pub name: Option<String>,
    pub base_url: Option<String>,
}
impl Language for Postman {
    const NAME: &'static str = "postman";
    type Settings = Settings;
    type Workspace = ();
}
pub fn package(path: impl Into<String>) -> Package<Postman> {
    Package::new(path)
}
pub trait PackageExt {
    fn name(self, name: impl Into<String>) -> Self;
    fn base_url(self, url: impl Into<String>) -> Self;
}
impl PackageExt for Package<Postman> {
    fn name(mut self, name: impl Into<String>) -> Self {
        self.settings_mut().name = Some(name.into());
        self
    }
    fn base_url(mut self, url: impl Into<String>) -> Self {
        self.settings_mut().base_url = Some(url.into());
        self
    }
}
/// Values remain untrusted examples and are scrubbed before collection emission.
#[derive(Clone, Default)]
pub struct RequestExamples {
    pub requests: BTreeMap<(String, String), Value>,
    pub responses: BTreeMap<(String, String, String), Value>,
}
impl Contract for RequestExamples {
    const NAME: &'static str = "postman.request-examples";
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Diagnostic {
    pub operation: Option<String>,
    pub severity: String,
    pub code: String,
    pub message: String,
}
#[derive(Clone)]
pub struct CollectionDocument {
    pub document: Value,
    pub diagnostics: Vec<Diagnostic>,
    pub variables: BTreeMap<String, bool>,
}
impl Contract for CollectionDocument {
    const NAME: &'static str = "postman.collection";
}
#[derive(Clone)]
pub struct EnvironmentTemplate {
    pub document: Value,
}
impl Contract for EnvironmentTemplate {
    const NAME: &'static str = "postman.environment";
}

pub struct Examples {
    meta: Meta,
}
pub fn examples() -> Examples {
    Examples { meta: Meta::new() }
}
impl Examples {
    pub fn handle(&self) -> Handle<RequestExamples> {
        self.meta.handle()
    }
}
impl Plugin<Postman> for Examples {
    fn kind(&self) -> &'static str {
        "postman-examples"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<RequestExamples>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Postman>) -> Result<()> {
        cx.publish(render::example_contract(cx.api)?)
    }
}
pub struct Collection {
    meta: Meta,
    output: String,
    strict: bool,
    group_by_tag: bool,
    split_by_group: bool,
    examples: Option<Handle<RequestExamples>>,
}
pub fn collection() -> Collection {
    Collection {
        meta: Meta::new(),
        output: "collection.json".into(),
        strict: false,
        group_by_tag: true,
        split_by_group: false,
        examples: None,
    }
}
pub fn sdk() -> Collection {
    collection()
}
impl Collection {
    pub fn output(mut self, path: impl Into<String>) -> Self {
        self.output = path.into();
        self
    }
    pub fn strict(mut self, enabled: bool) -> Self {
        self.strict = enabled;
        self
    }
    pub fn group_by_tag(mut self, enabled: bool) -> Self {
        self.group_by_tag = enabled;
        self
    }
    /// Also emit one standalone collection per tag or path-resource folder.
    /// The aggregate collection and its contract remain available.
    pub fn split_by_group(mut self, enabled: bool) -> Self {
        self.split_by_group = enabled;
        self
    }
    pub fn using_examples(mut self, handle: Handle<RequestExamples>) -> Self {
        self.examples = Some(handle);
        self
    }
    pub fn handle(&self) -> Handle<CollectionDocument> {
        self.meta.handle()
    }
}
impl Plugin<Postman> for Collection {
    fn kind(&self) -> &'static str {
        "postman-collection"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on::<RequestExamples>(self.examples).optional()]
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<CollectionDocument>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Postman>) -> Result<()> {
        let document = render::collection(
            cx.api,
            cx.security_schemes,
            cx.settings,
            cx.inputs.optional::<RequestExamples>()?,
            self.group_by_tag,
        )?;
        if self.strict && document.diagnostics.iter().any(|d| d.severity == "error") {
            bail!(
                "Postman mapping incomplete: {}",
                document
                    .diagnostics
                    .iter()
                    .filter(|d| d.severity == "error")
                    .map(|d| format!(
                        "{}: {}",
                        d.operation.as_deref().unwrap_or("collection"),
                        d.message
                    ))
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        }
        cx.files.emit(GeneratedFile::new(
            &self.output,
            serde_json::to_string_pretty(&document.document)? + "\n",
        )?)?;
        if self.split_by_group {
            for (path, collection) in render::split_collections(&document.document)? {
                cx.files.emit(GeneratedFile::new(
                    path,
                    serde_json::to_string_pretty(&collection)? + "\n",
                )?)?;
            }
        }
        cx.files.emit(GeneratedFile::new(
            "diagnostics.json",
            serde_json::to_string_pretty(&document.diagnostics)? + "\n",
        )?)?;
        cx.publish(document)
    }
}
pub struct Environment {
    meta: Meta,
    output: String,
    collection: Option<Handle<CollectionDocument>>,
}
pub fn environment() -> Environment {
    Environment {
        meta: Meta::new(),
        output: "environment.json".into(),
        collection: None,
    }
}
impl Environment {
    pub fn output(mut self, path: impl Into<String>) -> Self {
        self.output = path.into();
        self
    }
    pub fn using_collection(mut self, handle: Handle<CollectionDocument>) -> Self {
        self.collection = Some(handle);
        self
    }
    pub fn handle(&self) -> Handle<EnvironmentTemplate> {
        self.meta.handle()
    }
}
impl Plugin<Postman> for Environment {
    fn kind(&self) -> &'static str {
        "postman-environment"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on::<CollectionDocument>(self.collection)]
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<EnvironmentTemplate>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Postman>) -> Result<()> {
        let collection = cx.inputs.get::<CollectionDocument>()?;
        let document = json!({"id":render::id(&format!("environment:{}",cx.api.name)),"name":cx.settings.name.as_deref().unwrap_or(&cx.api.name),"_postman_variable_scope":"environment","values":collection.variables.iter().map(|(name,secret)|json!({"key":name,"value":"","type":if *secret {"secret"} else {"default"},"enabled":true})).collect::<Vec<_>>()});
        cx.files.emit_custom(GeneratedFile::new(
            &self.output,
            serde_json::to_string_pretty(&document)? + "\n",
        )?)?;
        cx.publish(EnvironmentTemplate { document })
    }
}
/// Resolve local schema references; external references remain diagnostic failures.
fn resolve<'a>(api: &'a Api, schema: &'a SchemaValue) -> Result<&'a SchemaValue> {
    let mut current = schema;
    let mut seen = std::collections::BTreeSet::new();
    while let SchemaKind::Reference { reference } = &current.kind {
        if !seen.insert(reference) {
            bail!("cyclic schema reference {reference}");
        }
        let name = reference
            .strip_prefix("#/components/schemas/")
            .unwrap_or(reference);
        current = &api
            .schemas
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| anyhow::anyhow!("unresolved schema reference {reference}"))?
            .value;
    }
    Ok(current)
}
