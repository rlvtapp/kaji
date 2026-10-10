//! Native fixed-operation GraphQL Postman collections and editable environments.
#[cfg(test)]
mod tests;
mod variables;
use super::{CollectionDocument, Diagnostic, EnvironmentTemplate, Postman, render};
use anyhow::{Result, bail, ensure};
use poolster_core::{
    GeneratedFile,
    engine::{Handle, Meta, Plugin, PluginContext, Provision, Requirement},
    native::{GraphqlOperationKind, GraphqlOperations, ModelField, ModelKind, ModelType},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use variables::*;
pub struct GraphqlCollection {
    meta: Meta,
    input: Option<Handle<GraphqlOperations>>,
    output: String,
    strict: bool,
    group_by_kind: bool,
    split_by_group: bool,
    variables: BTreeMap<String, Value>,
}
pub fn graphql() -> GraphqlCollection {
    GraphqlCollection {
        meta: Meta::new(),
        input: None,
        output: "collection.json".into(),
        strict: true,
        group_by_kind: true,
        split_by_group: false,
        variables: BTreeMap::new(),
    }
}
impl GraphqlCollection {
    pub fn input(mut self, input: Handle<GraphqlOperations>) -> Self {
        self.input = Some(input);
        self
    }
    pub fn output(mut self, path: impl Into<String>) -> Self {
        self.output = path.into();
        self
    }
    pub fn strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }
    pub fn group_by_kind(mut self, enabled: bool) -> Self {
        self.group_by_kind = enabled;
        self
    }
    /// Native GraphQL has operation kinds rather than HTTP tags.
    pub fn group_by_tag(self, enabled: bool) -> Self {
        self.group_by_kind(enabled)
    }
    pub fn split_by_group(mut self, enabled: bool) -> Self {
        self.split_by_group = enabled;
        self
    }
    /// Provide explicit JSON variables, including custom scalar values, for an operation.
    pub fn variables(mut self, operation: impl Into<String>, value: Value) -> Self {
        self.variables.insert(operation.into(), value);
        self
    }
    pub fn variable_defaults(mut self, variables: BTreeMap<String, Value>) -> Self {
        self.variables = variables;
        self
    }
    pub fn handle(&self) -> Handle<CollectionDocument> {
        self.meta.handle()
    }
}
impl Plugin<Postman> for GraphqlCollection {
    fn kind(&self) -> &'static str {
        "postman-graphql"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.input)]
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<CollectionDocument>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Postman>) -> Result<()> {
        let collection = collection(
            cx.inputs.get::<GraphqlOperations>()?,
            cx.settings,
            self.group_by_kind,
            &self.variables,
        )?;
        if self.strict && !collection.diagnostics.is_empty() {
            bail!(
                "Postman GraphQL mapping incomplete: {}",
                collection
                    .diagnostics
                    .iter()
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
            serde_json::to_string_pretty(&collection.document)? + "\n",
        )?)?;
        if self.split_by_group {
            for (path, document) in render::split_collections(&collection.document)? {
                cx.files.emit(GeneratedFile::new(
                    path,
                    serde_json::to_string_pretty(&document)? + "\n",
                )?)?;
            }
        }
        cx.files.emit(GeneratedFile::new(
            "diagnostics.json",
            serde_json::to_string_pretty(&collection.diagnostics)? + "\n",
        )?)?;
        cx.files.emit(GeneratedFile::new("README.md","# GraphQL Postman collection\n\nImport collection.json and environment.json. Set base_url to your GraphQL HTTP endpoint. Each operation has a JSON variables environment entry; required variables use illustrative values, optional variables are omitted. Edit the full JSON object to supply optional inputs or explicit null. Operation defaults remain in the document and are applied by the server.\n\nQuery and mutation requests use Postman's native graphql body mode with explicit operationName. The test hook validates the response envelope and sets poolster_graphql_status to success, partial, error or protocol_error, and poolster_graphql_errors to the returned errors JSON. GraphQL errors can occur on HTTP200; partial data is retained in the response. These checks validate protocol shape, not application success.\n\nSubscriptions need a separate streaming transport and are rejected by default. Explicit strict(false) skips them with diagnostics.json; an all-subscription input emits an empty collection with the report. Required recursive/custom-scalar inputs without a trustworthy sample are diagnosed and strict mode rejects them. Environment files inherit the explicitly configured endpoint, remain editable, and omit schema default values. Sensitive JSON variable fields are replaced by placeholders; operation documents are retained exactly, including any user-written literals. HTTP auth can be configured in Postman separately.\n")?)?;
        cx.publish(collection)
    }
}
pub struct GraphqlEnvironment {
    meta: Meta,
    collection: Option<Handle<CollectionDocument>>,
    output: String,
}
pub fn graphql_environment() -> GraphqlEnvironment {
    GraphqlEnvironment {
        meta: Meta::new(),
        collection: None,
        output: "environment.json".into(),
    }
}
impl GraphqlEnvironment {
    pub fn using_collection(mut self, handle: Handle<CollectionDocument>) -> Self {
        self.collection = Some(handle);
        self
    }
    pub fn output(mut self, path: impl Into<String>) -> Self {
        self.output = path.into();
        self
    }
    pub fn handle(&self) -> Handle<EnvironmentTemplate> {
        self.meta.handle()
    }
}
impl Plugin<Postman> for GraphqlEnvironment {
    fn kind(&self) -> &'static str {
        "postman-graphql-environment"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.collection)]
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<EnvironmentTemplate>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Postman>) -> Result<()> {
        let collection = cx.inputs.get::<CollectionDocument>()?;
        let defaults = collection.document["variable"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Postman collection has no variables"))?;
        let values=collection.variables.iter().map(|(name,secret)|{let value=defaults.iter().find(|v|v["key"].as_str()==Some(name)).and_then(|v|v.get("value")).cloned().unwrap_or(Value::String(String::new()));json!({"key":name,"value":value,"type":if *secret{"secret"}else{"default"},"enabled":true})}).collect::<Vec<_>>();
        let name = cx
            .settings
            .name
            .as_deref()
            .or(collection.document["info"]["name"].as_str())
            .unwrap_or("GraphQL");
        let document = json!({"id":render::id(&format!("graphql-environment:{name}")),"name":name,"_postman_variable_scope":"environment","values":values});
        cx.files.emit_custom(GeneratedFile::new(
            &self.output,
            serde_json::to_string_pretty(&document)? + "\n",
        )?)?;
        cx.publish(EnvironmentTemplate { document })
    }
}
fn collection(
    c: &GraphqlOperations,
    settings: &super::Settings,
    group: bool,
    configured: &BTreeMap<String, Value>,
) -> Result<CollectionDocument> {
    ensure!(
        !c.operations.is_empty(),
        "Postman GraphQL requires validated operation documents"
    );
    for name in configured.keys() {
        ensure!(
            c.operations.iter().any(|op| &op.name == name),
            "Configured variables reference unknown operation {name}"
        );
    }
    let name = settings.name.as_deref().unwrap_or("GraphQL");
    let mut names = BTreeSet::new();
    let mut diagnostics = Vec::new();
    let mut items = BTreeMap::<String, Vec<Value>>::new();
    let mut flat = Vec::new();
    let mut variables = BTreeMap::from([("base_url".into(), false)]);
    let mut defs = vec![
        json!({"key":"base_url","value":settings.base_url.as_deref().unwrap_or(""),"type":"string"}),
    ];
    for op in &c.operations {
        ensure!(
            !op.name.is_empty() && names.insert(op.name.clone()),
            "Postman GraphQL operation names must be nonempty and unique"
        );
        ensure!(
            !op.document.trim().is_empty(),
            "Postman GraphQL operation {} has no document",
            op.name
        );
        if op.kind == GraphqlOperationKind::Subscription {
            diagnostics.push(Diagnostic{operation:Some(op.name.clone()),severity:"warning".into(),code:"unsupported_subscription".into(),message:"Subscriptions require a streaming GraphQL transport; no HTTP request was emitted.".into()});
            continue;
        }
        let key = format!(
            "graphql_variables_{}",
            render::id(&op.name).replace('-', "")
        );
        let mut seen = BTreeSet::new();
        let mut sample = if let Some(value) = configured.get(&op.name) {
            validate_fields(&op.variables, value, c, 0)?;
            value.clone()
        } else {
            fields(&op.variables,c,&mut seen,0).unwrap_or_else(|error|{diagnostics.push(Diagnostic{operation:Some(op.name.clone()),severity:"warning".into(),code:"variables_require_configuration".into(),message:format!("Required variables cannot be safely illustrated: {error}. Configure variables(operation, JSON) or enable strict(false) and fill the operation variables environment entry.")});json!({})})
        };
        scrub(&mut sample);
        variables.insert(key.clone(), op.variables.iter().any(|f| sensitive(&f.name)));
        defs.push(
            json!({"key":key,"value":serde_json::to_string_pretty(&sample)?,"type":"string"}),
        );
        let request = json!({"id":render::id(&format!("graphql:{}",op.name)),"name":op.name,"request":{"method":"POST","header":[{"key":"Content-Type","value":"application/json"},{"key":"Accept","value":"application/graphql-response+json, application/json"}],"url":"{{base_url}}","body":{"mode":"graphql","graphql":{"query":op.document,"operationName":op.name,"variables":format!("{{{{{key}}}}}")}},"description":"Fixed validated GraphQL operation. Edit its variables JSON environment entry. Optional fields are omitted by default; JSON null is explicit null. Inspect data and errors together, including partial results."},"response":[]});
        if group {
            items
                .entry(
                    match op.kind {
                        GraphqlOperationKind::Query => "Query",
                        _ => "Mutation",
                    }
                    .into(),
                )
                .or_default()
                .push(request);
        } else {
            flat.push(request);
        }
    }
    let items = if group {
        items
            .into_iter()
            .map(|(name, item)| json!({"name":name,"item":item}))
            .collect::<Vec<_>>()
    } else {
        flat
    };
    let document = json!({"info":{"_postman_id":render::id(&format!("graphql-collection:{name}")),"name":name,"schema":"https://schema.getpostman.com/json/collection/v2.1.0/collection.json","description":"Fixed-operation GraphQL HTTP collection. Inspect GraphQL errors independently of HTTP status; partial results retain data and errors. Subscriptions are unsupported."},"item":items,"variable":defs,"event":[{"listen":"test","script":{"type":"text/javascript","exec":include_str!("envelope.js").lines().collect::<Vec<_>>()}}]});
    Ok(CollectionDocument {
        document,
        diagnostics,
        variables,
    })
}
