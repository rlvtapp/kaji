use super::*;

pub(crate) const BARREL_EXPORTS_PER_FILE: usize = 100;

/// Builds a shallow, stable public export topology. A large API no longer
/// writes every symbol into `index.ts`: consumers retain normal root imports
/// while TypeScript only has to parse small barrel modules at each level.
pub(crate) fn poolster_barrels(
    tree: &GeneratedTree,
    root: &str,
    api: &Api,
    group_by_tag: bool,
    client_name: Option<&str>,
    export_runtime_types: bool,
) -> Result<Vec<(String, String)>> {
    let type_export = if export_runtime_types {
        "export * from"
    } else {
        "export type * from"
    };
    let mut files = Vec::new();
    let mut output = String::new();
    output.push_str("export * from './custom'\n");
    if let Some(client_name) = client_name {
        output.push_str(&format!("export {{ {client_name} }} from './client'\n"));
    }
    output.push_str("export { createClient } from './.poolster/client'\nexport type { ClientConfig, ClientInstance, RequestOptions, ClientMiddleware, MiddlewareNext, MiddlewareResponse } from './.poolster/client'\n");
    output.push_str("export * from './models'\nexport * from './clients'\n");
    files.push(("index.ts".into(), output));

    let schema_symbols: std::collections::BTreeSet<String> = api
        .schemas
        .iter()
        .filter_map(|schema| {
            tree.get(format!(
                "{root}/models/{}.ts",
                crate::models::schema_file_identifier(&schema.name)
            ))
        })
        .flat_map(exported_symbols)
        .collect();
    let mut reserved_symbols: std::collections::BTreeSet<String> = tree
        .iter()
        .flat_map(|(_, source)| exported_symbols(source))
        .collect();
    let schema_paths = api
        .schemas
        .iter()
        .map(|schema| format!("./{}", crate::models::schema_file_identifier(&schema.name)))
        .collect::<Vec<_>>();
    let schema_chunks = render_barrel_chunks(
        &mut files,
        "models",
        "schemas",
        &schema_paths,
        type_export,
        None,
    )?;

    let mut operation_groups = BTreeMap::<String, Vec<&Operation>>::new();
    for operation in &api.operations {
        let group = if group_by_tag {
            operation_tag_directory(operation)
        } else {
            String::new()
        };
        operation_groups.entry(group).or_default().push(operation);
    }

    let mut model_index = String::new();
    for chunk in &schema_chunks {
        let _ = std::fmt::Write::write_fmt(
            &mut model_index,
            format_args!("export * from './{chunk}'\n"),
        );
    }
    let mut client_index = String::new();
    for (group, operations) in &operation_groups {
        let (model_dir, client_dir) = if group.is_empty() {
            ("models".to_owned(), "clients".to_owned())
        } else {
            (format!("models/{group}"), format!("clients/{group}"))
        };
        let model_exports = operations
            .iter()
            .map(|operation| format!("./{}", operation_model_file_identifier(&operation.id)))
            .collect::<Vec<_>>();
        let mut model_chunks = Vec::new();
        for (index, chunk) in model_exports.chunks(BARREL_EXPORTS_PER_FILE).enumerate() {
            let name = format!("operation_types_{:04}", index + 1);
            let mut contents = String::new();
            for path in chunk {
                let source = tree
                    .get(format!(
                        "{root}/{model_dir}/{}.ts",
                        path.trim_start_matches("./")
                    ))
                    .expect("operation model emitted");
                let symbols = exported_symbols(source);
                if symbols.iter().any(|symbol| schema_symbols.contains(symbol)) {
                    let exports = symbols
                        .into_iter()
                        .map(|symbol| {
                            if !schema_symbols.contains(&symbol) {
                                return symbol;
                            }
                            let base = format!("{symbol}Operation");
                            let mut alias = base.clone();
                            let mut suffix = 2;
                            while !reserved_symbols.insert(alias.clone()) {
                                alias = format!("{base}{suffix}");
                                suffix += 1;
                            }
                            format!("{symbol} as {alias}")
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    let export = if export_runtime_types {
                        "export"
                    } else {
                        "export type"
                    };
                    contents.push_str(&format!("{export} {{ {exports} }} from '{path}'\n"));
                } else {
                    contents.push_str(&format!("{type_export} '{path}'\n"));
                }
            }
            files.push((format!("{model_dir}/{name}.ts"), contents));
            model_chunks.push(name);
        }
        let client_exports = operations
            .iter()
            .map(|operation| {
                (
                    format!("./{}", operation_file_identifier(&operation.id)),
                    lower_camel_identifier(&operation.id),
                )
            })
            .collect::<Vec<_>>();
        let client_chunks = render_client_barrel_chunks(&mut files, &client_dir, &client_exports)?;

        let mut model_group_index = String::new();
        for chunk in model_chunks {
            let _ = std::fmt::Write::write_fmt(
                &mut model_group_index,
                format_args!("export * from './{chunk}'\n"),
            );
        }
        if group.is_empty() {
            model_index.push_str(&model_group_index);
        } else {
            files.push((format!("{model_dir}/index.ts"), model_group_index));
            let _ = std::fmt::Write::write_fmt(
                &mut model_index,
                format_args!("export * from './{group}'\n"),
            );
        }

        let mut client_group_index = String::new();
        for chunk in client_chunks {
            let _ = std::fmt::Write::write_fmt(
                &mut client_group_index,
                format_args!("export * from './{chunk}'\n"),
            );
        }
        if group.is_empty() {
            client_index.push_str(&client_group_index);
        } else {
            files.push((format!("{client_dir}/index.ts"), client_group_index));
            let _ = std::fmt::Write::write_fmt(
                &mut client_index,
                format_args!("export * from './{group}'\n"),
            );
        }
    }
    files.push(("models/index.ts".into(), model_index));
    files.push(("clients/index.ts".into(), client_index));
    Ok(files)
}

pub(crate) fn exported_symbols(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            line.strip_prefix("export type ")
                .or_else(|| line.strip_prefix("export const "))
                .or_else(|| line.strip_prefix("export enum "))
                .and_then(|rest| rest.split_whitespace().next())
                .map(str::to_owned)
        })
        .collect()
}

pub(crate) fn render_barrel_chunks(
    files: &mut Vec<(String, String)>,
    directory: &str,
    prefix: &str,
    exports: &[String],
    export_kind: &str,
    _reserved: Option<()>,
) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for (index, chunk) in exports.chunks(BARREL_EXPORTS_PER_FILE).enumerate() {
        let name = format!("{prefix}_{:04}", index + 1);
        let contents = chunk
            .iter()
            .map(|path| format!("{export_kind} '{path}'\n"))
            .collect();
        files.push((format!("{directory}/{name}.ts"), contents));
        names.push(name);
    }
    Ok(names)
}

pub(crate) fn render_client_barrel_chunks(
    files: &mut Vec<(String, String)>,
    directory: &str,
    exports: &[(String, String)],
) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for (index, chunk) in exports.chunks(BARREL_EXPORTS_PER_FILE).enumerate() {
        let name = format!("operations_{:04}", index + 1);
        let contents = chunk
            .iter()
            .map(|(path, function)| format!("export {{ {function} }} from '{path}'\n"))
            .collect();
        files.push((format!("{directory}/{name}.ts"), contents));
        names.push(name);
    }
    Ok(names)
}
