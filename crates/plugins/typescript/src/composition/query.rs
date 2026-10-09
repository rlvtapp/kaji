//! Native framework consumers with stable provider-bound imports.
mod options;
use super::*;

pub enum QueryFramework {
    React,
    Vue,
    Swr,
}
#[derive(Clone, Copy, Debug)]
pub enum QueryKind {
    Query,
    Mutation,
}

pub struct Query {
    http_input: poolster_core::engine::HttpInput,
    meta: Meta,
    framework: QueryFramework,
    provider: Option<Handle<Operations>>,
    output: String,
    operations_per_file: Option<usize>,
    layout: Option<poolster_core::SourceLayout>,
    include: Option<std::collections::BTreeSet<String>>,
    kinds: BTreeMap<String, QueryKind>,
    names: BTreeMap<String, String>,
}
pub fn react_query() -> Query {
    Query {
        http_input: Default::default(),
        meta: Meta::new(),
        framework: QueryFramework::React,
        provider: None,
        output: "react-query".into(),
        operations_per_file: Some(50),
        layout: None,
        include: None,
        kinds: BTreeMap::new(),
        names: BTreeMap::new(),
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

impl Plugin<TypeScript> for Query {
    fn supports_native_input(&self) -> bool {
        self.http_input.is_explicit()
    }
    fn kind(&self) -> &'static str {
        "typescript-query"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        let mut requirements = vec![Requirement::on(self.provider)];
        requirements.extend(self.http_input.requirements());
        requirements
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
        let selected = self.http_input.resolve(cx)?;
        let input_api = &selected.api;
        let config = render::ArtifactOptions {
            output_dir: Some(".".into()),
            group_by_tag: false,
            ..Default::default()
        };
        let (dependency, version) = match self.framework {
            QueryFramework::React => ("@tanstack/react-query", "^5.0.0"),
            QueryFramework::Vue => ("@tanstack/vue-query", "^5.0.0"),
            QueryFramework::Swr => ("swr", "^2.0.0"),
        };
        if let Some(count) = self.operations_per_file {
            anyhow::ensure!(count > 0, "max_operations_per_file must be positive");
        }
        let known = input_api
            .operations
            .iter()
            .map(|op| op.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        for id in self
            .include
            .iter()
            .flatten()
            .chain(self.kinds.keys())
            .chain(self.names.keys())
        {
            anyhow::ensure!(
                known.contains(id.as_str()),
                "query consumer refers to unknown operation {id:?}"
            );
        }
        let mut prepared = crate::symbols::prepare(input_api);
        for (original, native) in input_api.operations.iter().zip(&mut prepared.operations) {
            native.annotations.insert(
                "poolster.query.operation_id".into(),
                serde_json::json!(original.id),
            );
            if let Some(kind) = self.kinds.get(&original.id) {
                native.annotations.insert(
                    "poolster.query.kind".into(),
                    serde_json::json!(match kind {
                        QueryKind::Query => "query",
                        QueryKind::Mutation => "mutation",
                    }),
                );
            }
            if let Some(name) = self.names.get(&original.id) {
                anyhow::ensure!(
                    !name.trim().is_empty(),
                    "query operation name must not be empty"
                );
                native
                    .annotations
                    .insert("poolster.query.name".into(), serde_json::json!(name));
            }
        }
        let selected = prepared
            .operations
            .iter()
            .filter(|operation| {
                self.include.as_ref().is_none_or(|ids| {
                    ids.contains(
                        operation.annotations["poolster.query.operation_id"]
                            .as_str()
                            .unwrap(),
                    )
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut exported = std::collections::BTreeSet::new();
        for operation in &selected {
            let name = crate::query_helpers::name(operation);
            anyhow::ensure!(
                exported.insert(crate::symbols::identifier(name)),
                "query helper name collision for {name:?}"
            );
            poolster_core::pagination::normalize_pagination(&prepared, operation, None)?;
        }
        let count = self.operations_per_file.unwrap_or(usize::MAX);
        let framework = match self.framework {
            QueryFramework::React => "@tanstack/react-query",
            QueryFramework::Vue => "@tanstack/vue-query",
            QueryFramework::Swr => "swr",
        };
        let layout = self.layout.as_ref().or(cx.common.layout.as_ref());
        let groups = if let Some(layout) = layout {
            let resources = selected
                .iter()
                .map(sdk::operation_group)
                .collect::<Vec<_>>();
            let units = selected
                .iter()
                .zip(&resources)
                .map(
                    |(operation, resource)| poolster_core::source_layout::SourceUnit {
                        bytes: render::render_query_operation(
                            &prepared,
                            operation,
                            &config,
                            framework,
                            matches!(self.framework, QueryFramework::Swr),
                        )
                        .len(),
                        resource: Some(resource.as_str()),
                    },
                )
                .collect::<Vec<_>>();
            layout.groups(&units, 7500)?
        } else {
            (0..selected.len())
                .collect::<Vec<_>>()
                .chunks(count)
                .map(|indices| indices.to_vec())
                .collect()
        };
        let split = layout.is_some_and(|layout| {
            matches!(
                layout,
                poolster_core::SourceLayout::PerOperation
                    | poolster_core::SourceLayout::PerResource { .. }
            )
        }) || groups.len() > 1;
        let groups = if groups.is_empty() {
            vec![vec![]]
        } else {
            groups
        };
        let operations = cx.inputs.get::<Operations>()?;
        let mut barrel = String::new();
        let shared_path =
            GeneratedFile::new(format!("{}_chunks/runtime.ts", self.output), "")?.path;
        if split {
            cx.files.emit(GeneratedFile::new(
                &shared_path,
                crate::query_helpers::shared_runtime(true),
            )?)?;
        }

        for (index, indices) in groups.into_iter().enumerate() {
            let chunk = indices
                .iter()
                .map(|index| selected[*index].clone())
                .collect::<Vec<_>>();
            let mut group = prepared.clone();
            group.operations = chunk.clone();
            let files = match self.framework {
                QueryFramework::React => render::TypeScriptReactQuery.generate(&group, &config)?,
                QueryFramework::Vue => render::TypeScriptVueQuery.generate(&group, &config)?,
                QueryFramework::Swr => render::TypeScriptSwr.generate(&group, &config)?,
            };
            let module = if split {
                match layout {
                    Some(poolster_core::SourceLayout::PerOperation) if !chunk.is_empty() => {
                        format!(
                            "{}_operations/{}",
                            self.output,
                            crate::clients::operation_file_identifier(&chunk[0].id)
                        )
                    }
                    Some(poolster_core::SourceLayout::PerResource { .. }) => {
                        format!("{}_resources/chunk_{index:04}", self.output)
                    }
                    _ => format!("{}_chunks/chunk_{index:04}", self.output),
                }
            } else {
                self.output.clone()
            };
            let target = GeneratedFile::new(format!("{module}.ts"), "")?.path;
            for file in files {
                let mut contents = file.contents;
                if split {
                    let start = contents
                        .find("// __poolster_shared_start")
                        .context("query runtime boundary missing")?;
                    let end = contents
                        .find("// __poolster_shared_end")
                        .context("query runtime boundary missing")?
                        + "// __poolster_shared_end\n".len();
                    let shared = Symbol {
                        module: shared_path.with_extension(""),
                        name: String::new(),
                    }
                    .import_from(&target)?;
                    let source = format!(
                        "import {{ __poolsterInputs, __poolsterQueryCall, __poolsterInitial, __poolsterNext, __poolsterPageOptions, __poolsterMaxPages, type PoolsterQueryScope, type PoolsterPageParam, type PoolsterPaginationOptions }} from {shared:?};\nexport type {{ PoolsterQueryScope, PoolsterPageParam, PoolsterPaginationOptions }} from {shared:?};\n"
                    );
                    contents.replace_range(start..end, &source);
                }

                for (operation, native) in input_api.operations.iter().zip(&prepared.operations) {
                    if !chunk.iter().any(|item| item.id == native.id) {
                        continue;
                    }
                    let symbol = operations
                        .functions
                        .get(&operation.id)
                        .context("query plugin requires operation symbol")?;
                    let function = sdk::lower_camel_identifier(&native.id);
                    let old = serde_json::to_string(&format!(
                        "{}/{function}",
                        config.clients_import.trim_end_matches('/')
                    ))?;
                    contents = contents
                        .replace(&old, &serde_json::to_string(&symbol.import_from(&target)?)?);
                    if symbol.name != function {
                        contents = contents.replace(
                            &format!("import {{ {function} }}"),
                            &format!("import {{ {} as {function} }}", symbol.name),
                        );
                    }
                }
                cx.files.emit(GeneratedFile::new(&target, contents)?)?;
            }
            if split {
                let specifier = Symbol {
                    module: PathBuf::from(&module),
                    name: String::new(),
                }
                .import_from(format!("{}.ts", self.output))?;
                let (mut values, types) = render::query_exports(
                    &group,
                    &config,
                    matches!(self.framework, QueryFramework::Swr),
                );
                if matches!(self.framework, QueryFramework::React) {
                    for operation in &group.operations {
                        if crate::query_helpers::is_read(operation) {
                            let name =
                                crate::symbols::identifier(crate::query_helpers::name(operation));
                            values.push(format!("use{name}Suspense"));
                            if poolster_core::poolster_extension(
                                &operation.annotations,
                                "pagination",
                            )
                            .is_some()
                                || operation.annotations.contains_key("x-speakeasy-pagination")
                            {
                                values.push(format!("use{name}SuspenseInfinite"));
                            }
                        }
                    }
                }
                if !values.is_empty() {
                    barrel.push_str(&format!(
                        "export {{ {} }} from {};\n",
                        values.join(", "),
                        serde_json::to_string(&specifier)?
                    ));
                }
                let mut types = types;
                if index == 0 {
                    types.extend([
                        "PoolsterQueryScope".into(),
                        "PoolsterPageParam".into(),
                        "PoolsterPaginationOptions".into(),
                    ]);
                }
                if !types.is_empty() {
                    barrel.push_str(&format!(
                        "export type {{ {} }} from {};\n",
                        types.join(", "),
                        serde_json::to_string(&specifier)?
                    ));
                }
            }
        }
        if split {
            emit_barrel(cx, &self.output, &barrel)?;
        }
        cx.workspace.peer_dependency(dependency, version)?;
        cx.workspace
            .export_namespace(&self.output, &sdk::lower_camel_identifier(&self.output))
    }
}

/// Bound export registries as well as operation bodies, retaining one public module.
fn emit_barrel(cx: &mut PluginContext<'_, TypeScript>, output: &str, source: &str) -> Result<()> {
    let declarations = source.lines().collect::<Vec<_>>();
    let units = declarations
        .iter()
        .map(|line| poolster_core::SourceUnit {
            bytes: line.len() + 1,
            resource: None,
        })
        .collect::<Vec<_>>();
    let groups = poolster_core::SourceLayout::Chunked {
        max_file_bytes: 128 * 1024,
        max_declarations: Some(100),
    }
    .groups(&units, 0)?;
    if groups.len() <= 1 {
        return cx
            .files
            .emit(GeneratedFile::new(format!("{output}.ts"), source)?);
    }
    let target = PathBuf::from(format!("{output}.ts"));
    let parent = target.parent().unwrap_or(Path::new("."));
    let mut entry = String::new();
    for (index, group) in groups.iter().enumerate() {
        let path = PathBuf::from(format!("{output}_chunks/index_{index:04}.ts"));
        let mut contents = String::new();
        for item in group {
            let (declaration, specifier) = declarations[*item]
                .split_once(" from ")
                .context("invalid generated query export")?;
            let specifier: String = serde_json::from_str(specifier.trim_end_matches(';'))?;
            let module = parent.join(specifier).with_extension("");
            let resolved = Symbol {
                module,
                name: String::new(),
            }
            .import_from(&path)?;
            contents.push_str(&format!(
                "{declaration} from {};\n",
                serde_json::to_string(&resolved)?
            ));
        }
        cx.files.emit(GeneratedFile::new(&path, contents)?)?;
        let specifier = Symbol {
            module: path.with_extension(""),
            name: String::new(),
        }
        .import_from(&target)?;
        entry.push_str(&format!(
            "export * from {};\n",
            serde_json::to_string(&specifier)?
        ));
    }
    cx.files.emit(GeneratedFile::new(target, entry)?)
}
