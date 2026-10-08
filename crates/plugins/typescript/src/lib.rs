//! TypeScript renderers and package configuration live outside neutral core.
mod oauth;
pub use oauth::{OAuth, oauth};
mod webhooks;
pub use webhooks::{Webhooks, webhooks};
mod auxiliary_layout;
mod bundled_middleware;
mod clients;
pub mod composition;
mod esm;
mod operation_tests;
pub use operation_tests::{OperationTests, operation_tests};
mod json;
mod models;
mod render;
mod request_control;
mod sdk;
mod symbols;
mod workspace;
pub use models::{
    ArrayType, EnumConstCasing, EnumKeyCasing, EnumType, Int64Type, ModelOptions, OptionalType,
    Syntax,
};
pub use workspace::{Symbol, TsTypes, Workspace};

/// Standalone auxiliary renderers with explicit typed configuration. Plugin
/// authors can publish their files through the typed engine's emitter.
pub mod artifacts {
    pub use crate::render::{
        ArtifactOptions, McpToolManifest, Naming, ReDoc, TypeScriptCypress, TypeScriptFaker,
        TypeScriptMsw, TypeScriptReactQuery, TypeScriptSwr, TypeScriptVueQuery, TypeScriptZod,
    };
}

use anyhow::Result;
use kaji_core::engine::{
    FinalizeContext, Handle, Language, Meta, Package, Plugin, PluginContext, Provision,
};
use kaji_core::{GeneratedFile, SdkClientStyle};
use sdk::{SdkSurface, SdkTransport};
use std::path::Path;

pub struct TypeScript;
#[derive(Default)]
pub struct Settings {
    pub package_name: Option<String>,
}
impl Language for TypeScript {
    const NAME: &'static str = "typescript";
    type Settings = Settings;
    type Workspace = Workspace;
    fn finalize(cx: &mut FinalizeContext<'_, Self>) -> Result<()> {
        workspace::finalize(cx)
    }
    fn finalize_files(tree: &mut kaji_core::GeneratedTree) -> Result<()> {
        esm::finalize(tree)
    }
    fn bundle_middleware(
        tree: &mut kaji_core::GeneratedTree,
        middleware: &[kaji_core::customization::BundledMiddleware],
    ) -> Result<()> {
        bundled_middleware::bundle(tree, middleware)
    }
}
pub fn package(dir: impl Into<String>) -> Package<TypeScript> {
    Package::new(dir)
}
pub trait PackageExt {
    fn name(self, name: impl Into<String>) -> Self;
}
impl PackageExt for Package<TypeScript> {
    fn name(mut self, name: impl Into<String>) -> Self {
        self.settings_mut().package_name = Some(name.into());
        self
    }
}

/// Generates a TypeScript SDK with one selected transport and typed options.
pub struct Sdk {
    meta: Meta,
    options: sdk::SdkConfig,
    client_style: Option<SdkClientStyle>,
}
pub fn sdk() -> Sdk {
    Sdk {
        meta: Meta::new(),
        options: sdk::SdkConfig::new("__package"),
        client_style: None,
    }
}
impl Sdk {
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.meta = self.meta.label(label);
        self
    }
    pub fn fetch(mut self) -> Self {
        self.options.transport = SdkTransport::Fetch;
        self
    }
    pub fn axios(mut self) -> Self {
        self.options.transport = SdkTransport::Axios;
        self
    }
    pub fn client_name(mut self, name: impl Into<String>) -> Self {
        self.options.client_name = Some(name.into());
        self
    }
    pub fn flat(mut self) -> Self {
        self.client_style = Some(SdkClientStyle::Flat);
        self
    }
    pub fn namespaced(mut self) -> Self {
        self.client_style = Some(SdkClientStyle::Namespaced);
        self
    }
    pub fn raw(mut self) -> Self {
        self.options.surface = SdkSurface::Raw;
        self
    }
    pub fn group_by_tag(mut self, value: bool) -> Self {
        self.options.group_by_tag = value;
        self
    }
    pub fn model_options(mut self, options: ModelOptions) -> Self {
        self.options.model_options = options;
        self
    }
    /// Sets the default operation error behavior; callers may override per request.
    pub fn throw_on_error(mut self, value: bool) -> Self {
        self.options.throw_on_error = value;
        self
    }
}
impl Sdk {
    pub fn operations_handle(&self) -> Handle<composition::Operations> {
        self.meta.handle()
    }
    pub fn models_handle(&self) -> Handle<composition::Models> {
        self.meta.handle()
    }
    pub fn transport_handle(&self) -> Handle<composition::Transport> {
        self.meta.handle()
    }
}
impl Plugin<TypeScript> for Sdk {
    fn kind(&self) -> &'static str {
        "typescript-sdk"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        vec![
            Provision::of::<composition::Models>(),
            Provision::of::<composition::Transport>(),
            Provision::of::<composition::Operations>(),
        ]
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
        let mut options = self.options.clone();
        options.client_name = options
            .client_name
            .or_else(|| cx.common.client_name.clone());
        options.client_style = self
            .client_style
            .or(cx.common.client_style)
            .unwrap_or(options.client_style);
        options.package_name = cx.settings.package_name.clone();
        let tree = sdk::generate_sdk(cx.api, &options, cx.security_schemes)?;
        for (path, contents) in tree.iter() {
            let relative = path.strip_prefix("__package")?;
            let file = GeneratedFile::new(relative, contents)?;
            if matches!(
                relative.to_str(),
                Some("package.json" | "index.ts" | "tsconfig.json")
            ) {
                cx.workspace.package_file(file)?;
            } else if tree.preserves_existing(path) {
                cx.files.emit_custom(file)?;
            } else {
                cx.files.emit(file)?;
            }
        }
        let prepared_api = symbols::prepare(cx.api);
        let mut schemas = std::collections::BTreeMap::new();
        let mut operation_modules = std::collections::BTreeMap::new();
        let mut functions = std::collections::BTreeMap::new();
        let schema_files = cx
            .api
            .schemas
            .iter()
            .zip(&prepared_api.schemas)
            .map(|(schema, rendered)| {
                (
                    models::schema_file_identifier(&rendered.name),
                    (schema, rendered),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let operation_types = cx
            .api
            .operations
            .iter()
            .zip(&prepared_api.operations)
            .map(|(operation, rendered)| {
                (
                    models::operation_model_file_identifier(&rendered.id),
                    operation,
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let operation_files = cx
            .api
            .operations
            .iter()
            .zip(&prepared_api.operations)
            .map(|(operation, rendered)| {
                (
                    clients::operation_file_identifier(&rendered.id),
                    (operation, rendered),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        for (path, _) in tree.iter() {
            let relative = path.strip_prefix("__package")?;
            let stem = relative.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if relative.starts_with("models") {
                if let Some((schema, rendered)) = schema_files.get(stem) {
                    schemas.insert(
                        schema.name.clone(),
                        cx.workspace.declare(
                            relative.with_extension(""),
                            &models::model_type_name(rendered, &options.model_options),
                            self.kind(),
                        )?,
                    );
                }
                if let Some(operation) = operation_types.get(stem) {
                    operation_modules.insert(operation.id.clone(), relative.with_extension(""));
                }
            }
            if relative.starts_with("clients") {
                if let Some((operation, rendered)) = operation_files.get(stem) {
                    functions.insert(
                        operation.id.clone(),
                        cx.workspace.declare(
                            relative.with_extension(""),
                            &sdk::lower_camel_identifier(&rendered.id),
                            self.kind(),
                        )?,
                    );
                }
            }
        }
        cx.publish(composition::Models {
            schemas,
            operation_modules,
            options: options.model_options.clone(),
        })?;
        cx.workspace
            .native_transports
            .insert(".kaji/client".into(), options.transport);
        cx.publish(composition::Transport {
            module: ".kaji/client".into(),
            lossless_json: true,
        })?;
        cx.publish(composition::Operations { functions })?;
        cx.files.emit(GeneratedFile::new(
            "STYLE_GUIDE.md",
            style_guide(cx.api, &options),
        )?)
    }
}

/// A standalone model generator which publishes real schema symbols.
pub struct Types {
    meta: Meta,
    output: String,
}
pub fn types() -> Types {
    Types {
        meta: Meta::new(),
        output: "models".into(),
    }
}
impl Types {
    /// Module name without `.ts`, relative to this package.
    pub fn output(mut self, module: impl Into<String>) -> Self {
        self.output = module.into();
        self
    }
    pub fn handle(&self) -> Handle<TsTypes> {
        self.meta.handle()
    }
}
impl Plugin<TypeScript> for Types {
    fn kind(&self) -> &'static str {
        "typescript-types"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<TsTypes>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
        let mut schemas = std::collections::BTreeMap::new();
        for schema in &cx.api.schemas {
            let symbol = cx.workspace.declare(
                &self.output,
                &render::type_identifier(&schema.name),
                self.kind(),
            )?;
            if schemas.insert(schema.name.clone(), symbol).is_some() {
                anyhow::bail!("duplicate schema name {}", schema.name);
            }
        }
        let config = render::ArtifactOptions {
            output_dir: Some(".".into()),
            package_name: cx.settings.package_name.clone(),
            ..Default::default()
        };
        for file in render::TypeScriptModels.generate(cx.api, &config)? {
            cx.files.emit(GeneratedFile::new(
                format!("{}.ts", self.output),
                file.contents,
            )?)?;
        }
        for mut file in render::TypeScriptPackage.generate(cx.api, &config)? {
            if file.path == Path::new("index.ts") {
                file.contents = format!(
                    "export type * from {};\n",
                    serde_json::to_string(&format!("./{}", self.output))?
                );
            }
            if file.path == Path::new("tsconfig.json") {
                // Types can be placed in a nested module directory.
                file.contents = file.contents.replace("[\"*.ts\"]", "[\"**/*.ts\"]");
            }
            cx.workspace.package_file(file)?;
        }
        cx.publish(TsTypes { schemas })
    }
}

fn style_guide(api: &kaji_core::Api, options: &sdk::SdkConfig) -> String {
    let selected = match options.surface {
        SdkSurface::Raw => "raw exports",
        SdkSurface::Client => match options.client_style {
            SdkClientStyle::Flat => "flat instantiated client",
            SdkClientStyle::Namespaced => "namespaced instantiated client",
        },
    };
    format!(
        "# {} TypeScript SDK style guide\n\nThis package selected the **{selected}** surface. Generated models and direct operation exports are available in every mode.\n\n- `ts::sdk().raw()`: direct models and operation functions only.\n- `ts::sdk().flat()`: `client.createContact(...)`.\n- `ts::sdk().namespaced()`: `client.contacts.create(...)`.\n\nConfigure schema rendering with `ts::sdk().model_options(ts::ModelOptions {{ ..Default::default() }})`. Select Fetch or Axios through `.fetch()` or `.axios()`.\n",
        api.name
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaji_core::engine::Packages;
    use kaji_core::{
        Api, Field, HttpMethod, Operation, OperationResponse, Schema, SchemaKind, SchemaValue,
        SecurityRequirement,
    };

    fn api() -> Api {
        Api {
            name: "Contacts".into(),
            version: "1.0.0".into(),
            schemas: vec![Schema::new(
                "Contact",
                SchemaValue::new(SchemaKind::Object {
                    fields: vec![Field {
                        name: "ids".into(),
                        required: false,
                        value: SchemaValue::new(SchemaKind::Array {
                            items: Box::new(SchemaValue::new(SchemaKind::Integer)),
                        }),
                        annotations: Default::default(),
                    }],
                    additional_properties: Default::default(),
                }),
            )],
            operations: vec![Operation {
                id: "listContacts".into(),
                method: HttpMethod::Get,
                path: "/contacts".into(),
                responses: vec![OperationResponse {
                    status: "200".into(),
                    description: None,
                    media_types: vec![kaji_core::OperationMediaType {
                        content_type: "application/json".into(),
                        schema: Some(SchemaValue::reference("#/components/schemas/Contact")),
                    }],
                }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn typed_model_settings_reach_sdk_rendering() {
        let tree = Packages::new()
            .package(package("ts").with(sdk().model_options(ModelOptions {
                array_type: ArrayType::Generic,
                optional_type: OptionalType::Undefined,
                syntax: Syntax::Interface,
                integer_as_string: true,
                ..Default::default()
            })))
            .generate(&api(), None)
            .unwrap();
        let source = tree.get("ts/models/Contact.ts").unwrap();
        assert!(source.contains("export interface Contact"), "{source}");
        assert!(
            source.contains("ids: Array<string> | undefined"),
            "{source}"
        );
    }

    #[test]
    fn enum_values_are_exported_and_flat_layout_imports_the_shared_runtime() {
        let mut api = api();
        let mut status = SchemaValue::new(SchemaKind::String);
        status.enum_values = vec![serde_json::json!("active"), serde_json::json!("disabled")];
        api.schemas.push(Schema::new("Status", status));
        let tree = Packages::new()
            .package(
                package("ts").with(
                    sdk()
                        .raw()
                        .group_by_tag(false)
                        .throw_on_error(false)
                        .model_options(ModelOptions {
                            enum_type: EnumType::AsConst,
                            ..Default::default()
                        }),
                ),
            )
            .generate(&api, None)
            .unwrap();
        assert!(
            tree.get("ts/models/schemas_0001.ts")
                .unwrap()
                .contains("export * from './Status.js'")
        );
        assert!(
            tree.get("ts/models/Status.ts")
                .unwrap()
                .contains("as const")
        );
        let operation = tree.get("ts/clients/listContacts.ts").unwrap();
        assert!(operation.contains("from '../.kaji/client.js'"));
        assert!(operation.contains("ThrowOnError extends boolean = false"));
        assert!(tree.get("ts/client.ts").is_none());
    }

    #[test]
    fn dotted_component_names_keep_distinct_model_files_and_exports() {
        let mut api = api();
        api.schemas.push(Schema::new(
            "meeting.participant",
            SchemaValue::new(SchemaKind::String),
        ));
        api.schemas.push(Schema::new(
            "chat.participant",
            SchemaValue::new(SchemaKind::String),
        ));
        let tree = Packages::new()
            .package(package("ts").with(sdk().raw()))
            .generate(&api, None)
            .unwrap();
        assert!(tree.get("ts/models/MeetingParticipant.ts").is_some());
        assert!(tree.get("ts/models/ChatParticipant.ts").is_some());
        let barrel = tree.get("ts/models/schemas_0001.ts").unwrap();
        assert!(barrel.contains("./MeetingParticipant"));
        assert!(barrel.contains("./ChatParticipant"));
    }

    #[test]
    fn namespaced_sdk_keeps_the_root_facade_small_and_splits_resources() {
        let tree = Packages::new()
            .package(package("ts").with(sdk().namespaced()))
            .generate(&api(), None)
            .unwrap();
        let root = tree.get("ts/client.ts").unwrap();
        let resource = tree.get("ts/resources/contacts.ts").unwrap();
        let resource_operations = tree
            .get("ts/resources/contacts/operations_0001.ts")
            .unwrap();
        assert!(root.contains("import { ContactsClient } from './resources/contacts.js'"));
        assert!(!root.contains("import { listContacts }"));
        assert!(resource.contains("export class ContactsClient"));
        assert!(
            resource_operations
                .contains("import { listContacts } from '../../clients/contacts/listContacts.js'")
        );
        assert!(resource_operations.contains(", client }"));
        assert!(!resource_operations.contains("this.client"));
    }

    #[test]
    fn large_sdk_uses_bounded_barrels_and_composed_resource_chunks() {
        let mut source = api();
        source.operations = (0..101)
            .map(|index| Operation {
                id: format!("listContacts{index}"),
                method: HttpMethod::Get,
                path: format!("/contacts/{index}"),
                annotations: std::collections::BTreeMap::from([(
                    "tags".into(),
                    serde_json::json!(["Contacts"]),
                )]),
                ..Operation::default()
            })
            .collect();
        let tree = Packages::new()
            .package(package("ts").with(sdk().namespaced()))
            .generate(&source, None)
            .unwrap();
        let root = tree.get("ts/index.ts").unwrap();
        assert!(root.contains("export * from './models/index.js'"));
        assert!(root.contains("export * from './clients/index.js'"));
        assert!(!root.contains("listContacts100"));
        assert!(tree.get("ts/clients/contacts/operations_0002.ts").is_some());
        assert!(
            tree.get("ts/models/contacts/operation_types_0002.ts")
                .is_some()
        );
        let resource = tree.get("ts/resources/contacts.ts").unwrap();
        assert!(resource.contains("extends ContactsClientOperations0001"));
        assert!(resource.contains("ContactsClientOperations0002"));
        assert!(
            tree.get("ts/resources/contacts/operations_0002.ts")
                .is_some()
        );
    }

    #[test]
    fn query_artifacts_use_actual_operation_modules_not_a_second_transport_api() {
        let artifacts = artifacts::TypeScriptReactQuery
            .generate(&api(), &artifacts::ArtifactOptions::default())
            .unwrap();
        assert!(
            artifacts[0]
                .contents
                .contains("from \"./clients/contacts/listContacts\"")
        );
        assert!(
            artifacts[0]
                .contents
                .contains("Parameters<typeof listContacts>[0]")
        );
        assert!(!artifacts[0].contents.contains("FetchClient"));
    }

    #[test]
    fn missing_security_definitions_fail_instead_of_guessing_from_names() {
        let mut api = api();
        api.operations[0].security = vec![SecurityRequirement {
            schemes: [("myKey".into(), vec![])].into_iter().collect(),
        }];
        let error = Packages::new()
            .package(package("ts").with(sdk()))
            .generate(&api, None)
            .unwrap_err();
        assert!(format!("{error:#}").contains("definition is missing"));
    }

    fn response_contract_tree() -> kaji_core::GeneratedTree {
        let mut api = api();
        // Require a body field so structural TypeScript assignability cannot
        // mistake an open object schema with only optional fields for an envelope.
        if let SchemaKind::Object { fields, .. } = &mut api.schemas[0].value.kind {
            fields[0].required = true;
        }
        api.operations[0].responses.push(OperationResponse::json(
            "400",
            SchemaValue::new(SchemaKind::String),
        ));
        Packages::new()
            .package(package("raw").with(sdk().raw()))
            .package(package("full").with(sdk().client_name("Contacts")))
            .generate(&api, None)
            .unwrap()
    }

    #[test]
    fn operation_references_and_success_body_types_match_transport_contract() {
        let tree = response_contract_tree();
        for package in ["raw", "full"] {
            let models = tree
                .get(format!("{package}/models/contacts/ListContacts.ts"))
                .unwrap();
            assert!(models.contains("import type { Contact } from '../Contact.js'"));
            let runtime = tree.get(format!("{package}/.kaji/client.ts")).unwrap();
            assert!(runtime.contains("SuccessOf<T> = T[Extract<keyof T, `2${string}`>]"));
            assert!(
                runtime.contains("ThrowOnError extends true ? SuccessResult<T> : StatusResult<T>")
            );
            assert!(runtime.contains("security?: SecurityDescriptor[][]"));
            assert!(!runtime.contains("Array.isArray(security[0])"));
            assert!(!runtime.contains("scheme.name ?? scheme.id"));
        }
    }

    #[test]
    #[ignore = "requires Node and KAJI_TSC_JS pointing to a locally installed TypeScript compiler"]
    fn generated_fetch_consumer_compiles_with_strict_typescript() {
        let compiler =
            std::env::var("KAJI_TSC_JS").expect("set KAJI_TSC_JS to typescript/lib/tsc.js");
        let directory = std::env::temp_dir().join(format!(
            "kaji-typescript-consumer-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        let mut tree = response_contract_tree();
        tree.insert(
            kaji_core::GeneratedFile::new(
                "consumer.ts",
                r#"
import { listContacts, type Contact } from './raw/index';
import { Contacts } from './full/index';
const rawData: Contact = await listContacts({});
const fullData: Contact = await new Contacts().contacts.list({});
const errorOrSuccess = await listContacts({ throwOnError: false });
if (errorOrSuccess.status === 200) {
  const success: Contact = errorOrSuccess.data;
  console.log(success, errorOrSuccess.contentType);
}
// @ts-expect-error: status-discriminated results are not bare response bodies.
const incorrect: Contact = await listContacts({ throwOnError: false });
// @ts-expect-error: regular operations resolve to the body, not an unwrappable envelope.
await listContacts({}).unwrap();
console.log(rawData, fullData, errorOrSuccess, incorrect);
"#,
            )
            .unwrap(),
        )
        .unwrap();
        tree.write_to(&directory).unwrap();
        let output = std::process::Command::new("node")
            .arg(compiler)
            .args([
                "--noEmit",
                "--strict",
                "--target",
                "ES2022",
                "--module",
                "ESNext",
                "--moduleResolution",
                "bundler",
                "--lib",
                "ES2022,DOM,DOM.Iterable",
                "consumer.ts",
            ])
            .current_dir(&directory)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}\nGenerated fixture: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
            directory.display()
        );
        std::fs::remove_dir_all(&directory).unwrap();
    }
}

#[cfg(test)]
mod idempotency_tests;
#[cfg(test)]
mod middleware_tests;

#[cfg(test)]
mod openapi32;
