use super::*;

impl ModelRenderer {
    pub(crate) fn generate(
        &self,
        api: &Api,
        config: &ModelRenderOptions,
    ) -> Result<Vec<GeneratedFile>> {
        let type_names = api
            .schemas
            .iter()
            .map(|schema| (schema.name.clone(), model_type_name(schema, &config.model)))
            .collect::<BTreeMap<_, _>>();
        let mut prepared = api.clone();
        crate::json::visit_api(&mut prepared, &mut |value| {
            if let Some(name) = value
                .kind
                .reference_name()
                .and_then(|name| type_names.get(name))
            {
                value
                    .extensions
                    .insert("x-poolster-type-name".into(), Value::String(name.clone()));
            }
        });
        let api = &prepared;
        let output_dir = config.output_dir.trim_matches('/');
        let schema_output_dir = config
            .schema_output_dir
            .as_deref()
            .unwrap_or(output_dir)
            .trim_matches('/');
        let operation_output_dir = config
            .operation_output_dir
            .as_deref()
            .unwrap_or(output_dir)
            .trim_matches('/');
        let group_by_tag = config.group_by_tag;
        let options = &config.model;
        let notice = generated_notice(api);
        let schemas = api.schemas.iter().map(|schema| {
            let file = format!("{}.ts", schema_file_identifier(&schema.name));
            let path = if schema_output_dir.is_empty() {
                file
            } else {
                format!("{schema_output_dir}/{file}")
            };
            let mut references = BTreeSet::new();
            collect_references(&schema.value, &mut references);
            references.remove(&schema.name);
            let notice =
                reference_notice(&notice, &path, schema_output_dir, &references, &type_names)?;
            GeneratedFile::new(
                path,
                render_schema(&schema.name, &schema.value, options, &notice),
            )
        });
        let operations = api.operations.iter().map(|operation| {
            let file = format!("{}.ts", operation_model_file_identifier(&operation.id));
            let group = group_by_tag.then(|| operation_tag_directory(operation));
            let directory = match group {
                Some(group) if operation_output_dir.is_empty() => group,
                Some(group) => format!("{operation_output_dir}/{group}"),
                None => operation_output_dir.to_owned(),
            };
            let path = if directory.is_empty() {
                file
            } else {
                format!("{directory}/{file}")
            };
            let mut references = BTreeSet::new();
            for schema in operation
                .parameters
                .iter()
                .filter_map(|parameter| parameter.schema.as_ref())
                .chain(
                    operation
                        .request_body
                        .iter()
                        .flat_map(|body| &body.media_types)
                        .filter_map(|media| media.schema.as_ref()),
                )
                .chain(
                    operation
                        .responses
                        .iter()
                        .flat_map(|response| &response.media_types)
                        .filter_map(|media| media.schema.as_ref()),
                )
            {
                collect_references(schema, &mut references);
            }
            // Operation-local Body/Response aliases may shadow component imports.
            let bare = render_operation(operation, options, "");
            let locals: BTreeSet<String> = bare
                .lines()
                .filter_map(|line| {
                    line.strip_prefix("export type ")
                        .or_else(|| line.strip_prefix("export const "))
                        .and_then(|rest| rest.split_whitespace().next())
                        .map(str::to_owned)
                })
                .collect();
            let mut reserved = locals.clone();
            reserved.extend(
                references
                    .iter()
                    .filter_map(|name| type_names.get(name))
                    .cloned(),
            );
            let mut aliases = BTreeMap::new();
            for reference in &references {
                let name = type_names
                    .get(reference)
                    .cloned()
                    .unwrap_or_else(|| type_identifier(reference));
                if locals.contains(&name) {
                    let base = format!("{name}Model");
                    let mut alias = base.clone();
                    let mut suffix = 2;
                    while !reserved.insert(alias.clone()) {
                        alias = format!("{base}{suffix}");
                        suffix += 1;
                    }
                    aliases.insert(reference.clone(), alias);
                }
            }
            let notice =
                reference_notice(&notice, &path, schema_output_dir, &references, &type_names)?;
            let mut aliased_notice = notice;
            for (reference, alias) in &aliases {
                let name = type_names
                    .get(reference)
                    .cloned()
                    .unwrap_or_else(|| type_identifier(reference));
                aliased_notice = aliased_notice.replace(
                    &format!("import type {{ {name} }}"),
                    &format!("import type {{ {name} as {alias} }}"),
                );
            }
            let mut local_api = Api {
                operations: vec![operation.clone()],
                ..Default::default()
            };
            crate::json::visit_api(&mut local_api, &mut |value| {
                if let Some(alias) = value
                    .kind
                    .reference_name()
                    .and_then(|name| aliases.get(name))
                {
                    value
                        .extensions
                        .insert("x-poolster-type-name".into(), Value::String(alias.clone()));
                }
            });
            GeneratedFile::new(
                path,
                render_operation(&local_api.operations[0], options, &aliased_notice),
            )
        });

        schemas.chain(operations).collect()
    }
}

pub(crate) fn collect_references(value: &SchemaValue, references: &mut BTreeSet<String>) {
    match &value.kind {
        SchemaKind::Reference { .. } => {
            if let Some(name) = value.kind.reference_name() {
                references.insert(name.to_owned());
            }
        }
        SchemaKind::Array { items } => collect_references(items, references),
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            for field in fields {
                collect_references(&field.value, references);
            }
            if let AdditionalProperties::Schema { value } = additional_properties {
                collect_references(value, references);
            }
        }
        SchemaKind::OneOf { variants }
        | SchemaKind::AnyOf { variants }
        | SchemaKind::AllOf { variants } => {
            for variant in variants {
                collect_references(variant, references);
            }
        }
        _ => {}
    }
}

pub(crate) fn reference_notice(
    notice: &str,
    path: &str,
    directory: &str,
    references: &BTreeSet<String>,
    type_names: &BTreeMap<String, String>,
) -> Result<String> {
    let mut output = notice.to_owned();
    for reference in references {
        let symbol = crate::Symbol {
            module: std::path::Path::new(directory).join(schema_file_identifier(reference)),
            name: type_names
                .get(reference)
                .cloned()
                .unwrap_or_else(|| type_identifier(reference)),
        };
        writeln!(
            output,
            "import type {{ {} }} from '{}'",
            symbol.name,
            symbol.import_from(path)?
        )?;
    }
    if !references.is_empty() {
        output.push('\n');
    }
    Ok(output)
}

pub(crate) fn operation_tag_directory(operation: &Operation) -> String {
    operation
        .annotations
        .get("tags")
        .and_then(Value::as_array)
        .and_then(|tags| tags.first())
        .and_then(Value::as_str)
        .map(lower_camel_identifier)
        .filter(|tag| !tag.is_empty())
        .unwrap_or_else(|| "default".into())
}
