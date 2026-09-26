//! TypeScript renderers and package configuration live outside neutral core.
pub mod clients;
pub mod models;
pub mod render;
pub mod sdk;
mod workspace;
pub use workspace::{Symbol, TsTypes, Workspace};

use anyhow::Result;
use kaji_core::engine::{
    FinalizeContext, Handle, Language, Meta, Package, Plugin, PluginContext, Provision,
};
use kaji_core::{CodegenPlugin, GeneratedFile, GeneratorConfig, SdkClientStyle};
pub use sdk::{SdkStyle, SdkSurface, SdkTransport};
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeScriptOptions {
    pub client_name: Option<String>,
    pub client_style: SdkClientStyle,
    pub surface: SdkSurface,
    pub group_by_tag: bool,
}
impl Default for TypeScriptOptions {
    fn default() -> Self {
        Self {
            client_name: None,
            client_style: SdkClientStyle::Namespaced,
            surface: SdkSurface::Client,
            group_by_tag: true,
        }
    }
}
impl TypeScriptOptions {
    pub fn raw() -> Self {
        Self {
            surface: SdkSurface::Raw,
            ..Self::default()
        }
    }
    pub fn flat_client() -> Self {
        Self {
            client_style: SdkClientStyle::Flat,
            ..Self::default()
        }
    }
}

/// Complete SDK wrapper during migration. Fetch/Axios are local typed options.
/// Use separate packages for variants until operation contracts are extracted.
pub struct Sdk {
    meta: Meta,
    transport: SdkTransport,
    options: TypeScriptOptions,
    client_style: Option<SdkClientStyle>,
}
pub fn sdk() -> Sdk {
    Sdk {
        meta: Meta::new(),
        transport: SdkTransport::Fetch,
        options: TypeScriptOptions::default(),
        client_style: None,
    }
}
impl Sdk {
    pub fn fetch(mut self) -> Self {
        self.transport = SdkTransport::Fetch;
        self
    }
    pub fn axios(mut self) -> Self {
        self.transport = SdkTransport::Axios;
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
}
impl Plugin<TypeScript> for Sdk {
    fn kind(&self) -> &'static str {
        "typescript-sdk"
    }
    fn meta(&self) -> &Meta {
        &self.meta
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
        let profile = sdk::SdkProfile {
            package_name: cx.settings.package_name.clone(),
            client_name: options.client_name.clone(),
            client_style: options.client_style,
            surface: options.surface,
            group_by_tag: options.group_by_tag,
            transports: vec![self.transport],
            ..sdk::SdkProfile::typescript("__package")
        };
        let tree = sdk::generate_sdk(cx.api, &profile, cx.security_schemes)?;
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
        let config = GeneratorConfig::from([
            ("output_dir".into(), ".".into()),
            ("clients".into(), "".into()),
        ]);
        let mut config = config;
        if let Some(name) = &cx.settings.package_name {
            config.insert("package_name".into(), name.clone());
        }
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

pub fn style_guide(api: &kaji_core::Api, options: &TypeScriptOptions) -> String {
    let selected = match options.surface {
        SdkSurface::Raw => "raw exports",
        SdkSurface::Client => match options.client_style {
            SdkClientStyle::Flat => "flat instantiated client",
            SdkClientStyle::Namespaced => "namespaced instantiated client",
        },
    };
    format!(
        "# {} TypeScript SDK style guide\n\nThis package selected the **{selected}** surface. Generated models and direct operation exports remain available in every mode.\n\n- `SdkSurface::Raw`: direct models and operation functions only.\n- `SdkClientStyle::Flat`: `client.createContact(...)`.\n- `SdkClientStyle::Namespaced`: `client.contacts.create(...)`.\n\nConfigure the package with `TypeScriptOptions` in `kaji`.\n",
        api.name
    )
}
