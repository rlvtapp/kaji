//! Independently selectable SDK providers. Contracts describe emitted modules,
//! so consumers never reconstruct imports from operation IDs or output recipes.
use crate::{ModelOptions, Symbol, TypeScript, render, sdk};
use anyhow::{Context, Result};
use kaji_core::{
    GeneratedFile,
    engine::{Contract, Handle, Meta, Plugin, PluginContext, Provision, Requirement},
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct Models {
    pub schemas: BTreeMap<String, Symbol>,
    pub operation_modules: BTreeMap<String, PathBuf>,
    pub options: ModelOptions,
}
impl Contract for Models {
    const NAME: &'static str = "typescript.sdk-models";
}
/// A module implementing Kaji's runtime ABI: Options, RequestResult,
/// ResponseResult, ClientConfig, ClientInstance, client, createClient,
/// resolveResponse and stream. Community transports may publish this contract.
#[derive(Clone)]
pub struct Transport {
    pub module: PathBuf,
    /// The runtime consumes schema-directed JSON plans without losing digits.
    pub lossless_json: bool,
}
impl Transport {
    pub fn new(module: impl Into<PathBuf>) -> Self {
        Self {
            module: module.into(),
            lossless_json: false,
        }
    }
    pub fn lossless_json(mut self, supported: bool) -> Self {
        self.lossless_json = supported;
        self
    }
}
impl Contract for Transport {
    const NAME: &'static str = "typescript.transport";
}
#[derive(Clone)]
pub struct Operations {
    pub functions: BTreeMap<String, Symbol>,
}
impl Contract for Operations {
    const NAME: &'static str = "typescript.operations";
}
#[derive(Clone)]
pub struct Client {
    pub symbol: Symbol,
}
impl Contract for Client {
    const NAME: &'static str = "typescript.client";
}

#[derive(Clone, Copy)]
enum Part {
    Models,
    Transport,
    Operations,
    Client,
}
/// Uses the same maintained renderers as `sdk()`, emitting only its owned part.
pub struct Provider {
    meta: Meta,
    part: Part,
    config: sdk::SdkConfig,
    output: String,
    models: Option<Handle<Models>>,
    transport: Option<Handle<Transport>>,
    operations: Option<Handle<Operations>>,
}
fn provider(part: Part, output: &str) -> Provider {
    let mut config = sdk::SdkConfig::new("__package");
    config.group_by_tag = false;
    config.client_style = kaji_core::SdkClientStyle::Flat;
    Provider {
        meta: Meta::new(),
        part,
        config,
        output: output.into(),
        models: None,
        transport: None,
        operations: None,
    }
}
pub fn models() -> Provider {
    provider(Part::Models, "models")
}
pub fn transport() -> Provider {
    provider(Part::Transport, ".kaji/client")
}
pub fn operations() -> Provider {
    provider(Part::Operations, "operations")
}
pub fn client() -> Provider {
    provider(Part::Client, "client")
}
impl Provider {
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.meta = self.meta.label(label);
        self
    }
    pub fn output(mut self, module: impl Into<String>) -> Self {
        self.output = module.into();
        self
    }
    pub fn model_options(mut self, options: ModelOptions) -> Self {
        self.config.model_options = options;
        self
    }
    pub fn axios(mut self) -> Self {
        self.config.transport = sdk::SdkTransport::Axios;
        self
    }
    pub fn client_name(mut self, name: impl Into<String>) -> Self {
        self.config.client_name = Some(name.into());
        self
    }
    pub fn namespaced(mut self) -> Self {
        self.config.client_style = kaji_core::SdkClientStyle::Namespaced;
        self
    }
    pub fn throw_on_error(mut self, value: bool) -> Self {
        self.config.throw_on_error = value;
        self
    }
    pub fn using_models(mut self, handle: Handle<Models>) -> Self {
        self.models = Some(handle);
        self
    }
    pub fn using_transport(mut self, handle: Handle<Transport>) -> Self {
        self.transport = Some(handle);
        self
    }
    pub fn using_operations(mut self, handle: Handle<Operations>) -> Self {
        self.operations = Some(handle);
        self
    }
    pub fn models_handle(&self) -> Handle<Models> {
        self.meta.handle()
    }
    pub fn transport_handle(&self) -> Handle<Transport> {
        self.meta.handle()
    }
    pub fn operations_handle(&self) -> Handle<Operations> {
        self.meta.handle()
    }
    pub fn client_handle(&self) -> Handle<Client> {
        self.meta.handle()
    }
}
fn module_import(module: &Path, file: &Path) -> Result<String> {
    Symbol {
        module: module.into(),
        name: String::new(),
    }
    .import_from(file)
}
impl Plugin<TypeScript> for Provider {
    fn kind(&self) -> &'static str {
        match self.part {
            Part::Models => "typescript-models",
            Part::Transport => "typescript-transport",
            Part::Operations => "typescript-operations",
            Part::Client => "typescript-client",
        }
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        vec![match self.part {
            Part::Models => Provision::of::<Models>(),
            Part::Transport => Provision::of::<Transport>(),
            Part::Operations => Provision::of::<Operations>(),
            Part::Client => Provision::of::<Client>(),
        }]
    }
    fn requires(&self) -> Vec<Requirement> {
        match self.part {
            Part::Operations => vec![
                Requirement::on(self.models),
                Requirement::on(self.transport),
            ],
            Part::Client => vec![
                Requirement::on(self.operations),
                Requirement::on(self.transport),
            ],
            _ => vec![],
        }
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
        let output = GeneratedFile::new(&self.output, "")?.path;
        let mut config = self.config.clone();
        config.package_name = cx.settings.package_name.clone();
        if let Part::Operations = self.part {
            config.model_options = cx.inputs.get::<Models>()?.options.clone();
            anyhow::ensure!(
                !(config.model_options.integer_as_string
                    || config.model_options.int64_type != crate::Int64Type::Number)
                    || cx.inputs.get::<Transport>()?.lossless_json,
                "selected TypeScript transport does not support schema-directed lossless JSON; select a compatible transport or numeric model representation"
            );
        }
        let mut tree = kaji_core::GeneratedTree::default();
        match self.part {
            Part::Models => {
                for file in crate::models::ModelRenderer.generate(
                    cx.api,
                    &crate::models::ModelRenderOptions {
                        output_dir: "__package/models".into(),
                        schema_output_dir: None,
                        operation_output_dir: None,
                        group_by_tag: false,
                        model: config.model_options.clone(),
                    },
                )? {
                    tree.insert(file)?;
                }
                tree.insert(GeneratedFile::new(
                    "__package/package.json",
                    sdk::kaji_package(
                        cx.api,
                        sdk::SdkTransport::Fetch,
                        cx.settings.package_name.as_deref(),
                    )?,
                )?)?;
                tree.insert(GeneratedFile::new("__package/tsconfig.json", r#"{"compilerOptions":{"declaration":true,"module":"ESNext","moduleResolution":"Bundler","outDir":"dist","strict":true,"skipLibCheck":true,"target":"ES2022"},"include":["**/*.ts"]}"#)?)?;
            }
            Part::Transport => {
                tree.insert(GeneratedFile::new(
                    "__package/.kaji/client.ts",
                    sdk::kaji_runtime(config.transport, cx.security_schemes),
                )?)?;
            }
            Part::Operations => {
                for file in crate::clients::generate_operations(
                    cx.api,
                    &crate::clients::ClientRenderOptions {
                        model_options: Some(config.model_options.clone()),
                        output_dir: "__package/clients".into(),
                        throw_on_error: config.throw_on_error,
                        group_by_tag: false,
                        group_default_directory: false,
                        type_import_prefix: Some("../models".into()),
                        runtime_import_prefix: Some("..".into()),
                        runtime_dir: ".kaji".into(),
                    },
                    cx.security_schemes,
                )? {
                    tree.insert(file)?;
                }
            }
            Part::Client => {
                let name = config
                    .client_name
                    .clone()
                    .unwrap_or_else(|| sdk::sdk_client_name(&cx.api.name));
                for file in sdk::kaji_sdk_client(
                    cx.api,
                    &name,
                    false,
                    config.client_style,
                    "__package",
                    ".kaji",
                )? {
                    tree.insert(file)?;
                }
            }
        }
        let mut emitted_modules = Vec::new();
        let mut schema_symbols = BTreeMap::new();
        let mut operation_modules = BTreeMap::new();
        let mut functions = BTreeMap::new();
        for (path, source) in tree.iter() {
            let path = path.strip_prefix("__package")?;
            let selected = match self.part {
                Part::Models => {
                    path.starts_with("models") && path.extension().is_some_and(|e| e == "ts")
                }
                Part::Transport => path == Path::new(".kaji/client.ts"),
                Part::Operations => {
                    path.starts_with("clients")
                        && cx.api.operations.iter().any(|o| {
                            path.file_stem().is_some_and(|s| {
                                s == crate::clients::operation_file_identifier(&o.id).as_str()
                            })
                        })
                }
                Part::Client => path == Path::new("client.ts") || path.starts_with("resources"),
            };
            if !selected {
                continue;
            }
            let target = match self.part {
                Part::Models => output.join(path.strip_prefix("models")?),
                Part::Operations => output.join(path.strip_prefix("clients")?),
                Part::Client if path.starts_with("resources") => {
                    output.parent().unwrap_or(Path::new("")).join(path)
                }
                _ => PathBuf::from(format!("{}.ts", output.display())),
            };
            let mut contents = source.to_owned();
            if matches!(self.part, Part::Operations | Part::Client) {
                let transport = cx.inputs.get::<Transport>()?;
                let import = module_import(&transport.module, &target)?;
                let original = module_import(Path::new(".kaji/client"), path)?;
                contents = contents.replace(&format!("'{original}'"), &format!("'{import}'"));
            }
            if let Part::Operations = self.part {
                let operation = cx
                    .api
                    .operations
                    .iter()
                    .find(|o| {
                        path.file_stem().is_some_and(|s| {
                            s == crate::clients::operation_file_identifier(&o.id).as_str()
                        })
                    })
                    .context("operation module has no contract operation")?;
                let models = cx.inputs.get::<Models>()?;
                let model = models
                    .operation_modules
                    .get(&operation.id)
                    .context("model provider omitted operation")?;
                let old = format!(
                    "'../models/{}'",
                    crate::models::operation_model_file_identifier(&operation.id)
                );
                contents = contents.replace(&old, &format!("'{}'", module_import(model, &target)?));
                functions.insert(
                    operation.id.clone(),
                    cx.workspace.declare(
                        target.with_extension(""),
                        &sdk::lower_camel_identifier(&operation.id),
                        self.kind(),
                    )?,
                );
            }
            if let Part::Client = self.part {
                let operations = cx.inputs.get::<Operations>()?;
                for operation in &cx.api.operations {
                    let symbol = operations
                        .functions
                        .get(&operation.id)
                        .context("operation provider omitted operation")?;
                    let old = format!(
                        "'{}'",
                        module_import(
                            Path::new(&format!(
                                "clients/{}",
                                crate::clients::operation_file_identifier(&operation.id)
                            )),
                            path
                        )?
                    );
                    contents =
                        contents.replace(&old, &format!("'{}'", symbol.import_from(&target)?));
                    let original = sdk::lower_camel_identifier(&operation.id);
                    if symbol.name != original {
                        contents = contents.replace(
                            &format!("import {{ {original} }}"),
                            &format!("import {{ {} as {original} }}", symbol.name),
                        );
                    }
                }
            }
            if let Part::Models = self.part {
                if let Some(schema) = cx.api.schemas.iter().find(|schema| {
                    target.file_stem().is_some_and(|stem| {
                        stem == crate::models::schema_file_identifier(&schema.name).as_str()
                    })
                }) {
                    let name = crate::models::model_type_name(schema, &config.model_options);
                    schema_symbols.insert(
                        schema.name.clone(),
                        cx.workspace
                            .declare(target.with_extension(""), &name, self.kind())?,
                    );
                }
                if let Some(operation) = cx.api.operations.iter().find(|operation| {
                    target.file_stem().is_some_and(|stem| {
                        stem == crate::models::operation_model_file_identifier(&operation.id)
                            .as_str()
                    })
                }) {
                    operation_modules.insert(operation.id.clone(), target.with_extension(""));
                }
            }
            emitted_modules.push(target.with_extension(""));
            cx.files.emit(GeneratedFile::new(target, contents)?)?;
        }
        match self.part {
            Part::Models | Part::Operations => {
                let barrel = output.join("_exports.ts");
                let contents = emitted_modules
                    .iter()
                    .map(|module| {
                        Ok(format!(
                            "export * from {};\n",
                            serde_json::to_string(&module_import(module, &barrel)?)?
                        ))
                    })
                    .collect::<Result<Vec<_>>>()?
                    .join("");
                cx.files.emit(GeneratedFile::new(
                    &barrel,
                    format!("export {{}}\n{contents}"),
                )?)?;
                cx.workspace.export_namespace(
                    barrel.with_extension(""),
                    &sdk::lower_camel_identifier(&self.output),
                )?;
            }
            Part::Transport => {
                cx.workspace
                    .export_namespace(&output, &sdk::lower_camel_identifier(&self.output))?;
            }
            Part::Client => {
                cx.workspace.export(&output)?;
            }
        }
        match self.part {
            Part::Models => {
                for path in ["package.json", "tsconfig.json"] {
                    cx.workspace.package_file(GeneratedFile::new(
                        path,
                        tree.get(format!("__package/{path}"))
                            .context("missing package file")?,
                    )?)?;
                }
                cx.publish(Models {
                    schemas: schema_symbols,
                    operation_modules,
                    options: config.model_options,
                })
            }
            Part::Transport => {
                if config.transport == sdk::SdkTransport::Axios {
                    cx.workspace.dependency("axios", "^1.7.0")?;
                }
                cx.publish(Transport {
                    module: output,
                    lossless_json: true,
                })
            }
            Part::Operations => cx.publish(Operations { functions }),
            Part::Client => {
                let name = config
                    .client_name
                    .unwrap_or_else(|| sdk::sdk_client_name(&cx.api.name));
                let symbol = cx.workspace.declare(output, &name, self.kind())?;
                cx.publish(Client { symbol })
            }
        }
    }
}

pub enum QueryFramework {
    React,
    Vue,
    Swr,
}
pub struct Query {
    meta: Meta,
    framework: QueryFramework,
    provider: Option<Handle<Operations>>,
    output: String,
}
pub fn react_query() -> Query {
    Query {
        meta: Meta::new(),
        framework: QueryFramework::React,
        provider: None,
        output: "react-query".into(),
    }
}
pub fn vue_query() -> Query {
    Query {
        framework: QueryFramework::Vue,
        output: "vue-query".into(),
        ..react_query()
    }
}
pub fn swr() -> Query {
    Query {
        framework: QueryFramework::Swr,
        output: "swr".into(),
        ..react_query()
    }
}
impl Query {
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.meta = self.meta.label(label);
        self
    }
    pub fn using_operations(mut self, handle: Handle<Operations>) -> Self {
        self.provider = Some(handle);
        self
    }
    pub fn output(mut self, module: impl Into<String>) -> Self {
        self.output = module.into();
        self
    }
}
impl Plugin<TypeScript> for Query {
    fn kind(&self) -> &'static str {
        "typescript-query"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.provider)]
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
        let config = render::ArtifactOptions {
            output_dir: Some(".".into()),
            group_by_tag: false,
            ..Default::default()
        };
        let (files, dependency, version) = match self.framework {
            QueryFramework::React => (
                render::TypeScriptReactQuery.generate(cx.api, &config)?,
                "@tanstack/react-query",
                "^5.0.0",
            ),
            QueryFramework::Vue => (
                render::TypeScriptVueQuery.generate(cx.api, &config)?,
                "@tanstack/vue-query",
                "^5.0.0",
            ),
            QueryFramework::Swr => (
                render::TypeScriptSwr.generate(cx.api, &config)?,
                "swr",
                "^2.0.0",
            ),
        };
        let target = GeneratedFile::new(format!("{}.ts", self.output), "")?.path;
        let operations = cx.inputs.get::<Operations>()?;
        for file in files {
            let mut contents = file.contents;
            for operation in &cx.api.operations {
                if matches!(self.framework, QueryFramework::Swr)
                    && operation.method != kaji_core::HttpMethod::Get
                {
                    continue;
                }
                let symbol = operations
                    .functions
                    .get(&operation.id)
                    .context("query plugin requires operation symbol")?;
                let function = sdk::lower_camel_identifier(&operation.id);
                let old = serde_json::to_string(&format!(
                    "{}/{function}",
                    config.clients_import.trim_end_matches('/')
                ))?;
                contents =
                    contents.replace(&old, &serde_json::to_string(&symbol.import_from(&target)?)?);
                if symbol.name != function {
                    contents = contents.replace(
                        &format!("import {{ {function} }}"),
                        &format!("import {{ {} as {function} }}", symbol.name),
                    );
                }
            }
            cx.files.emit(GeneratedFile::new(&target, contents)?)?;
        }
        cx.workspace.peer_dependency(dependency, version)?;
        cx.workspace
            .export_namespace(&self.output, &sdk::lower_camel_identifier(&self.output))
    }
}

/// Auxiliary artifacts tied to the selected model and operation providers.
pub struct Auxiliary {
    meta: Meta,
    kind: AuxiliaryKind,
    output: String,
    models: Option<Handle<Models>>,
    operations: Option<Handle<Operations>>,
}
enum AuxiliaryKind {
    Zod,
    Faker,
    Msw,
    Cypress,
}
fn auxiliary(kind: AuxiliaryKind, output: &str) -> Auxiliary {
    Auxiliary {
        meta: Meta::new(),
        kind,
        output: output.into(),
        models: None,
        operations: None,
    }
}
pub fn zod() -> Auxiliary {
    auxiliary(AuxiliaryKind::Zod, "zod")
}
pub fn faker() -> Auxiliary {
    auxiliary(AuxiliaryKind::Faker, "faker")
}
pub fn msw() -> Auxiliary {
    auxiliary(AuxiliaryKind::Msw, "msw")
}
pub fn cypress() -> Auxiliary {
    auxiliary(AuxiliaryKind::Cypress, "cypress")
}
impl Auxiliary {
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.meta = self.meta.label(label);
        self
    }
    pub fn output(mut self, module: impl Into<String>) -> Self {
        self.output = module.into();
        self
    }
    pub fn using_models(mut self, handle: Handle<Models>) -> Self {
        self.models = Some(handle);
        self
    }
    pub fn using_operations(mut self, handle: Handle<Operations>) -> Self {
        self.operations = Some(handle);
        self
    }
}
impl Plugin<TypeScript> for Auxiliary {
    fn kind(&self) -> &'static str {
        match self.kind {
            AuxiliaryKind::Zod => "typescript-zod",
            AuxiliaryKind::Faker => "typescript-faker",
            AuxiliaryKind::Msw => "typescript-msw",
            AuxiliaryKind::Cypress => "typescript-cypress",
        }
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        let mut requirements = vec![Requirement::on(self.models)];
        if matches!(self.kind, AuxiliaryKind::Msw | AuxiliaryKind::Cypress) {
            requirements.push(Requirement::on(self.operations));
        }
        requirements
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
        let models = cx.inputs.get::<Models>()?;
        let target = GeneratedFile::new(format!("{}.ts", self.output), "")?.path;
        let config = render::ArtifactOptions {
            output_dir: Some(".".into()),
            ..Default::default()
        };
        let (files, dependency, version) = match self.kind {
            AuxiliaryKind::Zod => (
                render::TypeScriptZod.generate_with_models(cx.api, &config, &models.options)?,
                "zod",
                "^4.0.0",
            ),
            AuxiliaryKind::Faker => (
                render::TypeScriptFaker.generate_with_models(cx.api, &config, &models.options)?,
                "@faker-js/faker",
                "^9.0.0",
            ),
            AuxiliaryKind::Msw => (
                render::TypeScriptMsw.generate(cx.api, &config)?,
                "msw",
                "^2.0.0",
            ),
            AuxiliaryKind::Cypress => (
                render::TypeScriptCypress.generate(cx.api, &config)?,
                "cypress",
                "^15.0.0",
            ),
        };
        for file in files {
            let mut source = file.contents;
            if matches!(self.kind, AuxiliaryKind::Faker) {
                let mut imports = String::new();
                for schema in &cx.api.schemas {
                    let symbol = models
                        .schemas
                        .get(&schema.name)
                        .context("model provider omitted schema")?;
                    let local = render::type_identifier(&schema.name);
                    imports.push_str(&format!(
                        "import type {{ {}{} }} from {};\n",
                        symbol.name,
                        if symbol.name == local {
                            String::new()
                        } else {
                            format!(" as {local}")
                        },
                        serde_json::to_string(&symbol.import_from(&target)?)?
                    ));
                }
                // The legacy renderer emits one consolidated model import;
                // the native consumer imports each actual provider symbol.
                source = source
                    .lines()
                    .filter(|line| {
                        !(line.starts_with("import type {") && line.ends_with("from './models';"))
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                source = format!("{imports}{source}\n");
            }
            if matches!(self.kind, AuxiliaryKind::Msw | AuxiliaryKind::Cypress) {
                let operations = cx.inputs.get::<Operations>()?;
                for operation in &cx.api.operations {
                    anyhow::ensure!(
                        operations.functions.contains_key(&operation.id),
                        "operation provider omitted {}",
                        operation.id
                    );
                }
            }
            cx.files.emit(GeneratedFile::new(&target, source)?)?;
        }
        if matches!(self.kind, AuxiliaryKind::Cypress) {
            cx.workspace.dev_dependency(dependency, version)?;
        } else {
            cx.workspace.dependency(dependency, version)?;
        }
        if !matches!(self.kind, AuxiliaryKind::Cypress) {
            cx.workspace
                .export_namespace(&self.output, &sdk::lower_camel_identifier(&self.output))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaji_core::{Api, HttpMethod, Operation, engine::Packages};
    fn api() -> Api {
        Api {
            name: "Contacts API".into(),
            version: "1.0.0".into(),
            operations: vec![Operation {
                id: "listContacts".into(),
                method: HttpMethod::Get,
                path: "/contacts".into(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }
    struct CustomTransport {
        meta: Meta,
    }
    impl Plugin<TypeScript> for CustomTransport {
        fn kind(&self) -> &'static str {
            "custom-transport"
        }
        fn meta(&self) -> &Meta {
            &self.meta
        }
        fn provides(&self) -> Vec<Provision> {
            vec![Provision::of::<Transport>()]
        }
        fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
            let config = sdk::SdkConfig::new("__package");
            let tree = sdk::generate_sdk(cx.api, &config, cx.security_schemes)?;
            cx.files.emit(GeneratedFile::new(
                "custom/request.ts",
                tree.get("__package/.kaji/client.ts").unwrap(),
            )?)?;
            cx.publish(Transport {
                module: "custom/request".into(),
                lossless_json: true,
            })
        }
    }
    #[test]
    fn relocated_providers_and_custom_transport_compose() {
        let tree = Packages::new()
            .package(
                crate::package("ts")
                    .with(models().output("domain/types"))
                    .with(CustomTransport { meta: Meta::new() })
                    .with(operations().output("api/calls"))
                    .with(client().output("api/client"))
                    .with(react_query().output("ui/queries")),
            )
            .generate(&api(), None)
            .unwrap();
        let call = tree.get("ts/api/calls/listContacts.ts").unwrap();
        assert!(call.contains("from '../../custom/request'"), "{call}");
        assert!(
            call.contains("from '../../domain/types/ListContacts'"),
            "{call}"
        );
        let client = tree.get("ts/api/client.ts").unwrap();
        assert!(client.contains("export class Contacts"), "{client}");
        assert!(client.contains("from './calls/listContacts'"), "{client}");
        let query = tree.get("ts/ui/queries.ts").unwrap();
        assert!(
            query.contains("from \"../api/calls/listContacts\""),
            "{query}"
        );
        assert!(tree.get("ts/.kaji/client.ts").is_none());
        let manifest: serde_json::Value =
            serde_json::from_str(tree.get("ts/package.json").unwrap()).unwrap();
        assert_eq!(
            manifest["peerDependencies"]["@tanstack/react-query"],
            "^5.0.0"
        );
    }
    #[test]
    fn unsupported_lossless_transport_fails_before_emitting_operations() {
        struct Unsupported {
            meta: Meta,
        }
        impl Plugin<TypeScript> for Unsupported {
            fn kind(&self) -> &'static str {
                "unsupported-runtime"
            }
            fn meta(&self) -> &Meta {
                &self.meta
            }
            fn provides(&self) -> Vec<Provision> {
                vec![Provision::of::<Transport>()]
            }
            fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
                cx.publish(Transport::new("foreign/runtime"))
            }
        }
        let error = Packages::new()
            .package(
                crate::package("ts")
                    .with(models().model_options(ModelOptions {
                        integer_as_string: true,
                        ..Default::default()
                    }))
                    .with(Unsupported { meta: Meta::new() })
                    .with(operations()),
            )
            .generate(&api(), None)
            .unwrap_err();
        assert!(format!("{error:#}").contains("does not support schema-directed lossless JSON"));
    }
    #[test]
    fn sdk_publishes_real_grouped_operation_symbols_for_consumers() {
        let sdk = crate::sdk().raw();
        let query = vue_query().using_operations(sdk.operations_handle());
        let tree = Packages::new()
            .package(crate::package("ts").with(query).with(sdk))
            .generate(&api(), None)
            .unwrap();
        assert!(
            tree.get("ts/vue-query.ts")
                .unwrap()
                .contains("from \"./clients/contacts/listContacts\"")
        );
    }
    #[test]
    fn ambiguous_transport_requires_an_explicit_binding() {
        let first = transport().output("runtime/first");
        let handle = first.transport_handle();
        let tree = Packages::new()
            .package(
                crate::package("ts")
                    .with(models())
                    .with(first)
                    .with(transport().output("runtime/second"))
                    .with(operations().using_transport(handle)),
            )
            .generate(&api(), None)
            .unwrap();
        assert!(
            tree.get("ts/operations/listContacts.ts")
                .unwrap()
                .contains("../runtime/first")
        );
        let error = Packages::new()
            .package(
                crate::package("ts")
                    .with(models())
                    .with(transport().output("runtime/first"))
                    .with(transport().output("runtime/second"))
                    .with(operations()),
            )
            .generate(&api(), None)
            .unwrap_err();
        assert!(format!("{error:#}").contains("Select a provider handle explicitly"));
    }
    #[test]
    #[ignore = "requires KAJI_TSC_JS and KAJI_TS_NODE_MODULES"]
    fn custom_transport_and_relocated_query_consumer_compile() {
        let compiler = std::env::var("KAJI_TSC_JS").unwrap();
        let modules = std::env::var("KAJI_TS_NODE_MODULES").unwrap();
        let directory =
            std::env::temp_dir().join(format!("kaji-ts-compose-{}", std::process::id()));
        let tree = Packages::new()
            .package(
                crate::package("ts")
                    .with(models().output("domain/types"))
                    .with(CustomTransport { meta: Meta::new() })
                    .with(operations().output("api/calls"))
                    .with(client().output("api/client"))
                    .with(react_query().output("ui/queries")),
            )
            .generate(&api(), None)
            .unwrap();
        tree.write_to(&directory).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(modules, directory.join("ts/node_modules")).unwrap();
        std::fs::write(directory.join("ts/consumer.ts"), "import { Contacts } from './api/client'; import { useListContacts } from './ui/queries'; const result = new Contacts().listContacts({}); console.log(result, useListContacts);").unwrap();
        let result = std::process::Command::new("node")
            .arg(compiler)
            .args(["--noEmit", "--project"])
            .arg(directory.join("ts/tsconfig.json"))
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(directory);
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }

    #[test]
    #[ignore = "requires KAJI_TSC_JS and KAJI_TS_NODE_MODULES with auxiliary dependencies"]
    fn namespaced_and_auxiliary_consumers_compile_and_validate_selected_models() {
        let compiler = std::env::var("KAJI_TSC_JS").unwrap();
        let modules = std::env::var("KAJI_TS_NODE_MODULES").unwrap();
        let directory = std::env::temp_dir().join(format!("kaji-ts-aux-{}", std::process::id()));
        let mut api = api();
        let mut id = kaji_core::SchemaValue::new(kaji_core::SchemaKind::Integer);
        id.format = Some("int64".into());
        let mut status = kaji_core::SchemaValue::new(kaji_core::SchemaKind::String);
        status.enum_values = vec![serde_json::json!("active"), serde_json::json!("disabled")];
        api.schemas = vec![
            kaji_core::Schema::new("Status", status),
            kaji_core::Schema::new(
                "Contact",
                kaji_core::SchemaValue::new(kaji_core::SchemaKind::Object {
                    fields: vec![
                        kaji_core::Field {
                            name: "id".into(),
                            value: id,
                            required: true,
                            annotations: Default::default(),
                        },
                        kaji_core::Field {
                            name: "status".into(),
                            value: kaji_core::SchemaValue::reference("#/components/schemas/Status"),
                            required: true,
                            annotations: Default::default(),
                        },
                    ],
                    additional_properties: Default::default(),
                }),
            ),
        ];
        api.operations[0].responses = vec![kaji_core::OperationResponse::json(
            "200",
            kaji_core::SchemaValue::reference("#/components/schemas/Contact"),
        )];
        let tree = Packages::new()
            .package(
                crate::package("ts")
                    .with(
                        models()
                            .output("domain/models")
                            .model_options(ModelOptions {
                                int64_type: crate::Int64Type::BigInt,
                                enum_type: crate::EnumType::AsConst,
                                ..Default::default()
                            }),
                    )
                    .with(transport().output("network/runtime"))
                    .with(operations().output("api/calls"))
                    .with(client().namespaced().output("facade/client"))
                    .with(react_query().output("hooks/react"))
                    .with(vue_query().output("hooks/vue"))
                    .with(swr().output("hooks/swr"))
                    .with(zod().output("validation/schemas"))
                    .with(faker().output("fixtures/factories"))
                    .with(msw().output("fixtures/handlers"))
                    .with(cypress().output("tests/smoke")),
            )
            .generate(&api, None)
            .unwrap();
        tree.write_to(&directory).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(modules, directory.join("ts/node_modules")).unwrap();
        std::fs::write(directory.join("ts/consumer.ts"),"import { Contacts } from './facade/client'; import { createContact } from './fixtures/factories'; import { ContactSchema } from './validation/schemas'; const contact = createContact(); ContactSchema.parse(contact); const id: bigint = contact.id; new Contacts().contacts.list({}); console.log(id);").unwrap();
        let result = std::process::Command::new("node")
            .arg(compiler)
            .args(["--project"])
            .arg(directory.join("ts/tsconfig.json"))
            .args([
                "--module",
                "commonjs",
                "--moduleResolution",
                "node",
                "--outDir",
                "compiled",
            ])
            .current_dir(directory.join("ts"))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        std::fs::write(
            directory.join("ts/compiled/package.json"),
            "{\"type\":\"commonjs\"}",
        )
        .unwrap();
        std::fs::write(directory.join("ts/test.cjs"),"const assert = require('node:assert/strict'); const {ContactSchema} = require('./compiled/validation/schemas.js'); const {createContact} = require('./compiled/fixtures/factories.js'); const contact = createContact(); assert.equal(typeof contact.id,'bigint'); assert.equal(ContactSchema.parse(contact).id,contact.id); assert.throws(()=>ContactSchema.parse({...contact,id:'1'}));").unwrap();
        let result = std::process::Command::new("node")
            .arg(directory.join("ts/test.cjs"))
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(&directory);
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
