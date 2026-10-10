use super::*;

impl Plugin<TypeScript> for Auxiliary {
    fn supports_native_input(&self) -> bool {
        self.graphql || self.http_input.is_explicit()
    }
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
        if self.graphql {
            return vec![Requirement::on(self.graphql_client)];
        }
        let mut requirements = {
            let mut requirements = vec![Requirement::on(self.models)];
            if matches!(self.kind, AuxiliaryKind::Msw | AuxiliaryKind::Cypress) {
                requirements.push(Requirement::on(self.operations));
            }
            requirements
        };
        requirements.extend(self.http_input.requirements());
        requirements
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
        if self.graphql {
            anyhow::ensure!(
                !self.http_input.is_explicit(),
                "GraphQL helpers cannot select HTTP input contracts"
            );
            anyhow::ensure!(
                self.layout.is_none(),
                "GraphQL helper custom source layout is unsupported"
            );
            let client = cx.inputs.get::<crate::GraphqlClient>()?.clone();
            let kind = match self.kind {
                AuxiliaryKind::Zod => crate::graphql::helpers::Kind::Zod,
                AuxiliaryKind::Faker => crate::graphql::helpers::Kind::Faker,
                AuxiliaryKind::Msw => crate::graphql::helpers::Kind::Msw,
                AuxiliaryKind::Cypress => crate::graphql::helpers::Kind::Cypress,
            };
            return crate::graphql::helpers::generate(
                cx,
                &client,
                kind,
                &self.output,
                &self.fixture_options,
                &self.cypress_options,
                self.max_file_bytes,
            );
        }
        let selected = self.http_input.resolve(cx)?;
        let input_api = &selected.api;
        let models = cx.inputs.get::<Models>()?;
        let target = GeneratedFile::new(format!("{}.ts", self.output), "")?.path;
        let config = render::ArtifactOptions {
            output_dir: Some(".".into()),
            max_file_bytes: self.max_file_bytes,
            layout: self.layout.clone().or_else(|| cx.common.layout.clone()),
            fixture_options: self.fixture_options.clone(),
            cypress_options: self.cypress_options.clone(),
            ..Default::default()
        };
        let mut api = crate::symbols::prepare(input_api);
        for (prepared, original) in api.schemas.iter_mut().zip(&input_api.schemas) {
            prepared.value.extensions.insert(
                "poolster.aux.schema_name".into(),
                original.name.clone().into(),
            );
        }
        for (prepared, original) in api.operations.iter_mut().zip(&input_api.operations) {
            prepared.annotations.insert(
                "poolster.aux.operation_id".into(),
                original.id.clone().into(),
            );
        }
        let (files, dependency, version) = match self.kind {
            AuxiliaryKind::Zod => (
                render::TypeScriptZod.generate_with_models(&api, &config, &models.options)?,
                "zod",
                "^4.0.0",
            ),
            AuxiliaryKind::Faker => (
                render::TypeScriptFaker.generate_with_models(&api, &config, &models.options)?,
                "@faker-js/faker",
                "^9.0.0",
            ),
            AuxiliaryKind::Msw => (
                render::TypeScriptMsw.generate(&api, &config)?,
                "msw",
                "^2.0.0",
            ),
            AuxiliaryKind::Cypress => (
                render::TypeScriptCypress.generate(&api, &config)?,
                "cypress",
                "^15.0.0",
            ),
        };
        let kind = match self.kind {
            AuxiliaryKind::Zod => "zod",
            AuxiliaryKind::Faker => "faker",
            AuxiliaryKind::Msw => "msw",
            AuxiliaryKind::Cypress => "cypress",
        };
        let paths = files
            .iter()
            .map(|file| {
                let path = file.path.to_string_lossy();
                let relocated = if path.contains(&format!("{kind}_chunks/")) {
                    PathBuf::from(path.replace(
                        &format!("{kind}_chunks/"),
                        &format!("{}_chunks/", self.output),
                    ))
                } else {
                    target.clone()
                };
                (file.path.clone(), relocated)
            })
            .collect::<BTreeMap<_, _>>();
        for file in files {
            let target = &paths[&file.path];
            let mut source = file.contents;
            for (old, new) in &paths {
                let old_import = module_import(&old.with_extension(""), &file.path)?;
                let new_import = module_import(&new.with_extension(""), target)?;
                source = source
                    .replace(&format!("'{old_import}'"), &format!("'{new_import}'"))
                    .replace(
                        &serde_json::to_string(&old_import)?,
                        &serde_json::to_string(&new_import)?,
                    );
            }
            if matches!(self.kind, AuxiliaryKind::Faker | AuxiliaryKind::Zod) {
                let mut rewritten = String::new();
                for line in source.lines() {
                    if line.starts_with("import type {")
                        && (line.ends_with("from './models';")
                            || line.ends_with("from '../models';"))
                    {
                        let names = line.split('{').nth(1).unwrap().split('}').next().unwrap();
                        for entry in names.split(',').map(str::trim).filter(|x| !x.is_empty()) {
                            let (local, alias) = entry.split_once(" as ").unwrap_or((entry, entry));
                            let index = api
                                .schemas
                                .iter()
                                .position(|schema| render::type_identifier(&schema.name) == local)
                                .context("auxiliary model import has no provider schema")?;
                            let original = &input_api.schemas[index].name;
                            let symbol = models
                                .schemas
                                .get(original)
                                .context("model provider omitted schema")?;
                            rewritten.push_str(&format!(
                                "import type {{ {}{} }} from {};\n",
                                symbol.name,
                                if symbol.name == alias {
                                    String::new()
                                } else {
                                    format!(" as {alias}")
                                },
                                serde_json::to_string(&symbol.import_from(target)?)?
                            ));
                        }
                    } else {
                        rewritten.push_str(line);
                        rewritten.push('\n');
                    }
                }
                source = rewritten;
            }
            if matches!(self.kind, AuxiliaryKind::Msw | AuxiliaryKind::Cypress) {
                let operations = cx.inputs.get::<Operations>()?;
                for operation in &input_api.operations {
                    anyhow::ensure!(
                        operations.functions.contains_key(&operation.id),
                        "operation provider omitted {}",
                        operation.id
                    );
                }
            }
            cx.files.emit(GeneratedFile::new(target, source)?)?;
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
