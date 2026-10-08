//! TypeScript SDK layout, runtime, facade and typed renderer options.
use crate::clients::{ClientRenderOptions, generate_operations, operation_file_identifier};
use crate::models::{
    ModelOptions, ModelRenderOptions, ModelRenderer, operation_model_file_identifier,
};
use anyhow::{Result, bail};
use poolster_core::{
    Api, GeneratedFile, GeneratedTree, Operation, SdkClientStyle, SecuritySchemeCatalog,
};
use serde_json::Value;
use std::collections::BTreeMap;
/// A transport implementation selected within one language profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SdkTransport {
    Fetch,
    Axios,
}

/// Whether a TypeScript SDK exposes only generated exports or also an
/// instantiated product-client facade.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SdkSurface {
    /// Models and direct operation functions only.
    Raw,
    /// Models, direct operation functions, and a configured SDK client.
    #[default]
    Client,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SdkConfig {
    pub output_dir: String,
    pub package_name: Option<String>,
    pub client_name: Option<String>,
    pub client_style: SdkClientStyle,
    pub surface: SdkSurface,
    pub transport: SdkTransport,
    pub group_by_tag: bool,
    pub model_options: ModelOptions,
    pub throw_on_error: bool,
}

impl SdkConfig {
    pub(crate) fn new(output_dir: impl Into<String>) -> Self {
        Self {
            output_dir: output_dir.into(),
            package_name: None,
            client_name: None,
            client_style: SdkClientStyle::Namespaced,
            surface: SdkSurface::Client,
            transport: SdkTransport::Fetch,
            group_by_tag: true,
            model_options: ModelOptions::default(),
            throw_on_error: true,
        }
    }
}

pub(crate) fn generate_sdk(
    api: &Api,
    profile: &SdkConfig,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> Result<GeneratedTree> {
    if profile.output_dir.is_empty() {
        bail!("SDK output directory cannot be empty")
    }
    for operation in &api.operations {
        if poolster_core::poolster_extension(&operation.annotations, "pagination")
            .or_else(|| operation.annotations.get("x-speakeasy-pagination"))
            .and_then(|v| v.get("type"))
            .and_then(Value::as_str)
            == Some("page")
        {
            poolster_core::pagination::normalize_pagination(api, operation, None)?;
        }
    }
    generate_typescript_sdk(api, profile, security_schemes)
}

fn generate_typescript_sdk(
    api: &Api,
    profile: &SdkConfig,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> Result<GeneratedTree> {
    // Poolster's tag-directory layout otherwise puts every untagged operation in
    // `default`. Poolster's SDK surface uses the first meaningful path segment as
    // a stable resource namespace, matching the native targets.
    let mut sdk_api = crate::symbols::prepare(api);
    if profile.group_by_tag {
        for operation in &mut sdk_api.operations {
            if operation_tag_directory_if_present(operation).is_none() {
                let namespace = sdk_namespace(operation);
                operation
                    .annotations
                    .insert("tags".into(), serde_json::json!([namespace]));
            }
        }
    }
    let root = profile.output_dir.trim_matches('/');
    let models_dir = format!("{root}/models");
    let clients_dir = format!("{root}/clients");
    let type_options = ModelRenderOptions {
        output_dir: models_dir,
        schema_output_dir: None,
        operation_output_dir: None,
        group_by_tag: profile.group_by_tag,
        model: profile.model_options.clone(),
    };
    let client_options = ClientRenderOptions {
        model_options: Some(profile.model_options.clone()),
        output_dir: clients_dir,
        runtime_dir: ".poolster".into(),
        throw_on_error: profile.throw_on_error,
        group_by_tag: profile.group_by_tag,
        group_default_directory: profile.group_by_tag,
        type_import_prefix: Some(
            if profile.group_by_tag {
                "../../models"
            } else {
                "../models"
            }
            .into(),
        ),
        runtime_import_prefix: Some("..".into()),
    };
    let mut tree = GeneratedTree::default();
    for file in ModelRenderer.generate(&sdk_api, &type_options)? {
        tree.insert(file)?;
    }
    let client_files = generate_operations(&sdk_api, &client_options, security_schemes)?;
    for file in client_files {
        tree.insert(file)?;
    }
    tree.insert(GeneratedFile::new(
        format!("{root}/.poolster/client.ts"),
        poolster_runtime(profile.transport, security_schemes),
    )?)?;
    let client_name = (profile.surface == SdkSurface::Client).then(|| {
        profile
            .client_name
            .clone()
            .unwrap_or_else(|| sdk_client_name(&sdk_api.name))
    });
    if let Some(client_name) = &client_name {
        for file in poolster_sdk_client(
            &sdk_api,
            client_name,
            profile.group_by_tag,
            profile.client_style,
            root,
            ".poolster",
        )? {
            tree.insert(file)?;
        }
    }
    tree.insert_custom(GeneratedFile::new(
        format!("{root}/custom/index.ts"),
        "// This module is created once and never overwritten by Poolster.\n// Add stable helpers, exports, or product-specific wrappers here.\nexport {}\n",
    )?)?;
    for file in poolster_barrels(
        &tree,
        root,
        &sdk_api,
        profile.group_by_tag,
        client_name.as_deref(),
        !matches!(profile.model_options.enum_type, crate::EnumType::Literal),
    )? {
        tree.insert(GeneratedFile::new(format!("{root}/{}", file.0), file.1)?)?;
    }
    tree.insert(GeneratedFile::new(
        format!("{root}/package.json"),
        poolster_package(&sdk_api, profile.transport, profile.package_name.as_deref())?,
    )?)?;
    tree.insert(GeneratedFile::new(
        format!("{root}/README.md"),
        poolster_readme(&sdk_api, profile),
    )?)?;
    tree.insert(GeneratedFile::new(
        format!("{root}/tsconfig.json"),
        "{\n  \"compilerOptions\": { \"declaration\": true, \"module\": \"ESNext\", \"moduleResolution\": \"Bundler\", \"outDir\": \"dist\", \"strict\": true, \"skipLibCheck\": true, \"target\": \"ES2022\" },\n  \"include\": [\"**/*.ts\"]\n}\n",
    )?)?;
    Ok(tree)
}

const BARREL_EXPORTS_PER_FILE: usize = 100;

/// Builds a shallow, stable public export topology. A large API no longer
/// writes every symbol into `index.ts`: consumers retain normal root imports
/// while TypeScript only has to parse small barrel modules at each level.
fn poolster_barrels(
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

fn exported_symbols(source: &str) -> Vec<String> {
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

fn render_barrel_chunks(
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

fn render_client_barrel_chunks(
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

pub(crate) fn poolster_sdk_client(
    api: &Api,
    class_name: &str,
    group_by_tag: bool,
    style: SdkClientStyle,
    root: &str,
    runtime_dir: &str,
) -> Result<Vec<GeneratedFile>> {
    match style {
        SdkClientStyle::Flat => Ok(vec![GeneratedFile::new(
            format!("{root}/client.ts"),
            poolster_flat_sdk_client(api, class_name, group_by_tag, runtime_dir),
        )?]),
        SdkClientStyle::Namespaced => {
            poolster_namespaced_sdk_client(api, class_name, group_by_tag, root, runtime_dir)
        }
    }
}

pub(crate) fn poolster_flat_sdk_client(
    api: &Api,
    class_name: &str,
    group_by_tag: bool,
    runtime_dir: &str,
) -> String {
    let mut output = String::from(&format!(
        "import type {{ ClientConfig, ClientInstance }} from './{runtime_dir}/client'\nimport {{ createClient }} from './{runtime_dir}/client'\n"
    ));
    for operation in &api.operations {
        let function = lower_camel_identifier(&operation.id);
        let module = operation_file_identifier(&operation.id);
        let group = if group_by_tag {
            operation_tag_directory(operation)
        } else {
            String::new()
        };
        let path = if group.is_empty() {
            format!("./clients/{module}")
        } else {
            format!("./clients/{group}/{module}")
        };
        output.push_str(&format!("import {{ {function} }} from '{path}'\n"));
    }
    if has_pagination(api) {
        output.push_str(pagination_helpers());
    }
    output.push_str(&format!(
        "\nexport class {class_name} {{\n  /** Configured request transport for direct operations and generated framework hooks. */\n  readonly transport: ClientInstance\n"
    ));
    for operation in &api.operations {
        let function = lower_camel_identifier(&operation.id);
        output.push_str(&format!("  readonly {function}: typeof {function}\n"));
        if pagination(operation).is_some() {
            output.push_str(&format!(
                "  readonly {function}Pages: (options: Parameters<typeof {function}>[0]) => AsyncIterable<Awaited<ReturnType<typeof {function}>>>\n"
            ));
        }
    }
    output.push_str(
        "\n  constructor(config: ClientConfig = {}) {\n    this.transport = createClient(config)\n",
    );
    for operation in &api.operations {
        let function = lower_camel_identifier(&operation.id);
        output.push_str(&format!(
            "    this.{function} = ((options: Parameters<typeof {function}>[0]) => {function}({{ ...options, client: this.transport }})) as typeof {function}\n"
        ));
        if let Some(pagination) = pagination(operation) {
            output.push_str(&render_pagination_iterator(
                &function,
                &function,
                &pagination,
            ));
        }
    }
    output.push_str("  }\n}\n");
    output
}

fn poolster_namespaced_sdk_client(
    api: &Api,
    class_name: &str,
    group_by_tag: bool,
    root: &str,
    runtime_dir: &str,
) -> Result<Vec<GeneratedFile>> {
    let mut groups: BTreeMap<String, Vec<(&Operation, String)>> = BTreeMap::new();
    for operation in &api.operations {
        let namespace = sdk_namespace(operation);
        let method = sdk_method_name(operation, &namespace);
        groups
            .entry(namespace)
            .or_default()
            .push((operation, method));
    }

    let mut client_types = Vec::new();
    let mut files = Vec::new();
    for (namespace, operations) in &groups {
        let namespace_type = format!("{}Client", pascal_identifier(namespace));
        client_types.push((namespace.clone(), namespace_type.clone()));
        let resolved_methods = resolved_resource_methods(operations);
        let mut chunk_types = Vec::new();
        for (index, chunk) in resolved_methods.chunks(BARREL_EXPORTS_PER_FILE).enumerate() {
            let chunk_type = format!("{namespace_type}Operations{:04}", index + 1);
            let chunk_file = format!("operations_{:04}", index + 1);
            chunk_types.push((chunk_type.clone(), chunk_file.clone()));
            files.push(GeneratedFile::new(
                format!("{root}/resources/{namespace}/{chunk_file}.ts"),
                render_namespaced_resource_chunk(chunk, &chunk_type, group_by_tag, runtime_dir),
            )?);
        }
        let mut output = String::from(&format!(
            "import type {{ ClientInstance }} from '../{runtime_dir}/client'\n"
        ));
        for (chunk_type, chunk_file) in &chunk_types {
            output.push_str(&format!(
                "import {{ {chunk_type} }} from './{namespace}/{chunk_file}'\n"
            ));
        }
        output.push_str(&format!("\nexport interface {namespace_type} extends "));
        output.push_str(
            &chunk_types
                .iter()
                .map(|(chunk_type, _)| chunk_type.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );
        output.push_str(" {}\n\n");
        output.push_str(&format!("export class {namespace_type} {{\n"));
        output.push_str("  constructor(client: ClientInstance) {\n    Object.assign(this, ");
        output.push_str(
            &chunk_types
                .iter()
                .map(|(chunk_type, _)| format!("new {chunk_type}(client)"))
                .collect::<Vec<_>>()
                .join(", "),
        );
        output.push_str(")\n  }\n}\n");
        files.push(GeneratedFile::new(
            format!("{root}/resources/{namespace}.ts"),
            output,
        )?);
    }

    let mut output = String::from(&format!(
        "import type {{ ClientConfig, ClientInstance }} from './{runtime_dir}/client'\nimport {{ createClient }} from './{runtime_dir}/client'\n"
    ));
    for (namespace, namespace_type) in &client_types {
        output.push_str(&format!(
            "import {{ {namespace_type} }} from './resources/{namespace}'\n"
        ));
    }
    output.push_str(&format!(
        "\nexport class {class_name} {{\n  /** Configured request transport for direct operations and generated framework hooks. */\n  readonly transport: ClientInstance\n"
    ));
    for (namespace, namespace_type) in &client_types {
        output.push_str(&format!("  readonly {namespace}: {namespace_type}\n"));
    }
    output.push_str(
        "\n  constructor(config: ClientConfig = {}) {\n    this.transport = createClient(config)\n",
    );
    for (namespace, namespace_type) in &client_types {
        output.push_str(&format!(
            "    this.{namespace} = new {namespace_type}(this.transport)\n"
        ));
    }
    output.push_str("  }\n}\n");
    files.push(GeneratedFile::new(format!("{root}/client.ts"), output)?);
    Ok(files)
}

fn resolved_resource_methods<'a>(
    operations: &'a [(&'a Operation, String)],
) -> Vec<(&'a Operation, String)> {
    let mut names = std::collections::BTreeSet::new();
    operations
        .iter()
        .map(|(operation, proposed_name)| {
            let function = lower_camel_identifier(&operation.id);
            let name = if names.insert(proposed_name.clone()) {
                proposed_name.clone()
            } else {
                function
            };
            (*operation, name)
        })
        .collect()
}

fn render_namespaced_resource_chunk(
    operations: &[(&Operation, String)],
    class_name: &str,
    group_by_tag: bool,
    runtime_dir: &str,
) -> String {
    let mut output = String::from(&format!(
        "import type {{ ClientInstance }} from '../../{runtime_dir}/client'\n"
    ));
    for (operation, _) in operations {
        let function = lower_camel_identifier(&operation.id);
        let module = operation_file_identifier(&operation.id);
        let group = if group_by_tag {
            operation_tag_directory(operation)
        } else {
            String::new()
        };
        let path = if group.is_empty() {
            format!("../../clients/{module}")
        } else {
            format!("../../clients/{group}/{module}")
        };
        output.push_str(&format!("import {{ {function} }} from '{path}'\n"));
    }
    if operations
        .iter()
        .any(|(operation, _)| pagination(operation).is_some())
    {
        output.push_str(pagination_helpers());
    }
    output.push_str(&format!("\nexport class {class_name} {{\n"));
    for (operation, name) in operations {
        let function = lower_camel_identifier(&operation.id);
        output.push_str(&format!("  readonly {name}: typeof {function}\n"));
        if pagination(operation).is_some() {
            output.push_str(&format!(
                "  readonly {name}Pages: (options: Parameters<typeof {function}>[0]) => AsyncIterable<Awaited<ReturnType<typeof {function}>>>\n"
            ));
        }
    }
    output.push_str("\n  constructor(client: ClientInstance) {\n");
    for (operation, name) in operations {
        let function = lower_camel_identifier(&operation.id);
        output.push_str(&format!(
            "    this.{name} = ((options: Parameters<typeof {function}>[0]) => {function}({{ ...options, client }})) as typeof {function}\n"
        ));
        if let Some(pagination) = pagination(operation) {
            output.push_str(&render_pagination_iterator_with_client(
                name,
                &function,
                &pagination,
                "client",
            ));
        }
    }
    output.push_str("  }\n}\n");
    output
}

fn sdk_namespace(operation: &Operation) -> String {
    operation_tag_directory_if_present(operation).unwrap_or_else(|| {
        operation
            .path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .find(|segment| {
                !segment.starts_with('{')
                    && !segment.starts_with('v')
                    && *segment != "email"
                    && *segment != "api"
            })
            .map(lower_camel_identifier)
            .filter(|segment| !segment.is_empty())
            .unwrap_or_else(|| "api".into())
    })
}

fn sdk_method_name(operation: &Operation, namespace: &str) -> String {
    let function = lower_camel_identifier(&operation.id);
    let singular = namespace.strip_suffix('s').unwrap_or(namespace);
    let candidates = [pascal_identifier(namespace), pascal_identifier(singular)];
    for candidate in candidates {
        if let Some(method) = function.strip_suffix(&candidate) {
            if !method.is_empty() {
                return method.into();
            }
        }
    }
    let prefix = lower_camel_identifier(namespace);
    if let Some(method) = function.strip_prefix(&prefix) {
        if !method.is_empty() {
            return lower_camel_identifier(method);
        }
    }
    function
}

#[derive(Clone, Debug)]
struct CursorPagination {
    input_name: String,
    input_location: String,
    input_body_path: Option<String>,
    next_cursor_path: String,
}

#[derive(Clone, Debug)]
struct PaginationInput {
    name: String,
    location: String,
    /// An RFC 6901 JSON Pointer into a JSON request body. This is intentionally
    /// a separate, opt-in field: an input name is a wire name, not a safe way
    /// to infer where a nested value lives in a request model.
    body_path: Option<String>,
}

#[derive(Clone, Debug)]
struct OffsetPagination {
    step: OffsetStep,
    limit: Option<PaginationInput>,
    results_path: Option<String>,
    num_pages_path: Option<String>,
}

#[derive(Clone, Debug)]
struct UrlPagination {
    next_url_path: String,
}

#[derive(Clone, Debug)]
enum OffsetStep {
    Page(PaginationInput),
    Offset(PaginationInput),
}

#[derive(Clone, Debug)]
enum Pagination {
    Cursor(CursorPagination),
    OffsetLimit(OffsetPagination),
    Url(UrlPagination),
}

fn pagination(operation: &Operation) -> Option<Pagination> {
    let extension = poolster_core::poolster_extension(&operation.annotations, "pagination")
        .or_else(|| operation.annotations.get("x-speakeasy-pagination"))?
        .as_object()?;
    let outputs = extension.get("outputs")?.as_object()?;
    match extension.get("type").and_then(Value::as_str) {
        Some("cursor") => {
            let inputs = extension.get("inputs").and_then(Value::as_array)?;
            let input = pagination_input(operation, inputs, "cursor")?;
            Some(Pagination::Cursor(CursorPagination {
                input_name: input.name,
                input_location: input.location,
                input_body_path: input.body_path,
                next_cursor_path: outputs.get("nextCursor")?.as_str()?.to_owned(),
            }))
        }
        Some("offsetLimit" | "page") => {
            let inputs = extension.get("inputs").and_then(Value::as_array)?;
            let input = |kind| pagination_input(operation, inputs, kind);
            let page = input("page");
            let offset = input("offset");
            let step = match (page, offset) {
                (Some(page), _) => OffsetStep::Page(page),
                (None, Some(offset)) => OffsetStep::Offset(offset),
                (None, None) => return None,
            };
            let results_path = outputs
                .get("results")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let num_pages_path = outputs
                .get("numPages")
                .and_then(Value::as_str)
                .map(str::to_owned);
            match step {
                OffsetStep::Page(_) if results_path.is_none() && num_pages_path.is_none() => None,
                OffsetStep::Offset(_) if results_path.is_none() => None,
                _ => Some(Pagination::OffsetLimit(OffsetPagination {
                    step,
                    limit: input("limit"),
                    results_path,
                    num_pages_path,
                })),
            }
        }
        Some("url") => Some(Pagination::Url(UrlPagination {
            next_url_path: outputs.get("nextUrl")?.as_str()?.to_owned(),
        })),
        _ => None,
    }
}

fn pagination_input(
    operation: &Operation,
    inputs: &[Value],
    kind: &str,
) -> Option<PaginationInput> {
    let input = inputs
        .iter()
        .find(|input| input.get("type").and_then(Value::as_str) == Some(kind))?
        .as_object()?;
    let name = input.get("name")?.as_str()?.to_owned();
    let location = match input.get("in").and_then(Value::as_str) {
        Some("requestBody") => "body".into(),
        Some("parameters") | None => operation
            .parameters
            .iter()
            .find(|parameter| parameter.name == name)
            .map(|parameter| typescript_options_location(&parameter.location))
            .unwrap_or_else(|| "query".into()),
        Some(location) => typescript_options_location(location),
    };
    let body_path = if location == "body" {
        let path = input
            .get("bodyPath")
            .and_then(Value::as_str)
            .map(str::to_owned)
            // Preserve the original top-level request-body convention. Nested
            // body values must opt in with `bodyPath`; there is no schema-path
            // inference hidden behind this fallback.
            .unwrap_or_else(|| format!("/{}", json_pointer_escape(&name)));
        json_pointer_targets_name(&path, &name).then_some(path)
    } else {
        None
    };
    if location == "body" && (body_path.is_none() || !operation_has_json_request_body(operation)) {
        return None;
    }
    Some(PaginationInput {
        name,
        location,
        body_path,
    })
}

fn operation_has_json_request_body(operation: &Operation) -> bool {
    operation.request_body.as_ref().is_some_and(|body| {
        body.media_types.iter().any(|media| {
            media.content_type.eq_ignore_ascii_case("application/json")
                || media.content_type.to_ascii_lowercase().ends_with("+json")
        })
    })
}

fn json_pointer_escape(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

/// Confirms the explicit pointer is structurally valid and ends at the input's
/// declared wire name. This keeps an accidental `bodyPath` typo from updating
/// a different body field while still allowing optional intermediate objects.
fn json_pointer_targets_name(pointer: &str, name: &str) -> bool {
    let Some(last) = pointer
        .strip_prefix('/')
        .and_then(|path| path.rsplit('/').next())
    else {
        return false;
    };
    let unescaped = last.replace("~1", "/").replace("~0", "~");
    !pointer.contains("//") && unescaped == name
}

fn typescript_options_location(location: &str) -> String {
    match location {
        "header" => "headers".into(),
        "requestBody" => "body".into(),
        location => location.into(),
    }
}

fn has_pagination(api: &Api) -> bool {
    api.operations
        .iter()
        .any(|operation| pagination(operation).is_some())
}

fn render_pagination_iterator(
    public_name: &str,
    function: &str,
    pagination: &Pagination,
) -> String {
    render_pagination_iterator_with_client(public_name, function, pagination, "this.transport")
}

fn render_pagination_iterator_with_client(
    public_name: &str,
    function: &str,
    pagination: &Pagination,
    client_expression: &str,
) -> String {
    if let Pagination::Url(pagination) = pagination {
        return render_url_pagination_iterator(
            public_name,
            function,
            pagination,
            client_expression,
        );
    }
    let initial = if let Pagination::OffsetLimit(OffsetPagination {
        step: OffsetStep::Page(input),
        ..
    }) = pagination
    {
        let value = pagination_input_value(input);
        let update = pagination_input_update(input, "initialPage");
        let validate_limit = if let Pagination::OffsetLimit(OffsetPagination {
            limit: Some(limit),
            ..
        }) = pagination
        {
            let value = pagination_input_value(limit);
            format!(
                "        const initialLimit = {value}\n        if (initialLimit !== undefined && initialLimit !== null && (!Number.isSafeInteger(initialLimit) || Number(initialLimit) <= 0)) throw new Error('limit must be a positive safe integer')\n"
            )
        } else {
            String::new()
        };
        format!(
            "{validate_limit}        const initialPage = {value} ?? 1\n        if (!Number.isSafeInteger(initialPage) || Number(initialPage) < 0) throw new Error('page must be a nonnegative safe integer')\n        current = {update}\n        if (current === undefined) throw new Error('invalid pagination request body')\n"
        )
    } else {
        String::new()
    };
    let header = format!(
        "    {{\n      const paginationClient = {client_expression}\n      this.{public_name}Pages = async function* (options: Parameters<typeof {function}>[0]) {{\n        let current: unknown = options\n{initial}        for (let pageCount = 0; pageCount < 10000; pageCount++) {{\n          const response = await {function}({{ ...(current as Record<string, unknown>), client: paginationClient }} as Parameters<typeof {function}>[0])\n          yield response\n"
    );
    let footer =
        "        }\n        throw new Error('pagination exceeded 10000 pages')\n      }\n    }\n";
    let body = match pagination {
        Pagination::Cursor(pagination) => {
            let update = if pagination.input_location == "body" {
                format!(
                    "poolsterWithBodyValue(current, {:?}, cursor)",
                    pagination
                        .input_body_path
                        .as_deref()
                        .expect("validated request body cursor")
                )
            } else {
                format!(
                    "poolsterWithValue(current, {:?}, {:?}, cursor)",
                    pagination.input_location, pagination.input_name
                )
            };
            format!(
                "          const cursor = poolsterJsonPath(response, {:?})\n          if (cursor === undefined || cursor === null || cursor === '') return\n          const next = {update}\n          if (next === undefined) return\n          current = next\n",
                pagination.next_cursor_path
            )
        }
        Pagination::OffsetLimit(pagination) => render_offset_pagination(pagination),
        Pagination::Url(_) => unreachable!("URL pagination has its own iterator renderer"),
    };
    format!("{header}{body}{footer}")
}

/// URL pagination deliberately re-enters the generated operation through a
/// small `ClientInstance` wrapper instead of making a bare request. This is
/// important: the operation continues to supply its declared method,
/// security descriptor, serialization settings, and error policy. The
/// runtime treats `paginationUrl` as an internal, same-origin continuation
/// target; it is not a general-purpose URL override on public SDK methods.
fn render_url_pagination_iterator(
    public_name: &str,
    function: &str,
    pagination: &UrlPagination,
    client_expression: &str,
) -> String {
    format!(
        "    {{\n      const paginationClient = {client_expression}\n      this.{public_name}Pages = async function* (options: Parameters<typeof {function}>[0]) {{\n        let current: unknown = options\n        let nextUrl: string | undefined\n        while (true) {{\n          const request: ClientInstance = nextUrl === undefined\n            ? paginationClient\n            : (operation) => paginationClient({{ ...operation, paginationUrl: nextUrl }})\n          const requestOptions = nextUrl === undefined\n            ? (current as Record<string, unknown>)\n            : {{ ...(current as Record<string, unknown>), query: undefined }}\n          const response = await {function}({{ ...requestOptions, client: request }} as Parameters<typeof {function}>[0])\n          yield response\n          nextUrl = poolsterPaginationUrl(response, {:?})\n          if (nextUrl === undefined) return\n        }}\n      }}\n    }}\n",
        pagination.next_url_path,
    )
}

fn render_offset_pagination(pagination: &OffsetPagination) -> String {
    let (input, increment) = match &pagination.step {
        OffsetStep::Page(input) => (input, "currentValue + 1"),
        OffsetStep::Offset(input) => (input, "currentValue + items.length"),
    };
    let mut output = String::new();
    if let (OffsetStep::Page(_), Some(num_pages_path)) =
        (&pagination.step, &pagination.num_pages_path)
    {
        let current_value = pagination_input_value(input);
        let update = pagination_input_update(input, "nextValue");
        output.push_str(&format!(
            "          const currentValue = Number({current_value})\n          const nextValue = currentValue + 1\n          const numPages = Number(poolsterJsonPath(response, {:?}))\n          if (!Number.isSafeInteger(currentValue) || !Number.isSafeInteger(nextValue) || !Number.isSafeInteger(numPages) || numPages < 0 || nextValue > numPages) return\n          const next = {update}\n          if (next === undefined) return\n          current = next\n",
            num_pages_path,
        ));
        return output;
    }

    let results_path = pagination
        .results_path
        .as_deref()
        .expect("validated offset pagination has a results path");
    let limit = pagination.limit.as_ref().map_or_else(
        || "NaN".to_owned(),
        |input| format!("Number({})", pagination_input_value(input)),
    );
    let current_value = pagination_input_value(input);
    let update = pagination_input_update(input, "nextValue");
    output.push_str(&format!(
        "          const items = poolsterJsonPath(response, {:?})\n          if (!Array.isArray(items)) return\n          const configuredLimit = {limit}\n          if (items.length === 0 || (Number.isFinite(configuredLimit) && configuredLimit > 0 && items.length < configuredLimit)) return\n          const currentValue = Number({current_value})\n          const nextValue = {increment}\n          if (!Number.isSafeInteger(currentValue) || currentValue < 0 || !Number.isSafeInteger(nextValue)) return\n          const next = {update}\n          if (next === undefined) return\n          current = next\n",
        results_path,
    ));
    output
}

fn pagination_input_value(input: &PaginationInput) -> String {
    if input.location == "body" {
        format!(
            "poolsterBodyValue(current, {:?})",
            input
                .body_path
                .as_deref()
                .expect("validated request body pagination input")
        )
    } else {
        format!(
            "poolsterOptionValue(current, {:?}, {:?})",
            input.location, input.name
        )
    }
}

fn pagination_input_update(input: &PaginationInput, value: &str) -> String {
    if input.location == "body" {
        format!(
            "poolsterWithBodyValue(current, {:?}, {value})",
            input
                .body_path
                .as_deref()
                .expect("validated request body pagination input")
        )
    } else {
        format!(
            "poolsterWithValue(current, {:?}, {:?}, {value})",
            input.location, input.name
        )
    }
}

fn pagination_helpers() -> &'static str {
    "\nconst poolsterJsonPath = (value: unknown, path: string): unknown => {\n  const segments: (string | number)[] = []\n  if (path.startsWith('/')) {\n    for (const part of path.slice(1).split('/')) {\n      if (/~(?![01])/.test(part)) return undefined\n      segments.push(part.replace(/~1/g, '/').replace(/~0/g, '~'))\n    }\n  } else if (path.startsWith('$')) {\n    let rest = path.slice(1)\n    while (rest) {\n      const match = /^(?:\\.([^.[\\]]+)|\\[(-?\\d+)\\])/.exec(rest)\n      if (!match) return undefined\n      segments.push(match[1] ?? Number(match[2]))\n      rest = rest.slice(match[0].length)\n    }\n  } else return undefined\n  let current: unknown = value\n  for (const segment of segments) {\n    if (Array.isArray(current)) {\n      if (typeof segment === 'string' && !/^(?:0|[1-9]\\d*)$/.test(segment)) return undefined\n      const index = Number(segment)\n      if (!Number.isSafeInteger(index)) return undefined\n      const position = index < 0 ? current.length + index : index\n      if (!Object.hasOwn(current, position)) return undefined\n      current = current[position]\n    } else if (current && typeof current === 'object' && Object.hasOwn(current, segment)) current = (current as Record<string, unknown>)[segment]\n    else return undefined\n  }\n  return current\n}\nconst poolsterPaginationUrl = (response: unknown, path: string): string | undefined => {\n  const value = poolsterJsonPath(response, path)\n  return typeof value === 'string' && value.trim() ? value : undefined\n}\nconst poolsterOptionValue = (options: unknown, location: string, name: string): unknown => {\n  const current = (options ?? {}) as Record<string, unknown>\n  const section = current[location] as Record<string, unknown> | undefined\n  return section?.[name]\n}\nconst poolsterWithValue = (options: unknown, location: string, name: string, value: unknown): Record<string, unknown> => {\n  const current = (options ?? {}) as Record<string, unknown>\n  const section = (current[location] ?? {}) as Record<string, unknown>\n  return { ...current, [location]: { ...section, [name]: value } }\n}\n// `bodyPath` is an explicit RFC 6901 JSON Pointer. This helper creates new\n// objects/arrays only along that declared path; it never mutates caller input\n// and refuses to invent a missing array shape.\nconst poolsterJsonPointer = (path: string): string[] | undefined => {\n  if (!path.startsWith('/') || path.includes('//')) return undefined\n  return path.slice(1).split('/').map((part) => part.split('~1').join('/').split('~0').join('~'))\n}\nconst poolsterBodyValue = (options: unknown, path: string): unknown => {\n  const current = (options ?? {}) as Record<string, unknown>\n  const pointer = poolsterJsonPointer(path)\n  if (!pointer) return undefined\n  let value: unknown = current.body\n  for (const key of pointer) {\n    if (value === null || typeof value !== 'object') return undefined\n    value = Array.isArray(value) ? value[Number(key)] : (value as Record<string, unknown>)[key]\n  }\n  return value\n}\nconst poolsterWithBodyValue = (options: unknown, path: string, value: unknown): Record<string, unknown> | undefined => {\n  const current = (options ?? {}) as Record<string, unknown>\n  const pointer = poolsterJsonPointer(path)\n  if (!pointer || pointer.length === 0) return undefined\n  const update = (node: unknown, index: number): unknown | undefined => {\n    const key = pointer[index]\n    if (Array.isArray(node)) {\n      if (!/^\\d+$/.test(key)) return undefined\n      const position = Number(key)\n      if (!Number.isSafeInteger(position) || position < 0 || position >= node.length) return undefined\n      const copy = node.slice()\n      const next = index + 1 === pointer.length ? value : update(node[position], index + 1)\n      if (next === undefined) return undefined\n      copy[position] = next\n      return copy\n    }\n    if (node !== null && typeof node === 'object') {\n      const record = node as Record<string, unknown>\n      const next = index + 1 === pointer.length ? value : update(record[key] ?? {}, index + 1)\n      if (next === undefined) return undefined\n      return { ...record, [key]: next }\n    }\n    // An absent optional object can be created, but scalar and array shapes\n    // remain unrepresentable without a schema-directed declaration.\n    if (node === undefined || node === null) return update({}, index)\n    return undefined\n  }\n  const body = update(current.body ?? {}, 0)\n  return body === undefined ? undefined : { ...current, body }\n}\n"
}

pub(crate) fn sdk_client_name(api_name: &str) -> String {
    let name = pascal_identifier(api_name);
    let name = name
        .get(..name.len().saturating_sub(3))
        .filter(|_| name[name.len().saturating_sub(3)..].eq_ignore_ascii_case("api"))
        .unwrap_or(&name);
    if name.is_empty() {
        "ApiClient".into()
    } else {
        name.into()
    }
}

pub(crate) fn operation_group(operation: &Operation) -> String {
    operation_tag_directory_if_present(operation).unwrap_or_else(|| sdk_namespace(operation))
}

fn operation_tag_directory(operation: &Operation) -> String {
    sdk_namespace(operation)
}

fn operation_tag_directory_if_present(operation: &Operation) -> Option<String> {
    operation
        .annotations
        .get("tags")
        .and_then(serde_json::Value::as_array)
        .and_then(|tags| tags.first())
        .and_then(serde_json::Value::as_str)
        .map(lower_camel_identifier)
        .filter(|tag| !tag.is_empty())
}

pub(crate) fn poolster_package(
    api: &Api,
    transport: SdkTransport,
    name: Option<&str>,
) -> Result<String> {
    let package_name = name
        .map(str::to_owned)
        .unwrap_or_else(|| poolster_package_name(api, transport));
    let mut package = serde_json::json!({
        "name": package_name,
        "version": package_version(&api.version),
        "type": "module",
        "sideEffects": false,
        "exports": {
            ".": { "types": "./dist/index.d.ts", "import": "./dist/index.js" },
            "./*": { "types": "./dist/*.d.ts", "import": "./dist/*.js" }
        },
        "files": ["dist"],
        "scripts": { "build": "tsc -p tsconfig.json" },
        "devDependencies": { "typescript": "^5.9.3" }
    });
    if transport == SdkTransport::Axios {
        package["peerDependencies"] = serde_json::json!({ "axios": "^1.0.0" });
    }
    Ok(format!("{}\n", serde_json::to_string_pretty(&package)?))
}

fn poolster_package_name(api: &Api, transport: SdkTransport) -> String {
    format!(
        "@poolster/{}-{}",
        package_slug(&api.name),
        match transport {
            SdkTransport::Fetch => "fetch",
            SdkTransport::Axios => "axios",
        }
    )
}

fn poolster_readme(api: &Api, profile: &SdkConfig) -> String {
    let package_name = profile
        .package_name
        .clone()
        .unwrap_or_else(|| poolster_package_name(api, profile.transport));
    let client_name = profile
        .client_name
        .clone()
        .unwrap_or_else(|| sdk_client_name(&api.name));
    let example = api
        .operations
        .iter()
        .find(|operation| {
            operation.id.starts_with("list")
                && operation
                    .parameters
                    .iter()
                    .all(|parameter| !parameter.required)
                && operation
                    .request_body
                    .as_ref()
                    .is_none_or(|body| !body.required)
        })
        .or_else(|| {
            api.operations.iter().find(|operation| {
                operation
                    .parameters
                    .iter()
                    .all(|parameter| !parameter.required)
                    && operation
                        .request_body
                        .as_ref()
                        .is_none_or(|body| !body.required)
            })
        })
        .map(|operation| {
            let namespace = sdk_namespace(operation);
            let method = sdk_method_name(operation, &namespace);
            format!(
                "const response = await client.{namespace}.{method}({{}});\nconsole.log(response);"
            )
        })
        .unwrap_or_else(|| "// Call a generated resource method with its typed options.".into());
    let client = if profile.surface == SdkSurface::Client {
        format!(
            "import {{ {client_name} }} from {package_name:?};\n\nconst client = new {client_name}({{\n  baseUrl: \"https://api.example.com\",\n  apiKey: process.env.API_KEY,\n}});\n\n{example}"
        )
    } else {
        "// This package was generated with the raw surface; import the direct operation functions from the package entrypoint.".into()
    };
    let middleware = format!(
        "## Runtime middleware\n\nFetch and Axios clients accept `middleware` in their configuration.\n\n```ts\nimport {{ createClient, type ClientMiddleware }} from {package_name:?};\n\nconst customerPolicy: ClientMiddleware = async (request, next) => {{\n  try {{\n    return await next({{ ...request, query: {{ ...request.query, tenant: 'customer-a' }} }});\n  }} catch (cause) {{\n    throw new Error('Customer API request failed', {{ cause }});\n  }}\n}};\nconst transport = createClient({{ middleware: [customerPolicy] }});\n// Pass the same middleware config to the generated SDK constructor.\n```\n\nThe first middleware is outermost. Call `next(updatedRequest)` at most once; request changes apply before serialization and authentication. Replace the returned response envelope to rewrite data, or return an envelope without calling `next` to short circuit. A short circuit bypasses remaining middleware, transport hooks and transport validation callbacks. Optional response shape checks still run after the completed chain. Middleware runs once per logical call; built-in retries remain inside `next`. Existing hooks keep their transport timing. Ordinary responses contain `status`, `contentType`, `data` and `headers`. Preserve native Fetch `Response` objects for streams, and Axios stream envelopes with a readable `data` stream.\n"
    );
    let middleware = format!(
        "{middleware}\n## Response shape checks\n\nSet `validateResponses: true` in client configuration to check declared buffered successful JSON response shapes, including responses returned or rewritten by middleware. Checks are disabled by default; a request can override the setting. `ResponseDecodeError` reports the failing path without response values. Extra fields and new enum strings remain accepted. HEAD, 204, SSE, error responses, and absent or unmatched response schemas are outside this scope. These checks cover structural types, required properties, nullability, and supported compositions, rather than all JSON Schema constraints.\n"
    );
    format!(
        "# {} TypeScript SDK\n\nGenerated by Poolster.\n\n```sh\nnpm install {package_name}\n```\n\n```ts\n{client}\n```\n\nSee [STYLE_GUIDE.md](STYLE_GUIDE.md) for the selected client surface.\n\n{middleware}",
        api.name
    )
}

/// Normalizes common OpenAPI release labels into a version accepted by npm
/// and Cargo. APIs often publish a date (for example `2026-09-19`) rather
/// than a semver version.
fn package_version(version: &str) -> String {
    let pieces: Vec<_> = version.split('-').collect();
    if pieces.len() == 3
        && pieces.iter().all(|piece| {
            !piece.is_empty() && piece.chars().all(|character| character.is_ascii_digit())
        })
    {
        return pieces
            .iter()
            .map(|piece| piece.parse::<u64>().unwrap_or_default().to_string())
            .collect::<Vec<_>>()
            .join(".");
    }
    let semver_parts: Vec<_> = version.split('.').collect();
    if semver_parts.len() == 3
        && semver_parts.iter().all(|piece| {
            !piece.is_empty()
                && piece.chars().all(|character| {
                    character.is_ascii_digit()
                        || character == '-'
                        || character.is_ascii_alphabetic()
                })
        })
    {
        return version.to_owned();
    }
    "0.1.0".to_owned()
}

pub(crate) fn poolster_runtime(
    transport: SdkTransport,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> String {
    let security_types = render_security_types(security_schemes);
    let runtime = match transport {
        SdkTransport::Fetch => {
            format!(
                "{}\n{}",
                security_types,
                r#"export interface RetryConfig { maxAttempts?: number; initialDelayMs?: number; maxDelayMs?: number }
export interface RequestHookContext { method: string; url: string; body?: unknown; path?: Record<string, unknown>; query: Record<string, unknown>; headers: Headers }
export interface ResponseHookContext { request: RequestHookContext; status: number; response: Response }
export interface ClientHooks { beforeRequest?: (request: RequestHookContext) => void | Promise<void>; afterResponse?: (response: ResponseHookContext) => void | Promise<void>; onError?: (error: unknown, request: RequestHookContext) => void | Promise<void> }
export interface Codec { encode?: (value: unknown) => BodyInit | undefined; decode?: (value: unknown, contentType: string) => unknown | Promise<unknown> }
export interface StandardSchema { readonly ['~standard']?: { readonly validate: (value: unknown) => { value?: unknown; issues?: readonly unknown[] } | Promise<{ value?: unknown; issues?: readonly unknown[] }> } }
export type Validator = StandardSchema | ((value: unknown) => void | Promise<void>)
export interface ClientValidation { request?: Validator; response?: Validator }
export interface ClientConfig { timeoutMs?: number; baseUrl?: string; apiKey?: string; apiKeyHeader?: string; apiKeyPrefix?: string; auth?: SecurityCredentials; headers?: HeadersInit; fetch?: typeof globalThis.fetch; retry?: RetryConfig | false; middleware?: readonly ClientMiddleware[]; validateResponses?: boolean; hooks?: ClientHooks; codecs?: Record<string, Codec>; validation?: ClientValidation; multipartEncoder?: MultipartEncoder }
export type ParameterStyle = { contentType?: string; style?: 'simple' | 'label' | 'matrix' | 'form' | 'spaceDelimited' | 'pipeDelimited' | 'deepObject'; explode?: boolean }
export type ParameterStyles = Partial<Record<'path' | 'query' | 'header' | 'cookie', Record<string, ParameterStyle>>>
export type FormPartHeader = { required?: boolean; style?: ParameterStyle['style']; explode?: boolean; allowReserved?: boolean; schema_definition?: unknown; example_json?: string }
export type FormEncoding = { contentType?: string; headers?: Record<string, FormPartHeader>; style?: 'form' | 'spaceDelimited' | 'pipeDelimited' | 'deepObject'; explode?: boolean; allowReserved?: boolean; encoding?: Record<string, FormEncoding>; prefixEncoding?: FormEncoding[]; itemEncoding?: FormEncoding }
export type FormEncodings = Record<string, Record<string, FormEncoding>>
export type FormPartHeaders = Record<string, Record<string, Record<string, unknown>>>
export type MultipartPart = { name: string; value: string | Blob; contentType?: string; headers?: Record<string, string> }
export interface MultipartEncoder { encode(parts: readonly MultipartPart[]): { body: BodyInit; contentType: string } }
export type RequestConfig = { requestOptions?: RequestOptions; method: string; url: string; body?: unknown; path?: Record<string, unknown>; query?: Record<string, unknown>; querystring?: Record<string, unknown>; wholeQuery?: { name: string; contentType: string }; headers?: HeadersInit | Record<string, unknown>; cookies?: Record<string, unknown>; throwOnError?: boolean; security?: SecurityDescriptor[][]; contentType?: { request?: string }; responseType?: 'stream' | 'arraybuffer'; styles?: ParameterStyles; formEncodings?: FormEncodings; formHeaders?: FormPartHeaders; multipartPlan?: MultipartPlan; validation?: ClientValidation; validateResponses?: boolean; paginationUrl?: string; idempotencyHeader?: string }
export type ClientInstance = (request: RequestConfig) => Promise<unknown>
export type Options<T, ThrowOnError extends boolean> = T & { requestOptions?: RequestOptions; client?: ClientInstance; throwOnError?: ThrowOnError; formHeaders?: FormPartHeaders; multipartPlan?: MultipartPlan; validation?: ClientValidation }
export type SuccessOf<T> = T[Extract<keyof T, `2${string}`>]
type StatusCode<S> = S extends `${infer Code extends number}` ? Code : number
type MediaResult<S, T> = T extends { contentType: infer ContentType extends string; data: infer Data } ? { status: StatusCode<S>; contentType: ContentType; data: Data; headers: Headers } : { status: StatusCode<S>; contentType: string; data: T; headers: Headers }
export type StatusResult<T> = { [S in keyof T]: MediaResult<S, T[S]> }[keyof T]
export type SuccessResult<T> = { [S in Extract<keyof T, `2${string}`>]: MediaResult<S, T[S]> }[Extract<keyof T, `2${string}`>]
export type RequestResult<T, ThrowOnError extends boolean> = ThrowOnError extends true ? SuccessResult<T> : StatusResult<T>
export type ResponseResult<T extends { status: number; data: unknown }, ThrowOnError extends boolean> = ThrowOnError extends true ? T['data'] : T
export type EventStreamResult<T> = AsyncIterable<T>
const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value)
const serializeWholeQuery = (value: unknown, contentType: string): string => {
  if (value === undefined) return ''
  const media = contentType.split(';')[0].trim().toLowerCase()
  if (value === null && media !== 'application/json' && !media.endsWith('+json')) return ''
  if (media === 'text/plain') { const raw = String(value).replace(/^\?/, ''); if (/[#\r\n]/.test(raw)) throw new TypeError('Whole-query value contains a fragment or line break'); return raw }
  if (media === 'application/json' || media.endsWith('+json')) return encodeURIComponent(JSON.stringify(value))
  if (media !== 'application/x-www-form-urlencoded') throw new TypeError(`Unsupported whole-query content type: ${contentType}`)
  if (!isRecord(value)) throw new TypeError('Form whole-query value must be an object')
  const params = new URLSearchParams()
  for (const [name, item] of Object.entries(value)) {
    if (item === undefined || item === null) continue
    for (const part of Array.isArray(item) ? item : [item]) params.append(name, String(part))
  }
  return params.toString()
}
const appendWholeQuery = (url: string, querystring: Record<string, unknown> | undefined, metadata: { name: string; contentType: string } | undefined): string => {
  if (!metadata) return url
  const search = serializeWholeQuery(querystring?.[metadata.name], metadata.contentType)
  return search ? `${url}${url.includes('?') ? '&' : '?'}${search}` : url
}
const decodeSequence = (text: string, contentType: string, jsonPlan?: JsonPlan, status = 200): unknown[] => {
  if (new TextEncoder().encode(text).length > 64 * 1024 * 1024) throw new TypeError('Sequential response exceeds 64 MiB')
  const media = contentType.split(';')[0].trim().toLowerCase()
  if (media === 'application/json-seq' && text.split('\x1e')[0].trim()) throw new TypeError('JSON sequence must begin with a record separator')
  const records = media === 'application/json-seq' ? text.split('\x1e') : text.split(/\r?\n/)
  const values = records.map(record => record.trim()).filter(Boolean)
  if (values.length > 100000) throw new TypeError('Sequential response exceeds 100000 records')
  const itemShape = jsonPlan ? resolvedShape(responseJsonShape(jsonPlan, status, contentType), jsonPlan.refs)?.items : undefined
  return values.map(record => jsonPlan?.lossless ? parseJson(record, itemShape, jsonPlan.refs) : JSON.parse(record) as unknown)
}
const isJsonSequence = (contentType: string): boolean => ['application/json-seq', 'application/x-ndjson', 'application/ndjson', 'application/jsonl'].includes(contentType.split(';')[0].trim().toLowerCase())
const encodeSequence = (body: unknown, contentType: string, jsonPlan?: JsonPlan): string => {
  if (!Array.isArray(body)) throw new TypeError('Sequential JSON request body must be an array')
  return body.map(item => {
    const itemShape = jsonPlan ? resolvedShape(requestJsonShape(jsonPlan, contentType), jsonPlan.refs)?.items : undefined
    const encoded = jsonPlan?.lossless ? stringifyJson(item, itemShape, jsonPlan.refs) : JSON.stringify(item)
    if (encoded === undefined) throw new TypeError('Sequential JSON item must be serializable')
    return (contentType.split(';')[0].trim().toLowerCase() === 'application/json-seq' ? '\x1e' : '') + encoded + '\n'
  }).join('')
}
const styleFor = (styles: ParameterStyles | undefined, location: keyof ParameterStyles, name: string): Required<Omit<ParameterStyle, 'contentType'>> & Pick<ParameterStyle, 'contentType'> => {
  const configured = styles?.[location]?.[name] ?? {}
  const style = configured.style ?? (location === 'path' || location === 'header' ? 'simple' : 'form')
  return { style, explode: configured.explode ?? (style === 'form'), contentType: configured.contentType ?? '' }
}
const scalar = (value: unknown) => encodeURIComponent(String(value))
const contentParameter = (value: unknown, contentType: string): string => contentType.split(';')[0].includes('json') ? JSON.stringify(value) : String(value)
const pairs = (value: Record<string, unknown>, separator: string) => Object.entries(value).map(([key, item]) => `${scalar(key)}=${scalar(item)}`).join(separator)
const serializePath = (name: string, value: unknown, parameter: Required<Omit<ParameterStyle, 'contentType'>> & Pick<ParameterStyle, 'contentType'>) => {
  if (value === undefined || (value === null && !parameter.contentType)) return ''
  if (parameter.contentType) return encodeURIComponent(contentParameter(value, parameter.contentType))
  const values = Array.isArray(value) ? value : undefined
  const object = isRecord(value) ? value : undefined
  if (parameter.style === 'label') return `.${values ? values.map(scalar).join(parameter.explode ? '.' : ',') : object ? (parameter.explode ? pairs(object, '.') : Object.entries(object).flatMap(([key, item]) => [scalar(key), scalar(item)]).join(',')) : scalar(value)}`
  if (parameter.style === 'matrix') {
    if (values) return parameter.explode ? values.map((item) => `;${name}=${scalar(item)}`).join('') : `;${name}=${values.map(scalar).join(',')}`
    if (object) return parameter.explode ? `;${pairs(object, ';')}` : `;${name}=${Object.entries(object).flatMap(([key, item]) => [scalar(key), scalar(item)]).join(',')}`
    return `;${name}=${scalar(value)}`
  }
  if (values) return values.map(scalar).join(',')
  if (object) return parameter.explode ? pairs(object, ',') : Object.entries(object).flatMap(([key, item]) => [scalar(key), scalar(item)]).join(',')
  return scalar(value)
}
const appendQuery = (params: URLSearchParams, name: string, value: unknown, parameter: Required<Omit<ParameterStyle, 'contentType'>> & Pick<ParameterStyle, 'contentType'>) => {
  if (value === undefined || (value === null && !parameter.contentType)) return
  if (parameter.contentType) { params.append(name, contentParameter(value, parameter.contentType)); return }
  const values = Array.isArray(value) ? value : undefined
  const object = isRecord(value) ? value : undefined
  if (parameter.style === 'deepObject' && object) { for (const [key, item] of Object.entries(object)) params.append(`${name}[${key}]`, String(item)); return }
  if (object && parameter.explode) { for (const [key, item] of Object.entries(object)) params.append(key, String(item)); return }
  const separator = parameter.style === 'spaceDelimited' ? ' ' : parameter.style === 'pipeDelimited' ? '|' : ','
  if (values) { if (parameter.explode && parameter.style === 'form') values.forEach((item) => params.append(name, String(item))); else params.append(name, values.join(separator)); return }
  if (object) { params.append(name, Object.entries(object).flatMap(([key, item]) => [key, String(item)]).join(separator)); return }
  params.append(name, String(value))
}
const resolveUrl = (template: string, path?: Record<string, unknown>, query?: Record<string, unknown>, styles?: ParameterStyles) => {
  const url = template.replace(/\{([^}]+)\}/g, (_match, key) => serializePath(key, path?.[key], styleFor(styles, 'path', key)) || `{${key}}`)
  const params = new URLSearchParams()
  for (const [key, value] of Object.entries(query ?? {})) appendQuery(params, key, value, styleFor(styles, 'query', key))
  const search = params.toString()
  return search ? `${url}${url.includes('?') ? '&' : '?'}${search}` : url
}
const resolvePaginationUrl = (nextUrl: string, baseUrl?: string): string => {
  // A continuation URL comes from a response body. Only accept an absolute
  // URL on the configured API origin, or a root-relative URL when no base URL
  // is configured. This prevents credentials from being sent to another host.
  if (!baseUrl) {
    if (!nextUrl.startsWith('/') || nextUrl.startsWith('//')) throw new TypeError('Pagination URL must be root-relative when baseUrl is not configured')
    return nextUrl
  }
  const base = new URL(baseUrl)
  const target = new URL(nextUrl, base)
  if (target.origin !== base.origin) throw new TypeError('Pagination URL must use the configured API origin')
  return target.toString()
}
const codecFor = (codecs: Record<string, Codec> | undefined, contentType: string) => codecs?.[contentType.split(';')[0].trim()] ?? codecs?.['*/*']
const validate = async (validator: Validator | undefined, value: unknown) => { if (!validator) return; if (typeof validator === 'function') return validator(value); const result = await validator['~standard']?.validate(value); if (result?.issues?.length) throw new TypeError(`Poolster validation failed: ${result.issues.map(String).join(', ')}`) }
const formEntries = (name: string, value: unknown, encoding: FormEncoding = {}): Array<[string, string | Blob]> => {
  if (value === undefined || value === null) return []
  const style = encoding.style ?? 'form'; const explode = encoding.explode ?? true
  if (style === 'deepObject' && isRecord(value)) return Object.entries(value).flatMap(([key, item]) => formEntries(`${name}[${key}]`, item, { ...encoding, explode: false }))
  if (isRecord(value)) { if (explode) return Object.entries(value).flatMap(([key, item]) => formEntries(key, item, { ...encoding, explode: false })); const separator = style === 'spaceDelimited' ? ' ' : style === 'pipeDelimited' ? '|' : ','; return [[name, Object.entries(value).flatMap(([key, item]) => [key, String(item)]).join(separator)]] }
  if (Array.isArray(value)) { const separator = style === 'spaceDelimited' ? ' ' : style === 'pipeDelimited' ? '|' : ','; return explode ? value.flatMap((item) => formEntries(name, item, { ...encoding, explode: false })) : [[name, value.map(String).join(separator)]] }
  return [[name, value instanceof Blob ? value : String(value)]]
}
const formPartHeaders = (contentType: string, property: string, encoding: FormEncoding | undefined, values: FormPartHeaders | undefined): Record<string, string> | undefined => {
  const supplied = values?.[contentType]?.[property]
  const declared = encoding?.headers ?? {}
  for (const [name, header] of Object.entries(declared)) if (name.toLowerCase() !== 'content-type' && header.required && !Object.keys(supplied ?? {}).some((key) => key.toLowerCase() === name.toLowerCase())) throw new TypeError(`Missing required multipart header ${name} for ${property}`)
  if (!supplied) return undefined
  return Object.fromEntries(Object.entries(supplied).map(([name, value]) => {
    const header = Object.entries(declared).find(([declaredName]) => declaredName.toLowerCase() === name.toLowerCase())?.[1]
    return [name, serializePath(name, value, { style: header?.style ?? 'simple', explode: header?.explode ?? false })]
  }))
}
const multipartParts = (body: Record<string, unknown>, encodings: FormEncodings | undefined, contentType: string, values?: FormPartHeaders): MultipartPart[] => Object.entries(body).flatMap(([property, value]) => {
  const encoding = encodings?.[contentType]?.[property]
  const headers = formPartHeaders(contentType, property, encoding, values)
  return formEntries(property, value, encoding).map(([name, item]) => ({ name, value: item, contentType: encoding?.contentType, headers }))
})
const appendForm = (form: FormData, parts: readonly MultipartPart[]) => { for (const part of parts) { if (part.headers && Object.keys(part.headers).length) throw new TypeError('Native FormData cannot set per-part headers; configure client.multipartEncoder') ; const item = part.contentType ? new Blob([part.value], { type: part.contentType }) : part.value; form.append(part.name, item) } return form }
const formComponent = (value: string, allowReserved?: boolean) => allowReserved ? encodeURI(value).replace(/[&=]/g, (char) => char === '&' ? '%26' : '%3D') : encodeURIComponent(value)
const encodedForm = (body: Record<string, unknown>, encodings: FormEncodings | undefined, contentType: string) => Object.entries(body).flatMap(([key, value]) => { const encoding = encodings?.[contentType]?.[key]; return formEntries(key, value, encoding).map(([part, item]) => [part, typeof item === 'string' ? item : String(item), encoding?.allowReserved] as const) }).map(([part, item, allowReserved]) => `${formComponent(part, allowReserved)}=${formComponent(item, allowReserved)}`).join('&')
const requestBody = (body: unknown, headers: Headers, contentType: string | undefined, codecs?: Record<string, Codec>, formEncodings?: FormEncodings, formHeaders?: FormPartHeaders, multipartEncoder?: MultipartEncoder, multipartPlan?: MultipartPlan) => {
  if (body === undefined || body === null) return undefined
  const mediaType = contentType ?? headers.get('content-type') ?? 'application/json'
  if (multipartPlan) { const encoded = encodePlannedMultipart(body, multipartPlan, formHeaders); headers.set('content-type', encoded.type); return encoded }
  if (mediaType.startsWith('multipart/form-data') && isRecord(body)) { const parts = multipartParts(body, formEncodings, mediaType, formHeaders); if (multipartEncoder) { const encoded = multipartEncoder.encode(parts); headers.set('content-type', encoded.contentType); return encoded.body }; headers.delete('content-type'); return appendForm(new FormData(), parts) }
  const codec = codecFor(codecs, mediaType)
  if (codec?.encode) { if (!headers.has('content-type') && !mediaType.startsWith('multipart/form-data')) headers.set('content-type', mediaType); return codec.encode(body) }
  if (isJsonSequence(mediaType)) { if (!headers.has('content-type')) headers.set('content-type', mediaType); return encodeSequence(body, mediaType, jsonPlan) }
  if (body instanceof FormData || body instanceof URLSearchParams || body instanceof Blob || typeof body === 'string') return body as BodyInit
  if (mediaType.startsWith('application/x-www-form-urlencoded') && isRecord(body)) { if (!headers.has('content-type')) headers.set('content-type', mediaType); return encodedForm(body, formEncodings, mediaType) }
  if (!headers.has('content-type')) headers.set('content-type', mediaType)
  return JSON.stringify(body)
}
const responseBody = async (response: Response, codecs?: Record<string, Codec>) => {
  const contentType = response.headers.get('content-type') ?? ''
  const mediaType = contentType.split(';')[0].trim().toLowerCase()
  const codec = codecFor(codecs, contentType)
  if (codec?.decode) return codec.decode(await response.text(), contentType)
  if (isJsonSequence(contentType)) return decodeSequence(await response.text(), contentType, jsonPlan, response.status)
  if (mediaType.includes('json') || mediaType.endsWith('+json')) return response.json()
  if (mediaType.startsWith('text/') || mediaType.includes('xml') || mediaType.includes('yaml')) return response.text()
  return response.arrayBuffer()
}
const applySecurity = (headers: Headers, query: Record<string, unknown>, security: SecurityDescriptor[][] | undefined, credentials?: SecurityCredentials) => {
  if (!security) return
  const values: Record<string, string | undefined> = { ...credentials }
  const selected = security.find((alternative) => alternative.every((scheme) => values[scheme.id]))
  if (!selected) return
  for (const scheme of selected) {
    const value = values[scheme.id]
    if (!value) continue
    if (scheme.type === 'apiKey') {
      const name = scheme.name
      if (!name) continue
      if (scheme.in === 'query') query[name] = value
      else if (scheme.in === 'cookie') headers.append('cookie', `${name}=${encodeURIComponent(value)}`)
      else headers.set(name, value)
    } else headers.set('authorization', `${scheme.scheme === 'basic' ? 'Basic' : 'Bearer'} ${value}`)
  }
}
const retryableStatus = (status: number) => status === 408 || status === 429 || status === 500 || status === 502 || status === 503 || status === 504
const retryAllowed = (method: string, headers: Headers, idempotencyHeader?: string) => ['GET', 'HEAD', 'OPTIONS', 'TRACE', 'QUERY', 'PUT', 'DELETE'].includes(method.toUpperCase()) || (['POST', 'PATCH'].includes(method.toUpperCase()) && (!!headers.get('idempotency-key')?.trim() || (!!idempotencyHeader && !!headers.get(idempotencyHeader)?.trim())))
const retryHeaderDelay = (value: string | null | undefined, milliseconds: boolean): number | undefined => {
  if (!value) return undefined
  const trimmed = value.trim()
  if (/^\d+(?:\.\d+)?$/.test(trimmed)) {
    const delay = Number(trimmed) * (milliseconds ? 1 : 1000)
    return Number.isFinite(delay) ? delay : undefined
  }
  if (milliseconds || !/^[A-Za-z]/.test(trimmed)) return undefined
  const timestamp = Date.parse(trimmed)
  return Number.isFinite(timestamp) ? Math.max(0, timestamp - Date.now()) : undefined
}
const retryDelay = async (attempt: number, retry: RetryConfig, retryAfter?: string | null, retryAfterMilliseconds?: string | null, signal?: AbortSignal) => {
  const fromHeader = retryHeaderDelay(retryAfterMilliseconds, true) ?? retryHeaderDelay(retryAfter, false)
  const exponential = (retry.initialDelayMs ?? 250) * 2 ** attempt
  const delay = Math.max(0, Math.min(fromHeader ?? exponential, retry.maxDelayMs ?? 8_000))
  await requestRetryPause(delay, signal)
}
const createTransport = (config: ClientConfig = {}): ClientInstance => async ({ method, url, body, path, query, querystring, wholeQuery, headers, cookies, throwOnError: _throwOnError, security, contentType, responseType, styles, formEncodings, formHeaders, multipartPlan, validation, paginationUrl, idempotencyHeader, requestOptions }) => {
  const signal = requestOptions?.signal
  const mergedHeaders = new Headers(config.headers)
  if (config.apiKey) mergedHeaders.set(config.apiKeyHeader ?? 'authorization', `${config.apiKeyPrefix ?? 'Bearer '}${config.apiKey}`)
  new Headers(headers as HeadersInit).forEach((value, key) => mergedHeaders.set(key, value))
  new Headers(requestOptions?.headers).forEach((value, key) => mergedHeaders.set(key, value))
  const resolvedQuery = { ...(query ?? {}) }
  applySecurity(mergedHeaders, resolvedQuery, security, config.auth)
  for (const [name, value] of Object.entries(cookies ?? {})) { if (value !== undefined && (value !== null || styles?.cookie?.[name]?.contentType)) mergedHeaders.append('cookie', `${name}=${serializePath(name, value, styleFor(styles, 'cookie', name))}`) }
  for (const [name, value] of Object.entries(headers as Record<string, unknown> ?? {})) if (styles?.header?.[name]) mergedHeaders.set(name, styles.header[name]?.contentType ? contentParameter(value, styles.header[name]!.contentType!) : serializePath(name, value, styleFor(styles, 'header', name)))
  new Headers(requestOptions?.headers).forEach((value, key) => mergedHeaders.set(key, value))
  const requestUrl = paginationUrl ? resolvePaginationUrl(paginationUrl, config.baseUrl) : `${config.baseUrl ?? ''}${appendWholeQuery(resolveUrl(url, path, resolvedQuery, styles), querystring, wholeQuery)}`
  const request = { method, url: requestUrl, body, path, query: resolvedQuery, headers: mergedHeaders }
  await validate(validation?.request ?? config.validation?.request, body)
  await config.hooks?.beforeRequest?.(request)
  const retry = config.retry === false ? undefined : config.retry ?? {}
  const maxAttempts = retry && retryAllowed(method, mergedHeaders, idempotencyHeader) ? Math.max(1, retry.maxAttempts ?? 3) : 1
  let response!: Response
  for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
      signal?.throwIfAborted()
    try {
      response = await (config.fetch ?? globalThis.fetch)(requestUrl, { method, body: requestBody(body, mergedHeaders, contentType?.request, config.codecs, formEncodings, formHeaders, config.multipartEncoder, multipartPlan), headers: mergedHeaders, signal })
      if (attempt + 1 < maxAttempts && retryableStatus(response.status)) {
        await retryDelay(attempt, retry ?? {}, response.headers.get('retry-after'), response.headers.get('retry-after-ms'), signal)
        continue
      }
      break
    } catch (error) {
      if (signal?.aborted || (typeof error === 'object' && error !== null && 'name' in error && error.name === 'AbortError') || attempt + 1 >= maxAttempts) {
        await config.hooks?.onError?.(error, request)
        throw error
      }
      await retryDelay(attempt, retry ?? {}, undefined, undefined, signal)
    }
  }
  await config.hooks?.afterResponse?.({ request, status: response.status, response: response.clone() })
  if (!response.ok && _throwOnError !== false) {
    const error = new ApiError(response.status, await responseBody(response.clone(), config.codecs))
    await config.hooks?.onError?.(error, request)
    throw error
  }
  if (responseType === 'stream') return response
  const data = response.status === 204 ? undefined : await responseBody(response.clone(), config.codecs)
  await validate(validation?.response ?? config.validation?.response, data)
  return { status: response.status, contentType: response.headers.get('content-type')?.split(';')[0].trim() ?? '', data, headers: response.headers }
}
export const toEventStream = async <T>(response: Promise<unknown>): Promise<EventStreamResult<T>> => {
  const raw = await response
  if (!(raw instanceof Response) || !raw.body) throw new TypeError('SSE requires a Fetch Response with a readable body')
  const reader = raw.body.getReader()
  const decoder = new TextDecoder()
  return (async function* (): AsyncGenerator<T> {
    let buffer = ''
    while (true) {
      const next = await reader.read()
      if (next.done) break
      buffer += decoder.decode(next.value, { stream: true })
      const events = buffer.split(/\r?\n\r?\n/)
      buffer = events.pop() ?? ''
      for (const event of events) {
        const data = event.split(/\r?\n/).filter((line) => line.startsWith('data:')).map((line) => line.slice(5).trimStart()).join('\n')
        if (!data) continue
        try { yield JSON.parse(data) as T } catch { yield data as T }
      }
    }
  })()
}
/** Middleware runs once per logical request, around transport retries and legacy hooks. */
export type MiddlewareNext = (request: RequestConfig) => Promise<unknown>
export type ClientMiddleware = (request: RequestConfig, next: MiddlewareNext) => Promise<unknown>
/** Ordinary responses use this envelope; Fetch streaming requests return a native Response. */
export interface MiddlewareResponse { status: number; contentType: string; data: unknown; headers: Headers | Record<string, unknown> }
export const createClient = (config: ClientConfig = {}): ClientInstance => {
  const transport = createTransport(config)
  const middleware = [...(config.middleware ?? [])]
  return (request) => withRequestControl(request.requestOptions, config.timeoutMs, async (requestOptions) => {
    request = { ...request, requestOptions }
    const dispatch = (index: number, current: RequestConfig): Promise<unknown> => {
      const handler = middleware[index]
      if (!handler) return transport(current)
      let called = false
      return Promise.resolve().then(() => handler(current, (updated) => {
        if (called) return Promise.reject(new TypeError('Middleware next may only be called once'))
        called = true
        return dispatch(index + 1, updated)
      }))
    }
    return dispatch(0, { ...request, path: request.path ? { ...request.path } : undefined, query: request.query ? { ...request.query } : undefined, headers: (request.headers instanceof Headers ? new Headers(request.headers) : Array.isArray(request.headers) ? request.headers.map(([name, value]) => [name, value]) : request.headers ? { ...request.headers } : undefined) as RequestConfig['headers'], cookies: request.cookies ? { ...request.cookies } : undefined }).then((response) => (request.validateResponses ?? config.validateResponses) && request.responseType !== 'stream' && request.method.toUpperCase() !== 'HEAD' ? checkResponseEnvelope(response, request.jsonPlan) : response)
  })
}
export const client = createClient()
export const resolveResponse = <T extends { status: number; data: unknown }, ThrowOnError extends boolean>(promise: Promise<T>, throwOnError: ThrowOnError): Promise<ResponseResult<T, ThrowOnError>> => (throwOnError ? promise.then((result) => result.data) : promise) as Promise<ResponseResult<T, ThrowOnError>>
"#
            )
        }
        SdkTransport::Axios => {
            format!(
                "{}\n{}",
                security_types,
                r#"import axios, { type AxiosInstance } from 'axios'
export interface RetryConfig { maxAttempts?: number; initialDelayMs?: number; maxDelayMs?: number }
export interface RequestHookContext { method: string; url: string; body?: unknown; path?: Record<string, unknown>; query: Record<string, unknown>; headers: Record<string, string> }
export interface ResponseHookContext { request: RequestHookContext; status: number; headers: Record<string, unknown>; data: unknown }
export interface ClientHooks { beforeRequest?: (request: RequestHookContext) => void | Promise<void>; afterResponse?: (response: ResponseHookContext) => void | Promise<void>; onError?: (error: unknown, request: RequestHookContext) => void | Promise<void> }
export interface Codec { encode?: (value: unknown) => unknown; decode?: (value: unknown, contentType: string) => unknown | Promise<unknown> }
export interface StandardSchema { readonly ['~standard']?: { readonly validate: (value: unknown) => { value?: unknown; issues?: readonly unknown[] } | Promise<{ value?: unknown; issues?: readonly unknown[] }> } }
export type Validator = StandardSchema | ((value: unknown) => void | Promise<void>)
export interface ClientValidation { request?: Validator; response?: Validator }
export interface ClientConfig { timeoutMs?: number; baseUrl?: string; apiKey?: string; apiKeyHeader?: string; apiKeyPrefix?: string; auth?: SecurityCredentials; headers?: Record<string, string>; client?: AxiosInstance; retry?: RetryConfig | false; middleware?: readonly ClientMiddleware[]; validateResponses?: boolean; hooks?: ClientHooks; codecs?: Record<string, Codec>; validation?: ClientValidation; multipartEncoder?: MultipartEncoder }
export type ParameterStyle = { contentType?: string; style?: 'simple' | 'label' | 'matrix' | 'form' | 'spaceDelimited' | 'pipeDelimited' | 'deepObject'; explode?: boolean }
export type ParameterStyles = Partial<Record<'path' | 'query' | 'header' | 'cookie', Record<string, ParameterStyle>>>
export type FormPartHeader = { required?: boolean; style?: ParameterStyle['style']; explode?: boolean; allowReserved?: boolean; schema_definition?: unknown; example_json?: string }
export type FormEncoding = { contentType?: string; headers?: Record<string, FormPartHeader>; style?: 'form' | 'spaceDelimited' | 'pipeDelimited' | 'deepObject'; explode?: boolean; allowReserved?: boolean; encoding?: Record<string, FormEncoding>; prefixEncoding?: FormEncoding[]; itemEncoding?: FormEncoding }
export type FormEncodings = Record<string, Record<string, FormEncoding>>
export type FormPartHeaders = Record<string, Record<string, Record<string, unknown>>>
export type MultipartPart = { name: string; value: unknown; contentType?: string; headers?: Record<string, string> }
export interface MultipartEncoder { encode(parts: readonly MultipartPart[]): { body: unknown; contentType: string } }
export type RequestConfig = { requestOptions?: RequestOptions; method: string; url: string; body?: unknown; path?: Record<string, unknown>; query?: Record<string, unknown>; querystring?: Record<string, unknown>; wholeQuery?: { name: string; contentType: string }; headers?: Record<string, unknown>; cookies?: Record<string, unknown>; throwOnError?: boolean; security?: SecurityDescriptor[][]; contentType?: { request?: string }; responseType?: 'stream' | 'arraybuffer'; styles?: ParameterStyles; formEncodings?: FormEncodings; formHeaders?: FormPartHeaders; multipartPlan?: MultipartPlan; validation?: ClientValidation; validateResponses?: boolean; paginationUrl?: string; idempotencyHeader?: string }
export type ClientInstance = (request: RequestConfig) => Promise<unknown>
export type Options<T, ThrowOnError extends boolean> = T & { requestOptions?: RequestOptions; client?: ClientInstance; throwOnError?: ThrowOnError; formHeaders?: FormPartHeaders; multipartPlan?: MultipartPlan; validation?: ClientValidation }
export type SuccessOf<T> = T[Extract<keyof T, `2${string}`>]
type StatusCode<S> = S extends `${infer Code extends number}` ? Code : number
type MediaResult<S, T> = T extends { contentType: infer ContentType extends string; data: infer Data } ? { status: StatusCode<S>; contentType: ContentType; data: Data; headers: Record<string, unknown> } : { status: StatusCode<S>; contentType: string; data: T; headers: Record<string, unknown> }
export type StatusResult<T> = { [S in keyof T]: MediaResult<S, T[S]> }[keyof T]
export type SuccessResult<T> = { [S in Extract<keyof T, `2${string}`>]: MediaResult<S, T[S]> }[Extract<keyof T, `2${string}`>]
export type RequestResult<T, ThrowOnError extends boolean> = ThrowOnError extends true ? SuccessResult<T> : StatusResult<T>
export type ResponseResult<T extends { status: number; data: unknown }, ThrowOnError extends boolean> = ThrowOnError extends true ? T['data'] : T
export type EventStreamResult<T> = AsyncIterable<T>
const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value)
const serializeWholeQuery = (value: unknown, contentType: string): string => {
  if (value === undefined) return ''
  const media = contentType.split(';')[0].trim().toLowerCase()
  if (value === null && media !== 'application/json' && !media.endsWith('+json')) return ''
  if (media === 'text/plain') { const raw = String(value).replace(/^\?/, ''); if (/[#\r\n]/.test(raw)) throw new TypeError('Whole-query value contains a fragment or line break'); return raw }
  if (media === 'application/json' || media.endsWith('+json')) return encodeURIComponent(JSON.stringify(value))
  if (media !== 'application/x-www-form-urlencoded') throw new TypeError(`Unsupported whole-query content type: ${contentType}`)
  if (!isRecord(value)) throw new TypeError('Form whole-query value must be an object')
  const params = new URLSearchParams()
  for (const [name, item] of Object.entries(value)) {
    if (item === undefined || item === null) continue
    for (const part of Array.isArray(item) ? item : [item]) params.append(name, String(part))
  }
  return params.toString()
}
const appendWholeQuery = (url: string, querystring: Record<string, unknown> | undefined, metadata: { name: string; contentType: string } | undefined): string => {
  if (!metadata) return url
  const search = serializeWholeQuery(querystring?.[metadata.name], metadata.contentType)
  return search ? `${url}${url.includes('?') ? '&' : '?'}${search}` : url
}
const decodeSequence = (text: string, contentType: string, jsonPlan?: JsonPlan, status = 200): unknown[] => {
  if (new TextEncoder().encode(text).length > 64 * 1024 * 1024) throw new TypeError('Sequential response exceeds 64 MiB')
  const media = contentType.split(';')[0].trim().toLowerCase()
  if (media === 'application/json-seq' && text.split('\x1e')[0].trim()) throw new TypeError('JSON sequence must begin with a record separator')
  const records = media === 'application/json-seq' ? text.split('\x1e') : text.split(/\r?\n/)
  const values = records.map(record => record.trim()).filter(Boolean)
  if (values.length > 100000) throw new TypeError('Sequential response exceeds 100000 records')
  const itemShape = jsonPlan ? resolvedShape(responseJsonShape(jsonPlan, status, contentType), jsonPlan.refs)?.items : undefined
  return values.map(record => jsonPlan?.lossless ? parseJson(record, itemShape, jsonPlan.refs) : JSON.parse(record) as unknown)
}
const isJsonSequence = (contentType: string): boolean => ['application/json-seq', 'application/x-ndjson', 'application/ndjson', 'application/jsonl'].includes(contentType.split(';')[0].trim().toLowerCase())
const encodeSequence = (body: unknown, contentType: string, jsonPlan?: JsonPlan): string => {
  if (!Array.isArray(body)) throw new TypeError('Sequential JSON request body must be an array')
  return body.map(item => {
    const itemShape = jsonPlan ? resolvedShape(requestJsonShape(jsonPlan, contentType), jsonPlan.refs)?.items : undefined
    const encoded = jsonPlan?.lossless ? stringifyJson(item, itemShape, jsonPlan.refs) : JSON.stringify(item)
    if (encoded === undefined) throw new TypeError('Sequential JSON item must be serializable')
    return (contentType.split(';')[0].trim().toLowerCase() === 'application/json-seq' ? '\x1e' : '') + encoded + '\n'
  }).join('')
}
const styleFor = (styles: ParameterStyles | undefined, location: keyof ParameterStyles, name: string): Required<Omit<ParameterStyle, 'contentType'>> & Pick<ParameterStyle, 'contentType'> => {
  const configured = styles?.[location]?.[name] ?? {}
  const style = configured.style ?? (location === 'path' || location === 'header' ? 'simple' : 'form')
  return { style, explode: configured.explode ?? (style === 'form'), contentType: configured.contentType ?? '' }
}
const scalar = (value: unknown) => encodeURIComponent(String(value))
const contentParameter = (value: unknown, contentType: string): string => contentType.split(';')[0].includes('json') ? JSON.stringify(value) : String(value)
const pairs = (value: Record<string, unknown>, separator: string) => Object.entries(value).map(([key, item]) => `${scalar(key)}=${scalar(item)}`).join(separator)
const serializePath = (name: string, value: unknown, parameter: Required<Omit<ParameterStyle, 'contentType'>> & Pick<ParameterStyle, 'contentType'>) => {
  if (value === undefined || (value === null && !parameter.contentType)) return ''
  if (parameter.contentType) return encodeURIComponent(contentParameter(value, parameter.contentType))
  const values = Array.isArray(value) ? value : undefined
  const object = isRecord(value) ? value : undefined
  if (parameter.style === 'label') return `.${values ? values.map(scalar).join(parameter.explode ? '.' : ',') : object ? (parameter.explode ? pairs(object, '.') : Object.entries(object).flatMap(([key, item]) => [scalar(key), scalar(item)]).join(',')) : scalar(value)}`
  if (parameter.style === 'matrix') { if (values) return parameter.explode ? values.map((item) => `;${name}=${scalar(item)}`).join('') : `;${name}=${values.map(scalar).join(',')}`; if (object) return parameter.explode ? `;${pairs(object, ';')}` : `;${name}=${Object.entries(object).flatMap(([key, item]) => [scalar(key), scalar(item)]).join(',')}`; return `;${name}=${scalar(value)}` }
  if (values) return values.map(scalar).join(',')
  if (object) return parameter.explode ? pairs(object, ',') : Object.entries(object).flatMap(([key, item]) => [scalar(key), scalar(item)]).join(',')
  return scalar(value)
}
const serializeQuery = (query: Record<string, unknown>, styles?: ParameterStyles) => {
  const output: Record<string, unknown> = {}
  for (const [name, value] of Object.entries(query)) { const parameter = styleFor(styles, 'query', name); if (value === undefined || (value === null && !parameter.contentType)) continue; if (parameter.contentType) { output[name] = contentParameter(value, parameter.contentType); continue }; if (parameter.style === 'deepObject' && isRecord(value)) { for (const [key, item] of Object.entries(value)) output[`${name}[${key}]`] = item } else if (isRecord(value) && parameter.explode) Object.assign(output, value); else output[name] = Array.isArray(value) ? (parameter.explode && parameter.style === 'form' ? value : value.join(parameter.style === 'spaceDelimited' ? ' ' : parameter.style === 'pipeDelimited' ? '|' : ',')) : isRecord(value) ? Object.entries(value).flatMap(([key, item]) => [key, String(item)]).join(',') : value }
  return output
}
const resolvePath = (template: string, path?: Record<string, unknown>, styles?: ParameterStyles) => template.replace(/\{([^}]+)\}/g, (_match, key) => serializePath(key, path?.[key], styleFor(styles, 'path', key)) || `{${key}}`)
const codecFor = (codecs: Record<string, Codec> | undefined, contentType: string) => codecs?.[contentType.split(';')[0].trim()] ?? codecs?.['*/*']
const validate = async (validator: Validator | undefined, value: unknown) => { if (!validator) return; if (typeof validator === 'function') return validator(value); const result = await validator['~standard']?.validate(value); if (result?.issues?.length) throw new TypeError(`Poolster validation failed: ${result.issues.map(String).join(', ')}`) }
const formEntries = (name: string, value: unknown, encoding: FormEncoding = {}): Array<[string, unknown]> => {
  if (value === undefined || value === null) return []
  const style = encoding.style ?? 'form'; const explode = encoding.explode ?? true
  if (style === 'deepObject' && isRecord(value)) return Object.entries(value).flatMap(([key, item]) => formEntries(`${name}[${key}]`, item, { ...encoding, explode: false }))
  if (isRecord(value)) { if (explode) return Object.entries(value).flatMap(([key, item]) => formEntries(key, item, { ...encoding, explode: false })); const separator = style === 'spaceDelimited' ? ' ' : style === 'pipeDelimited' ? '|' : ','; return [[name, Object.entries(value).flatMap(([key, item]) => [key, String(item)]).join(separator)]] }
  if (Array.isArray(value)) { const separator = style === 'spaceDelimited' ? ' ' : style === 'pipeDelimited' ? '|' : ','; return explode ? value.flatMap((item) => formEntries(name, item, { ...encoding, explode: false })) : [[name, value.map(String).join(separator)]] }
  return [[name, value]]
}
const formPartHeaders = (contentType: string, property: string, encoding: FormEncoding | undefined, values: FormPartHeaders | undefined): Record<string, string> | undefined => {
  const supplied = values?.[contentType]?.[property]
  const declared = encoding?.headers ?? {}
  for (const [name, header] of Object.entries(declared)) if (name.toLowerCase() !== 'content-type' && header.required && !Object.keys(supplied ?? {}).some((key) => key.toLowerCase() === name.toLowerCase())) throw new TypeError(`Missing required multipart header ${name} for ${property}`)
  if (!supplied) return undefined
  return Object.fromEntries(Object.entries(supplied).map(([name, value]) => {
    const header = Object.entries(declared).find(([declaredName]) => declaredName.toLowerCase() === name.toLowerCase())?.[1]
    return [name, serializePath(name, value, { style: header?.style ?? 'simple', explode: header?.explode ?? false })]
  }))
}
const multipartParts = (body: Record<string, unknown>, encodings: FormEncodings | undefined, contentType: string, values?: FormPartHeaders): MultipartPart[] => Object.entries(body).flatMap(([property, value]) => {
  const encoding = encodings?.[contentType]?.[property]
  const headers = formPartHeaders(contentType, property, encoding, values)
  return formEntries(property, value, encoding).map(([name, item]) => ({ name, value: item, contentType: encoding?.contentType, headers }))
})
const appendForm = (form: FormData, parts: readonly MultipartPart[]) => { for (const part of parts) { if (part.headers && Object.keys(part.headers).length) throw new TypeError('Native FormData cannot set per-part headers; configure client.multipartEncoder'); const item = part.value instanceof Blob && part.contentType ? new Blob([part.value], { type: part.contentType }) : part.value instanceof Blob ? part.value : String(part.value); form.append(part.name, item) } return form }
const formComponent = (value: string, allowReserved?: boolean) => allowReserved ? encodeURI(value).replace(/[&=]/g, (char) => char === '&' ? '%26' : '%3D') : encodeURIComponent(value)
const encodeForm = (body: Record<string, unknown>, encodings: FormEncodings | undefined, contentType: string) => Object.entries(body).flatMap(([key, value]) => { const encoding = encodings?.[contentType]?.[key]; return formEntries(key, value, encoding).map(([part, item]) => [part, item, encoding?.allowReserved] as const) }).map(([key, value, allowReserved]) => `${formComponent(key, allowReserved)}=${formComponent(String(value), allowReserved)}`).join('&')
const encodeBody = (body: unknown, headers: Record<string, string>, contentType: string | undefined, codecs?: Record<string, Codec>, formEncodings?: FormEncodings, formHeaders?: FormPartHeaders, multipartEncoder?: MultipartEncoder, multipartPlan?: MultipartPlan) => {
  if (body === undefined || body === null) return undefined
  const mediaType = contentType ?? 'application/json'
  if (multipartPlan) { const encoded = encodePlannedMultipart(body, multipartPlan, formHeaders); headers['content-type'] = encoded.type; return encoded }
  if (mediaType.startsWith('multipart/form-data') && isRecord(body)) { const parts = multipartParts(body, formEncodings, mediaType, formHeaders); if (multipartEncoder) { const encoded = multipartEncoder.encode(parts); headers['content-type'] = encoded.contentType; return encoded.body }; delete headers['content-type']; return appendForm(new FormData(), parts) }
  const codec = codecFor(codecs, mediaType)
  if (codec?.encode) return codec.encode(body)
  if (body instanceof FormData || body instanceof URLSearchParams || body instanceof Blob || typeof body === 'string') return body
  if (isJsonSequence(mediaType)) { headers['content-type'] ??= mediaType; return encodeSequence(body, mediaType, jsonPlan) }
  if (mediaType.startsWith('application/x-www-form-urlencoded') && isRecord(body)) return encodeForm(body, formEncodings, mediaType)
  return body
}
const resolvePaginationUrl = (nextUrl: string, baseUrl?: string): string => {
  // See the Fetch runtime: a response-supplied continuation must never be
  // allowed to carry configured credentials to a different origin.
  if (!baseUrl) {
    if (!nextUrl.startsWith('/') || nextUrl.startsWith('//')) throw new TypeError('Pagination URL must be root-relative when baseUrl is not configured')
    return nextUrl
  }
  const base = new URL(baseUrl)
  const target = new URL(nextUrl, base)
  if (target.origin !== base.origin) throw new TypeError('Pagination URL must use the configured API origin')
  return target.toString()
}
const applySecurity = (headers: Record<string, string>, query: Record<string, unknown>, security: SecurityDescriptor[][] | undefined, credentials?: SecurityCredentials) => {
  if (!security) return
  const values: Record<string, string | undefined> = { ...credentials }
  const selected = security.find((alternative) => alternative.every((scheme) => values[scheme.id]))
  if (!selected) return
  for (const scheme of selected) {
    const value = values[scheme.id]
    if (!value) continue
    if (scheme.type === 'apiKey') {
      const name = scheme.name
      if (!name) continue
      if (scheme.in === 'query') query[name] = value
      else if (scheme.in === 'cookie') headers.cookie = [headers.cookie, `${name}=${encodeURIComponent(value)}`].filter(Boolean).join('; ')
      else headers[name] = value
    } else headers.authorization = `${scheme.scheme === 'basic' ? 'Basic' : 'Bearer'} ${value}`
  }
}
const retryableStatus = (status: number) => status === 408 || status === 429 || status === 500 || status === 502 || status === 503 || status === 504
const retryAllowed = (method: string, headers: Record<string, string>, idempotencyHeader?: string) => ['GET', 'HEAD', 'OPTIONS', 'TRACE', 'QUERY', 'PUT', 'DELETE'].includes(method.toUpperCase()) || (['POST', 'PATCH'].includes(method.toUpperCase()) && Object.entries(headers).some(([key, value]) => !!value.trim() && (key.toLowerCase() === 'idempotency-key' || (!!idempotencyHeader && key.toLowerCase() === idempotencyHeader.toLowerCase()))))
const retryHeaderDelay = (value: string | null | undefined, milliseconds: boolean): number | undefined => {
  if (!value) return undefined
  const trimmed = value.trim()
  if (/^\d+(?:\.\d+)?$/.test(trimmed)) {
    const delay = Number(trimmed) * (milliseconds ? 1 : 1000)
    return Number.isFinite(delay) ? delay : undefined
  }
  if (milliseconds || !/^[A-Za-z]/.test(trimmed)) return undefined
  const timestamp = Date.parse(trimmed)
  return Number.isFinite(timestamp) ? Math.max(0, timestamp - Date.now()) : undefined
}
const retryDelay = async (attempt: number, retry: RetryConfig, retryAfter?: string | null, retryAfterMilliseconds?: string | null, signal?: AbortSignal) => {
  const fromHeader = retryHeaderDelay(retryAfterMilliseconds, true) ?? retryHeaderDelay(retryAfter, false)
  const exponential = (retry.initialDelayMs ?? 250) * 2 ** attempt
  const delay = Math.max(0, Math.min(fromHeader ?? exponential, retry.maxDelayMs ?? 8_000))
  await requestRetryPause(delay, signal)
}
const createTransport = (config: ClientConfig = {}): ClientInstance => {
  const headers = { ...config.headers }
  if (config.apiKey) headers[config.apiKeyHeader ?? 'authorization'] = `${config.apiKeyPrefix ?? 'Bearer '}${config.apiKey}`
  const instance = config.client ?? axios.create({ baseURL: config.baseUrl, headers })
  return async ({ method, url, body, path, query, querystring, wholeQuery, headers: requestHeaders, cookies, throwOnError: _throwOnError, security, contentType, responseType, styles, formEncodings, formHeaders, multipartPlan, validation, paginationUrl, idempotencyHeader, requestOptions }) => {
    const signal = requestOptions?.signal
    const resolvedHeaders: Record<string, string> = { ...headers, ...Object.fromEntries(Object.entries(requestHeaders ?? {}).map(([key, value]) => [key, String(value)])) }
    new Headers(requestOptions?.headers).forEach((value, key) => { resolvedHeaders[key] = value })
    const resolvedQuery = { ...(query ?? {}) }
    applySecurity(resolvedHeaders, resolvedQuery, security, config.auth)
    for (const [name, value] of Object.entries(requestHeaders ?? {})) if (styles?.header?.[name]) resolvedHeaders[name] = styles.header[name]?.contentType ? contentParameter(value, styles.header[name]!.contentType!) : serializePath(name, value, styleFor(styles, 'header', name))
    new Headers(requestOptions?.headers).forEach((value, key) => { resolvedHeaders[key] = value })
    if (contentType?.request && !contentType.request.startsWith('multipart/form-data')) resolvedHeaders['content-type'] ??= contentType.request
    if (cookies) resolvedHeaders.cookie = [...(resolvedHeaders.cookie ? [resolvedHeaders.cookie] : []), ...Object.entries(cookies).filter(([name, value]) => value !== undefined && (value !== null || styles?.cookie?.[name]?.contentType)).map(([name, value]) => `${name}=${serializePath(name, value, styleFor(styles, 'cookie', name))}`)].join('; ')
    const requestUrl = paginationUrl ? resolvePaginationUrl(paginationUrl, config.baseUrl) : appendWholeQuery(resolvePath(url, path, styles), querystring, wholeQuery)
    const request = { method, url: requestUrl, body, path, query: resolvedQuery, headers: resolvedHeaders }
    await validate(validation?.request ?? config.validation?.request, body)
    await config.hooks?.beforeRequest?.(request)
    const retry = config.retry === false ? undefined : config.retry ?? {}
    const maxAttempts = retry && retryAllowed(method, resolvedHeaders, idempotencyHeader) ? Math.max(1, retry.maxAttempts ?? 3) : 1
    for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
      signal?.throwIfAborted()
      let response
      try {
        response = await instance.request({ method, url: requestUrl, data: encodeBody(body, resolvedHeaders, contentType?.request, config.codecs, formEncodings, formHeaders, config.multipartEncoder, multipartPlan), params: serializeQuery(resolvedQuery, styles), headers: resolvedHeaders, signal, responseType: responseType === 'stream' ? 'stream' : responseType === 'arraybuffer' ? 'arraybuffer' : undefined, validateStatus: () => true })
      } catch (error) {
        // validateStatus above keeps HTTP responses out of this branch. Only
        // adapter/network failures retry; a hook, codec, or validator failure
        // must remain a single terminal outcome.
        if (signal?.aborted || axios.isCancel(error) || !axios.isAxiosError(error) || attempt + 1 >= maxAttempts) {
          await config.hooks?.onError?.(error, request)
          throw error
        }
        await retryDelay(attempt, retry ?? {}, undefined, undefined, signal)
        continue
      }
      if (attempt + 1 < maxAttempts && retryableStatus(response.status)) {
        await retryDelay(attempt, retry ?? {}, response.headers['retry-after'], response.headers['retry-after-ms'], signal)
        continue
      }
      const mediaType = String(response.headers['content-type'] ?? '')
      const data = await codecFor(config.codecs, mediaType)?.decode?.(response.data, mediaType) ?? (isJsonSequence(mediaType) ? decodeSequence(String(response.data), mediaType, jsonPlan, response.status) : (jsonPlan?.lossless && typeof response.data === 'string' && (mediaType.includes('json') || mediaType.includes('+json')) ? parseJson(response.data, responseJsonShape(jsonPlan, response.status, mediaType), jsonPlan.refs) : response.data))
      try {
        await config.hooks?.afterResponse?.({ request, status: response.status, headers: response.headers as Record<string, unknown>, data })
        if (response.status >= 400 && _throwOnError !== false) throw new ApiError(response.status, data)
        if (responseType !== 'stream') await validate(validation?.response ?? config.validation?.response, data)
        return { status: response.status, contentType: mediaType.split(';')[0].trim(), data, headers: response.headers as Record<string, unknown> }
      } catch (error) {
        await config.hooks?.onError?.(error, request)
        throw error
      }
    }
    throw new Error('Poolster retry loop completed without a response')
  }
}
export const toEventStream = async <T>(response: Promise<unknown>): Promise<EventStreamResult<T>> => {
  const raw = await response
  const stream = raw instanceof Response ? raw.body : raw as ReadableStream<Uint8Array> | null
  if (!stream || typeof stream.getReader !== 'function') throw new TypeError('SSE requires an Axios stream-capable adapter')
  const reader = stream.getReader()
  const decoder = new TextDecoder()
  return (async function* (): AsyncGenerator<T> {
    let buffer = ''
    while (true) {
      const next = await reader.read()
      if (next.done) break
      buffer += decoder.decode(next.value, { stream: true })
      const events = buffer.split(/\r?\n\r?\n/)
      buffer = events.pop() ?? ''
      for (const event of events) {
        const data = event.split(/\r?\n/).filter((line) => line.startsWith('data:')).map((line) => line.slice(5).trimStart()).join('\n')
        if (!data) continue
        try { yield JSON.parse(data) as T } catch { yield data as T }
      }
    }
  })()
}
/** Middleware runs once per logical request, around transport retries and legacy hooks. */
export type MiddlewareNext = (request: RequestConfig) => Promise<unknown>
export type ClientMiddleware = (request: RequestConfig, next: MiddlewareNext) => Promise<unknown>
/** Ordinary responses use this envelope; Fetch streaming requests return a native Response. */
export interface MiddlewareResponse { status: number; contentType: string; data: unknown; headers: Headers | Record<string, unknown> }
export const createClient = (config: ClientConfig = {}): ClientInstance => {
  const transport = createTransport(config)
  const middleware = [...(config.middleware ?? [])]
  return (request) => withRequestControl(request.requestOptions, config.timeoutMs, async (requestOptions) => {
    request = { ...request, requestOptions }
    const dispatch = (index: number, current: RequestConfig): Promise<unknown> => {
      const handler = middleware[index]
      if (!handler) return transport(current)
      let called = false
      return Promise.resolve().then(() => handler(current, (updated) => {
        if (called) return Promise.reject(new TypeError('Middleware next may only be called once'))
        called = true
        return dispatch(index + 1, updated)
      }))
    }
    return dispatch(0, { ...request, path: request.path ? { ...request.path } : undefined, query: request.query ? { ...request.query } : undefined, headers: (request.headers instanceof Headers ? new Headers(request.headers) : Array.isArray(request.headers) ? request.headers.map(([name, value]) => [name, value]) : request.headers ? { ...request.headers } : undefined) as RequestConfig['headers'], cookies: request.cookies ? { ...request.cookies } : undefined }).then((response) => (request.validateResponses ?? config.validateResponses) && request.responseType !== 'stream' && request.method.toUpperCase() !== 'HEAD' ? checkResponseEnvelope(response, request.jsonPlan) : response)
  })
}
export const client = createClient()
export const resolveResponse = <T extends { status: number; data: unknown }, ThrowOnError extends boolean>(promise: Promise<T>, throwOnError: ThrowOnError): Promise<ResponseResult<T, ThrowOnError>> => (throwOnError ? promise.then((result) => result.data) : promise) as Promise<ResponseResult<T, ThrowOnError>>
"#
            )
        }
    };
    let runtime = format!(
        "{}\n{}",
        runtime,
        include_str!("../templates/multipart32.ts.tmpl")
    );
    let runtime = runtime.replace("paginationUrl?: string; idempotencyHeader?: string }", "paginationUrl?: string; idempotencyHeader?: string; jsonPlan?: JsonPlan }")
        .replace("validation, paginationUrl, idempotencyHeader, requestOptions })", "validation, paginationUrl, idempotencyHeader, requestOptions, jsonPlan })")
        .replace("formHeaders, config.multipartEncoder, multipartPlan)", "formHeaders, config.multipartEncoder, multipartPlan, jsonPlan)")
        .replace("multipartEncoder?: MultipartEncoder, multipartPlan?: MultipartPlan) =>", "multipartEncoder?: MultipartEncoder, multipartPlan?: MultipartPlan, jsonPlan?: JsonPlan) =>")
        .replace("return JSON.stringify(body)", "return jsonPlan?.lossless ? stringifyJson(body, requestJsonShape(jsonPlan, mediaType), jsonPlan.refs) : JSON.stringify(body)")
        .replace("const responseBody = async (response: Response, codecs?: Record<string, Codec>)", "const responseBody = async (response: Response, codecs?: Record<string, Codec>, jsonPlan?: JsonPlan)")
        .replace("return response.json()", "return jsonPlan?.lossless ? parseJson(await response.text(), responseJsonShape(jsonPlan, response.status, contentType), jsonPlan.refs) : response.json()")
        .replace("responseBody(response.clone(), config.codecs)", "responseBody(response.clone(), config.codecs, jsonPlan)")
        .replace("  return body\n}", "  return jsonPlan?.lossless && (mediaType.includes('json') || mediaType.endsWith('+json')) ? stringifyJson(body, requestJsonShape(jsonPlan, mediaType), jsonPlan.refs) : body\n}")
        .replace("responseType === 'arraybuffer' ? 'arraybuffer' : undefined", "responseType === 'arraybuffer' ? 'arraybuffer' : jsonPlan?.lossless ? 'text' : undefined")
        .replace("validateStatus: () => true })", "validateStatus: () => true, ...(jsonPlan?.lossless && responseType !== 'stream' && responseType !== 'arraybuffer' ? { transformResponse: [(value: unknown) => value] } : {}) })")
        .replace("?? response.data", "?? (jsonPlan?.lossless && typeof response.data === 'string' && (mediaType.includes('json') || mediaType.includes('+json')) ? parseJson(response.data, responseJsonShape(jsonPlan, response.status, mediaType), jsonPlan.refs) : response.data)")
        .replace("(response: Promise<unknown>): Promise<EventStreamResult<T>>", "(response: Promise<unknown>, jsonPlan?: JsonPlan): Promise<EventStreamResult<T>>")
        .replace("const stream = raw instanceof Response ? raw.body : raw as ReadableStream<Uint8Array> | null", "const stream = raw instanceof Response ? raw.body : (raw && typeof raw === 'object' && 'data' in raw ? raw.data : raw) as ReadableStream<Uint8Array> | null")
        .replace("yield JSON.parse(data) as T", "yield (jsonPlan ? parseJson(data, responseJsonShape(jsonPlan, eventStreamStatus(raw), 'text/event-stream'), jsonPlan.refs) : JSON.parse(data)) as T");
    format!(
        "{}\n{}\n{}",
        runtime,
        include_str!("../templates/request_control.ts.tmpl"),
        crate::json::RUNTIME
    )
}

fn render_security_types(security_schemes: Option<&SecuritySchemeCatalog>) -> String {
    let fields = security_schemes
        .map(|catalog| {
            catalog
                .schemes
                .iter()
                .map(|scheme| {
                    format!(
                        "  {}?: string",
                        serde_json::to_string(&scheme.name).unwrap()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|fields| !fields.is_empty())
        .unwrap_or_else(|| "  [name: string]: string | undefined".into());
    format!(
        "export interface SecurityCredentials {{\n{fields}\n}}\nexport type SecurityDescriptor = {{ id: string; type: 'apiKey' | 'http' | 'oauth2'; name?: string; in?: 'header' | 'query' | 'cookie'; scheme?: string; scopes?: string[] }}\nexport class ApiError extends Error {{\n  constructor(public readonly status: number, public readonly body: unknown) {{ super(`Request failed: ${{status}}`) }}\n}}"
    )
}

fn pascal_identifier(value: &str) -> String {
    crate::symbols::identifier(value)
}

pub(crate) fn lower_camel_identifier(value: &str) -> String {
    crate::symbols::camel(value)
}

fn package_slug(value: &str) -> String {
    let slug = value
        .chars()
        .flat_map(char::to_lowercase)
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    match slug.trim_matches('-') {
        "" => "api".into(),
        value => value.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use poolster_core::{
        HttpMethod, OperationMediaType, OperationRequestBody, SecurityRequirement, SecurityScheme,
        SecuritySchemeKind,
    };
    #[test]
    fn structured_typescript_sdk_uses_named_catalog_credentials() {
        let api = Api {
            name: "Secure API".into(),
            version: "1.0.0".into(),
            operations: vec![Operation {
                id: "listMessages".into(),
                method: HttpMethod::Get,
                path: "/messages".into(),
                responses: vec![poolster_core::OperationResponse {
                    status: "200".into(),
                    description: None,
                    media_types: vec![poolster_core::OperationMediaType {
                        content_type: "application/json".into(),
                        schema: Some(poolster_core::SchemaValue::reference(
                            "#/components/schemas/MessageList",
                        )),
                    }],
                }],
                security: vec![SecurityRequirement {
                    schemes: [("poolsterApiKey".into(), Vec::new())]
                        .into_iter()
                        .collect(),
                }],
                ..Operation::default()
            }],
            ..Api::default()
        };
        let catalog = SecuritySchemeCatalog {
            schemes: vec![SecurityScheme {
                name: "poolsterApiKey".into(),
                description: None,
                kind: SecuritySchemeKind::ApiKey {
                    name: Some("X-API-Key".into()),
                    location: Some("header".into()),
                },
            }],
        };

        let tree = generate_sdk(&api, &SdkConfig::new("sdk/typescript"), Some(&catalog)).unwrap();
        let runtime = tree.get("sdk/typescript/.poolster/client.ts").unwrap();
        let operation = tree
            .get("sdk/typescript/clients/messages/listMessages.ts")
            .unwrap();
        assert!(runtime.contains("\"poolsterApiKey\"?: string"));
        assert!(runtime.contains("applySecurity"));
        assert!(operation.contains("id: 'poolsterApiKey'"));
        assert!(operation.contains("name: 'X-API-Key', in: 'header'"));
    }

    #[test]
    #[ignore = "requires Node and POOLSTER_TSC_JS"]
    fn generated_page_pagination_executes_defaults_and_selectors() {
        let compiler = std::env::var("POOLSTER_TSC_JS").unwrap();
        let directory = tempfile::tempdir().unwrap();
        let iterator = render_pagination_iterator(
            "listItems",
            "listItems",
            &Pagination::OffsetLimit(OffsetPagination {
                step: OffsetStep::Page(PaginationInput {
                    name: "page".into(),
                    location: "query".into(),
                    body_path: None,
                }),
                limit: Some(PaginationInput {
                    name: "limit".into(),
                    location: "query".into(),
                    body_path: None,
                }),
                results_path: Some("/a~1b/~0items/0".into()),
                num_pages_path: None,
            }),
        );
        let source = format!(
            "type ClientInstance = (request: any) => Promise<any>;\nconst seen: number[]=[];\nconst listItems=async (options: any) => {{ seen.push(options.query.page); return {{'a/b':{{'~items':[options.query.page < 3 || options.query.page === Number.MAX_SAFE_INTEGER ? [options.query.page] : []]}}}} }};\n{}\nclass Pages {{ transport: ClientInstance=async request=>request; listItemsPages!: (options: any) => AsyncGenerator<any>; constructor() {{ {} }} }}\n(async()=>{{ const p=new Pages(); let count=0; for await (const page of p.listItemsPages({{}})) count++; if(count!==3 || seen.join(',')!=='1,2,3') throw Error('default page failed'); seen.length=0; for await (const page of p.listItemsPages({{query:{{page:0,limit:2}}}})) {{}}; if(seen.join(',')!=='0') throw Error('zero or short limit failed'); if(poolsterJsonPath([{{items:[1,2]}}], '$[0].items[-1]')!==2) throw Error('selector failed'); if(poolsterJsonPath([10,20], '/01')!==undefined || poolsterJsonPath([10,20], '/-1')!==undefined || poolsterJsonPath([10,20], '/')!==undefined || poolsterJsonPath({{'a/b': 1}}, '/a~2b')!==undefined) throw Error('invalid pointer accepted'); const original={{query:{{page:0,limit:2}}}}; for await (const page of p.listItemsPages(original)) {{}}; if(original.query.page!==0) throw Error('caller mutated'); seen.length=0; for await(const page of p.listItemsPages({{query:{{page:Number.MAX_SAFE_INTEGER}}}})) {{}}; if(seen.length!==1) throw Error('unsafe page advanced'); for(const limit of [true,0,-1,1.5]) {{ let failed=false; try {{ for await (const result of p.listItemsPages({{query:{{limit}}}})) {{}} }} catch {{ failed=true }} if(!failed) throw Error('bad limit accepted'); }} for(const page of [-1,1.5,NaN]) {{ let failed=false; try {{ for await (const result of p.listItemsPages({{query:{{page}}}})) {{}} }} catch {{ failed=true }} if(!failed) throw Error('bad page accepted'); }} }})().catch(error=>{{ console.error(error); throw error }});",
            pagination_helpers(),
            iterator
        );
        std::fs::write(directory.path().join("page.ts"), &source).unwrap();
        let output = std::process::Command::new("node")
            .arg(compiler)
            .args([
                "--strict", "--target", "ES2022", "--module", "commonjs", "page.ts",
            ])
            .current_dir(directory.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            std::process::Command::new("node")
                .arg("page.js")
                .current_dir(directory.path())
                .status()
                .unwrap()
                .success()
        );
    }

    #[test]
    fn offset_limit_pagination_advances_from_declared_results() {
        let iterator = render_pagination_iterator(
            "listPages",
            "listThings",
            &Pagination::OffsetLimit(OffsetPagination {
                step: OffsetStep::Offset(PaginationInput {
                    name: "offset".into(),
                    location: "query".into(),
                    body_path: None,
                }),
                limit: Some(PaginationInput {
                    name: "limit".into(),
                    location: "query".into(),
                    body_path: None,
                }),
                results_path: Some("$.data.results".into()),
                num_pages_path: None,
            }),
        );
        assert!(iterator.contains("poolsterJsonPath(response, \"$.data.results\")"));
        assert!(iterator.contains("currentValue + items.length"));
        assert!(iterator.contains("poolsterWithValue(current, \"query\", \"offset\", nextValue)"));
    }

    #[test]
    fn body_pagination_requires_an_explicit_pointer_for_nested_values() {
        let operation = Operation {
            id: "listThings".into(),
            request_body: Some(OperationRequestBody {
                required: false,
                description: None,
                media_types: vec![OperationMediaType {
                    content_type: "application/json".into(),
                    schema: None,
                }],
            }),
            annotations: BTreeMap::from([(
                "x-poolster-pagination".into(),
                serde_json::json!({
                    "type": "cursor",
                    "inputs": [{
                        "name": "cursor",
                        "in": "requestBody",
                        "type": "cursor",
                        "bodyPath": "/filters/page/cursor"
                    }],
                    "outputs": { "nextCursor": "$.meta.next" }
                }),
            )]),
            ..Operation::default()
        };
        let Some(Pagination::Cursor(cursor_pager)) = pagination(&operation) else {
            panic!("explicit nested body pagination should be recognized")
        };
        assert_eq!(
            cursor_pager.input_body_path.as_deref(),
            Some("/filters/page/cursor")
        );
        let iterator = render_pagination_iterator(
            "listPages",
            "listThings",
            &Pagination::Cursor(cursor_pager),
        );
        assert!(
            iterator.contains("poolsterWithBodyValue(current, \"/filters/page/cursor\", cursor)")
        );
        assert!(iterator.contains("const next = poolsterWithBodyValue"));
        assert!(pagination_helpers().contains("poolsterJsonPointer"));

        let invalid = Operation {
            annotations: BTreeMap::from([(
                "x-poolster-pagination".into(),
                serde_json::json!({
                    "type": "cursor",
                    "inputs": [{
                        "name": "cursor",
                        "in": "requestBody",
                        "type": "cursor",
                        "bodyPath": "/filters/page/not-cursor"
                    }],
                    "outputs": { "nextCursor": "$.meta.next" }
                }),
            )]),
            ..Operation::default()
        };
        assert!(pagination(&invalid).is_none());
    }

    #[test]
    fn body_offset_pagination_uses_the_declared_pointer_without_mutation() {
        let pagination = OffsetPagination {
            step: OffsetStep::Offset(PaginationInput {
                name: "offset".into(),
                location: "body".into(),
                body_path: Some("/page/offset".into()),
            }),
            limit: Some(PaginationInput {
                name: "limit".into(),
                location: "body".into(),
                body_path: Some("/page/limit".into()),
            }),
            results_path: Some("$.items".into()),
            num_pages_path: None,
        };
        let rendered = render_offset_pagination(&pagination);
        assert!(rendered.contains("poolsterBodyValue(current, \"/page/offset\")"));
        assert!(rendered.contains("poolsterWithBodyValue(current, \"/page/offset\", nextValue)"));
    }

    #[test]
    fn url_pagination_reenters_the_generated_operation_with_a_safe_continuation() {
        let iterator = render_pagination_iterator(
            "listPages",
            "listThings",
            &Pagination::Url(UrlPagination {
                next_url_path: "$.links.next".into(),
            }),
        );
        assert!(iterator.contains("const request: ClientInstance"));
        assert!(iterator.contains("paginationClient({ ...operation, paginationUrl: nextUrl })"));
        assert!(iterator.contains("await listThings({ ...requestOptions, client: request }"));
        assert!(iterator.contains("query: undefined"));
        assert!(iterator.contains("poolsterPaginationUrl(response, \"$.links.next\")"));
        assert!(!iterator.contains("url: nextUrl"));

        let fetch_runtime = poolster_runtime(SdkTransport::Fetch, None);
        assert!(fetch_runtime.contains("paginationUrl?: string"));
        assert!(fetch_runtime.contains("Pagination URL must use the configured API origin"));
        assert!(
            fetch_runtime
                .contains("applySecurity(mergedHeaders, resolvedQuery, security, config.auth)")
        );

        let axios_runtime = poolster_runtime(SdkTransport::Axios, None);
        assert!(axios_runtime.contains("Pagination URL must use the configured API origin"));
        assert!(
            axios_runtime
                .contains("applySecurity(resolvedHeaders, resolvedQuery, security, config.auth)")
        );
    }

    #[test]
    fn url_pagination_uses_the_declared_next_url_output() {
        let operation = Operation {
            id: "listThings".into(),
            annotations: BTreeMap::from([(
                "x-speakeasy-pagination".into(),
                serde_json::json!({
                    "type": "url",
                    "outputs": { "nextUrl": "$.links.next" },
                }),
            )]),
            ..Operation::default()
        };
        let Some(Pagination::Url(pagination)) = pagination(&operation) else {
            panic!("URL pagination should be recognized")
        };
        assert_eq!(pagination.next_url_path, "$.links.next");
    }

    #[test]
    fn package_versions_accept_openapi_date_versions() {
        assert_eq!(package_version("2026-09-19"), "2026.9.19");
        assert_eq!(package_version("1.2.3"), "1.2.3");
        assert_eq!(package_version("latest"), "0.1.0");
    }

    #[test]
    fn fetch_runtime_has_openapi_serializers_codecs_and_status_results() {
        let runtime = poolster_runtime(SdkTransport::Fetch, None);
        assert!(runtime.contains("export interface Codec"));
        assert!(runtime.contains("export type StatusResult<T>"));
        assert!(runtime.contains("export type ResponseResult<T"));
        assert!(runtime.contains("export const resolveResponse"));
        assert!(!runtime.contains("withUnwrap"));
        assert!(runtime.contains("serializePath"));
        assert!(runtime.contains("deepObject"));
        assert!(runtime.contains("multipart/form-data"));
        assert!(runtime.contains("application/x-www-form-urlencoded"));
        assert!(runtime.contains("export type FormEncoding"));
        assert!(runtime.contains("export type FormPartHeader"));
        assert!(runtime.contains("export interface MultipartEncoder"));
        assert!(runtime.contains("Native FormData cannot set per-part headers"));
        assert!(runtime.contains("Missing required multipart header"));
        assert!(runtime.contains("contentType: response.headers.get('content-type')"));
        assert!(runtime.contains("scheme.in === 'cookie'"));
        assert!(runtime.contains("export interface StandardSchema"));
        assert!(runtime.contains("await config.hooks?.afterResponse?.({ request, status: response.status, response: response.clone() })\n  if (!response.ok"));
        assert!(
            runtime.contains("validate(validation?.response ?? config.validation?.response, data)")
        );
    }

    #[test]
    fn axios_runtime_has_openapi_serializers_codecs_and_status_results() {
        let runtime = poolster_runtime(SdkTransport::Axios, None);
        assert!(runtime.contains("export type FormEncoding"));
        assert!(runtime.contains("export type FormPartHeader"));
        assert!(runtime.contains("export interface MultipartEncoder"));
        assert!(runtime.contains("Native FormData cannot set per-part headers"));
        assert!(runtime.contains("contentType: mediaType.split(';')[0].trim()"));
        assert!(runtime.contains("export interface Codec"));
        assert!(runtime.contains("export type StatusResult<T>"));
        assert!(runtime.contains("export type ResponseResult<T"));
        assert!(runtime.contains("export const resolveResponse"));
        assert!(!runtime.contains("withUnwrap"));
        assert!(runtime.contains("serializeQuery"));
        assert!(runtime.contains("encodeBody"));
        assert!(runtime.contains("multipart/form-data"));
        assert!(runtime.contains("config.codecs"));
        assert!(runtime.contains("export interface StandardSchema"));
        assert!(runtime.contains("await config.hooks?.afterResponse?.({ request, status: response.status, headers: response.headers as Record<string, unknown>, data })\n        if (response.status >= 400"));
        assert!(runtime.contains(
            "if (signal?.aborted || axios.isCancel(error) || !axios.isAxiosError(error) || attempt + 1 >= maxAttempts)"
        ));
        assert!(
            runtime.contains("validate(validation?.request ?? config.validation?.request, body)")
        );
    }

    #[test]
    fn sdk_client_names_strip_the_openapi_api_suffix() {
        assert_eq!(sdk_client_name("Poolster Email API"), "PoolsterEmail");
        assert_eq!(sdk_client_name("Mistral API"), "Mistral");
    }
}
