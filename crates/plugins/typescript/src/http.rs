//! HTTP SDK and standalone model plugins.
use super::*;
#[path = "http_options.rs"]
mod options;

/// Generates a TypeScript SDK with one selected transport and typed options.
pub struct Sdk {
    http_input: poolster_core::engine::HttpInput,
    meta: Meta,
    options: sdk::SdkConfig,
    client_style: Option<SdkClientStyle>,
}
pub fn sdk() -> Sdk {
    Sdk {
        http_input: Default::default(),
        meta: Meta::new(),
        options: sdk::SdkConfig::new("__package"),
        client_style: None,
    }
}

impl Plugin<TypeScript> for Sdk {
    fn supports_native_input(&self) -> bool {
        self.http_input.is_explicit()
    }
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
    fn requires(&self) -> Vec<poolster_core::engine::Requirement> {
        self.http_input.requirements()
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
        let selected = self.http_input.resolve(cx)?;
        let input_api = &selected.api;
        let mut options = self.options.clone();
        options.client_name = options
            .client_name
            .or_else(|| cx.common.client_name.clone());
        options.client_style = self
            .client_style
            .or(cx.common.client_style)
            .unwrap_or(options.client_style);
        options.package_name = cx.settings.package_name.clone();
        let tree = sdk::generate_sdk(input_api, &options, selected.security_schemes.as_ref())?;
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
        let prepared_api = symbols::prepare(input_api);
        let mut schemas = std::collections::BTreeMap::new();
        let mut operation_modules = std::collections::BTreeMap::new();
        let mut functions = std::collections::BTreeMap::new();
        let schema_files = input_api
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
        let operation_types = input_api
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
        let operation_files = input_api
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
            .insert(".poolster/client".into(), options.transport);
        cx.publish(composition::Transport {
            module: ".poolster/client".into(),
            lossless_json: true,
        })?;
        cx.publish(composition::Operations { functions })?;
        cx.files.emit(GeneratedFile::new(
            "STYLE_GUIDE.md",
            style_guide(input_api, &options),
        )?)
    }
}

/// A standalone model generator which publishes real schema symbols.
pub struct Types {
    http_input: poolster_core::engine::HttpInput,
    meta: Meta,
    output: String,
    layout: Option<SourceLayout>,
}
pub fn types() -> Types {
    Types {
        http_input: Default::default(),
        meta: Meta::new(),
        output: "models".into(),
        layout: None,
    }
}

impl Plugin<TypeScript> for Types {
    fn supports_native_input(&self) -> bool {
        self.http_input.is_explicit()
    }
    fn kind(&self) -> &'static str {
        "typescript-types"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<TsTypes>()]
    }
    fn requires(&self) -> Vec<poolster_core::engine::Requirement> {
        self.http_input.requirements()
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
        let selected = self.http_input.resolve(cx)?;
        let input_api = &selected.api;
        let mut schemas = std::collections::BTreeMap::new();
        let prepared = crate::auxiliary_layout::prepare(input_api);
        for (schema, original) in prepared.schemas.iter().zip(&input_api.schemas) {
            let symbol = cx.workspace.declare(
                &self.output,
                &render::type_identifier(&schema.name),
                self.kind(),
            )?;
            if schemas.insert(original.name.clone(), symbol).is_some() {
                anyhow::bail!("duplicate schema name {}", schema.name);
            }
        }
        let config = render::ArtifactOptions {
            output_dir: Some(".".into()),
            package_name: cx.settings.package_name.clone(),
            ..Default::default()
        };
        let config = render::ArtifactOptions {
            layout: self.layout.clone().or_else(|| cx.common.layout.clone()),
            ..config
        };
        for file in render::TypeScriptModels.generate(input_api, &config)? {
            let path = file.path.to_string_lossy();
            let target = if path == "models.ts" {
                format!("{}.ts", self.output)
            } else {
                path.replacen("models_chunks", &format!("{}_chunks", self.output), 1)
            };
            let contents = file.contents.replace(
                "./models_chunks/",
                &format!(
                    "./{}_chunks/",
                    self.output.rsplit('/').next().unwrap_or(&self.output)
                ),
            );
            cx.files.emit(GeneratedFile::new(target, contents)?)?;
        }
        for mut file in render::TypeScriptPackage.generate(input_api, &config)? {
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

fn style_guide(api: &poolster_core::Api, options: &sdk::SdkConfig) -> String {
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
