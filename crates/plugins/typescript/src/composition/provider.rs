use super::*;

impl Plugin<TypeScript> for Provider {
    fn supports_native_input(&self) -> bool {
        self.http_input.is_explicit()
    }
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
        let mut requirements = match self.part {
            Part::Operations => vec![
                Requirement::on(self.models),
                Requirement::on(self.transport),
            ],
            Part::Client => vec![
                Requirement::on(self.operations),
                Requirement::on(self.transport),
            ],
            _ => vec![],
        };
        requirements.extend(self.http_input.requirements());
        requirements
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
        let selected = self.http_input.resolve(cx)?;
        let input_api = &selected.api;
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
        let prepared_api = crate::symbols::prepare(input_api);
        let mut tree = poolster_core::GeneratedTree::default();
        match self.part {
            Part::Models => {
                for file in crate::models::ModelRenderer.generate(
                    &prepared_api,
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
                    sdk::poolster_package(
                        &prepared_api,
                        sdk::SdkTransport::Fetch,
                        cx.settings.package_name.as_deref(),
                    )?,
                )?)?;
                tree.insert(GeneratedFile::new("__package/tsconfig.json", r#"{"compilerOptions":{"declaration":true,"module":"ESNext","moduleResolution":"Bundler","outDir":"dist","strict":true,"skipLibCheck":true,"target":"ES2022"},"include":["**/*.ts"]}"#)?)?;
            }
            Part::Transport => {
                tree.insert(GeneratedFile::new(
                    "__package/.poolster/client.ts",
                    sdk::poolster_runtime(config.transport, selected.security_schemes.as_ref()),
                )?)?;
            }
            Part::Operations => {
                for file in crate::clients::generate_operations(
                    &prepared_api,
                    &crate::clients::ClientRenderOptions {
                        model_options: Some(config.model_options.clone()),
                        output_dir: "__package/clients".into(),
                        throw_on_error: config.throw_on_error,
                        group_by_tag: false,
                        group_default_directory: false,
                        type_import_prefix: Some("../models".into()),
                        runtime_import_prefix: Some("..".into()),
                        runtime_dir: ".poolster".into(),
                    },
                    selected.security_schemes.as_ref(),
                )? {
                    tree.insert(file)?;
                }
            }
            Part::Client => {
                let name = config
                    .client_name
                    .clone()
                    .unwrap_or_else(|| sdk::sdk_client_name(&input_api.name));
                for file in sdk::poolster_sdk_client(
                    &prepared_api,
                    &name,
                    false,
                    config.client_style,
                    "__package",
                    ".poolster",
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
                Part::Transport => path == Path::new(".poolster/client.ts"),
                Part::Operations => {
                    path.starts_with("clients")
                        && (path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .is_some_and(|s| s.starts_with("_poolster_json_refs"))
                            || prepared_api.operations.iter().any(|o| {
                                path.file_stem().is_some_and(|s| {
                                    s == crate::clients::operation_file_identifier(&o.id).as_str()
                                })
                            }))
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
                let original = module_import(Path::new(".poolster/client"), path)?;
                contents = contents.replace(&format!("'{original}'"), &format!("'{import}'"));
            }
            if matches!(self.part, Part::Operations)
                && path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.starts_with("_poolster_json_refs"))
            {
                cx.files.emit(GeneratedFile::new(target, contents)?)?;
                continue;
            }
            if let Part::Operations = self.part {
                let operation = input_api
                    .operations
                    .iter()
                    .zip(&prepared_api.operations)
                    .find(|(_, o)| {
                        path.file_stem().is_some_and(|s| {
                            s == crate::clients::operation_file_identifier(&o.id).as_str()
                        })
                    })
                    .map(|(original, _)| original)
                    .context("operation module has no contract operation")?;
                let models = cx.inputs.get::<Models>()?;
                let model = models
                    .operation_modules
                    .get(&operation.id)
                    .context("model provider omitted operation")?;
                let rendered = &prepared_api.operations[input_api
                    .operations
                    .iter()
                    .position(|o| o.id == operation.id)
                    .unwrap()];
                let old = format!(
                    "'../models/{}'",
                    crate::models::operation_model_file_identifier(&rendered.id)
                );
                contents = contents.replace(&old, &format!("'{}'", module_import(model, &target)?));
                functions.insert(
                    operation.id.clone(),
                    cx.workspace.declare(
                        target.with_extension(""),
                        &sdk::lower_camel_identifier(&rendered.id),
                        self.kind(),
                    )?,
                );
            }
            if let Part::Client = self.part {
                let operations = cx.inputs.get::<Operations>()?;
                for (operation, rendered) in
                    input_api.operations.iter().zip(&prepared_api.operations)
                {
                    let symbol = operations
                        .functions
                        .get(&operation.id)
                        .context("operation provider omitted operation")?;
                    let old = format!(
                        "'{}'",
                        module_import(
                            Path::new(&format!(
                                "clients/{}",
                                crate::clients::operation_file_identifier(&rendered.id)
                            )),
                            path
                        )?
                    );
                    contents =
                        contents.replace(&old, &format!("'{}'", symbol.import_from(&target)?));
                    let original = sdk::lower_camel_identifier(&rendered.id);
                    if symbol.name != original {
                        contents = contents.replace(
                            &format!("import {{ {original} }}"),
                            &format!("import {{ {} as {original} }}", symbol.name),
                        );
                    }
                }
            }
            if let Part::Models = self.part {
                if let Some((schema, rendered)) = input_api
                    .schemas
                    .iter()
                    .zip(&prepared_api.schemas)
                    .find(|(_, schema)| {
                        target.file_stem().is_some_and(|stem| {
                            stem == crate::models::schema_file_identifier(&schema.name).as_str()
                        })
                    })
                {
                    let name = crate::models::model_type_name(rendered, &config.model_options);
                    schema_symbols.insert(
                        schema.name.clone(),
                        cx.workspace
                            .declare(target.with_extension(""), &name, self.kind())?,
                    );
                }
                if let Some((operation, _)) = input_api
                    .operations
                    .iter()
                    .zip(&prepared_api.operations)
                    .find(|(_, operation)| {
                        target.file_stem().is_some_and(|stem| {
                            stem == crate::models::operation_model_file_identifier(&operation.id)
                                .as_str()
                        })
                    })
                {
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
                cx.workspace
                    .native_transports
                    .insert(output.clone(), config.transport);
                cx.publish(Transport {
                    module: output,
                    lossless_json: true,
                })
            }
            Part::Operations => cx.publish(Operations { functions }),
            Part::Client => {
                let name = config
                    .client_name
                    .unwrap_or_else(|| sdk::sdk_client_name(&input_api.name));
                let symbol = cx.workspace.declare(output, &name, self.kind())?;
                cx.publish(Client { symbol })
            }
        }
    }
}
