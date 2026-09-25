//! Structured TypeScript Fetch and Axios operation-client generators.
//!
//! Fetch and Axios operation files share a consistent public source shape;
//! transport-specific setup lives in `.kaji/client`.

use anyhow::{Result, bail};
use serde_json::Value;

use crate::ast::{Api, Operation, SecuritySchemeCatalog, SecuritySchemeKind};
use crate::{CodegenPlugin, GeneratedFile, GeneratorConfig};

const ESLINT_HEADER: &str = "/* eslint-disable no-alert, no-console */\n\n";

/// Emits standalone Fetch operation functions.
///
/// The supported configuration keys mirror the Kaji defaults relevant to an
/// operation file:
///
/// - `output_dir`: output directory, defaulting to `clients`.
/// - `throw_on_error_default`: `true` (the default) or `false`.
#[derive(Default)]
pub struct StructuredTypeScriptFetch;

impl CodegenPlugin for StructuredTypeScriptFetch {
    fn name(&self) -> &'static str {
        "structured-typescript-fetch"
    }

    fn generate(&self, api: &Api, config: &GeneratorConfig) -> Result<Vec<GeneratedFile>> {
        self.generate_with_optional_security_catalog(api, config, None)
    }
}

impl StructuredTypeScriptFetch {
    /// Generates Fetch operations using typed component security metadata.
    ///
    /// SDK packages can opt into this overload once their OpenAPI adapter has
    /// loaded `security-schemes.json`.
    pub fn generate_with_security_catalog(
        &self,
        api: &Api,
        config: &GeneratorConfig,
        security_schemes: &SecuritySchemeCatalog,
    ) -> Result<Vec<GeneratedFile>> {
        // SDK package generation uses the named variant below so its internal
        // runtime can bind credentials by OpenAPI component name.
        self.generate_with_optional_security_catalog(api, config, Some(security_schemes))
    }

    pub fn generate_with_named_security_catalog(
        &self,
        api: &Api,
        config: &GeneratorConfig,
        security_schemes: &SecuritySchemeCatalog,
    ) -> Result<Vec<GeneratedFile>> {
        let mut config = config.clone();
        config.insert("kaji_named_security".into(), "true".into());
        self.generate_with_optional_security_catalog(api, &config, Some(security_schemes))
    }

    fn generate_with_optional_security_catalog(
        &self,
        api: &Api,
        config: &GeneratorConfig,
        security_schemes: Option<&SecuritySchemeCatalog>,
    ) -> Result<Vec<GeneratedFile>> {
        if let Some(class_name) = sdk_class_name(config) {
            return generate_sdk_class(api, config, class_name, security_schemes);
        }
        generate_operations(api, config, security_schemes)
    }
}

/// Emits standalone Axios operation functions.
///
/// Kaji's generated operation modules are transport agnostic: the Fetch and
/// Axios plugins differ in their injected `.kaji/client` runtime, while their
/// public operation modules are byte-identical for the same OpenAPI operation
/// and configuration.
#[derive(Default)]
pub struct StructuredTypeScriptAxios;

impl CodegenPlugin for StructuredTypeScriptAxios {
    fn name(&self) -> &'static str {
        "structured-typescript-axios"
    }

    fn generate(&self, api: &Api, config: &GeneratorConfig) -> Result<Vec<GeneratedFile>> {
        self.generate_with_optional_security_catalog(api, config, None)
    }
}

impl StructuredTypeScriptAxios {
    /// Generates Axios operations using typed component security metadata from
    /// the Go-sidecar catalog. This is deliberately an opt-in overload while
    /// existing operation-only callers migrate to loading the new artifact.
    pub fn generate_with_security_catalog(
        &self,
        api: &Api,
        config: &GeneratorConfig,
        security_schemes: &SecuritySchemeCatalog,
    ) -> Result<Vec<GeneratedFile>> {
        // SDK packages opt into named descriptors through the dedicated
        // method below.
        self.generate_with_optional_security_catalog(api, config, Some(security_schemes))
    }

    pub fn generate_with_named_security_catalog(
        &self,
        api: &Api,
        config: &GeneratorConfig,
        security_schemes: &SecuritySchemeCatalog,
    ) -> Result<Vec<GeneratedFile>> {
        let mut config = config.clone();
        config.insert("kaji_named_security".into(), "true".into());
        self.generate_with_optional_security_catalog(api, &config, Some(security_schemes))
    }

    fn generate_with_optional_security_catalog(
        &self,
        api: &Api,
        config: &GeneratorConfig,
        security_schemes: Option<&SecuritySchemeCatalog>,
    ) -> Result<Vec<GeneratedFile>> {
        if let Some(class_name) = sdk_class_name(config) {
            return generate_sdk_class(api, config, class_name, security_schemes);
        }
        generate_operations(api, config, security_schemes)
    }
}

fn generate_operations(
    api: &Api,
    config: &GeneratorConfig,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> Result<Vec<GeneratedFile>> {
    let output_dir = config
        .get("output_dir")
        .map(String::as_str)
        .unwrap_or("clients")
        .trim_matches('/');
    let throw_on_error = parse_throw_on_error(config)?;
    let return_data = matches!(
        config
            .get("return_type")
            .or_else(|| config.get("returnType"))
            .map(String::as_str),
        Some("data")
    );
    let validator_response = matches!(
        config.get("validator_response").map(String::as_str),
        Some("true")
    );
    let global_security = config
        .get("global_security")
        .and_then(|value| configured_security(value));
    let named_security = matches!(
        config.get("kaji_named_security").map(String::as_str),
        Some("true")
    );

    let group_by_tag = matches!(
        config
            .get("group_type")
            .or_else(|| config.get("group.type"))
            .map(String::as_str),
        Some("tag")
    );

    api.operations
        .iter()
        .map(|operation| {
            let name = lower_camel_identifier(&operation.id);
            let group = grouped_directory(operation, config, group_by_tag);
            let path = if output_dir.is_empty() {
                match group {
                    Some(group) => format!("{group}/{name}.ts"),
                    None => format!("{name}.ts"),
                }
            } else {
                match group {
                    Some(group) => format!("{output_dir}/{group}/{name}.ts"),
                    None => format!("{output_dir}/{name}.ts"),
                }
            };
            GeneratedFile::new(
                path,
                if return_data {
                    render_data_operation(operation, throw_on_error)
                } else {
                    let source = if operation.security.is_empty() {
                        global_security
                            .as_deref()
                            .map(|security| {
                                if security == "[{ type: 'oauth2' }]" {
                                    render_inline_security_operation(
                                        operation,
                                        throw_on_error,
                                        security,
                                    )
                                } else {
                                    render_security_operation(operation, throw_on_error, security)
                                }
                            })
                            .unwrap_or_else(|| {
                                render_operation(
                                    operation,
                                    throw_on_error,
                                    security_schemes,
                                    named_security,
                                )
                            })
                    } else {
                        render_operation(
                            operation,
                            throw_on_error,
                            security_schemes,
                            named_security,
                        )
                    };
                    let source = rewrite_import_paths(source, operation, config);
                    if validator_response {
                        render_validator_response(source, operation, throw_on_error)
                    } else {
                        source
                    }
                },
            )
        })
        .collect()
}

fn rewrite_import_paths(source: String, operation: &Operation, config: &GeneratorConfig) -> String {
    // Direct structured mode emits co-located files, including when
    // tag grouping is enabled. Only SDK package mode supplies explicit import
    // prefixes because it deliberately separates models, clients, and the
    // runtime into different directories.
    if !config.contains_key("type_import_prefix") && !config.contains_key("runtime_import_prefix") {
        return source;
    }
    let type_prefix = config
        .get("type_import_prefix")
        .map(String::as_str)
        .unwrap_or(".");
    let runtime_prefix = config
        .get("runtime_import_prefix")
        .map(String::as_str)
        .unwrap_or(".");
    let runtime_dir = config
        .get("runtime_dir")
        .map(String::as_str)
        .unwrap_or(".kaji");
    let group_by_tag = matches!(
        config
            .get("group_type")
            .or_else(|| config.get("group.type"))
            .map(String::as_str),
        Some("tag")
    );
    let group = grouped_directory(operation, config, group_by_tag);
    let type_path = group
        .as_deref()
        .map(|group| format!("{type_prefix}/{group}/{}", pascal_identifier(&operation.id)))
        .unwrap_or_else(|| format!("{type_prefix}/{}", pascal_identifier(&operation.id)));
    let runtime_path = if group.is_some() {
        format!("../../{runtime_dir}/client")
    } else {
        format!("{runtime_prefix}/{runtime_dir}/client")
    };
    source
        .replace(
            &format!("from './{}'", pascal_identifier(&operation.id)),
            &format!("from '{type_path}'"),
        )
        .replace("from './.kaji/client'", &format!("from '{runtime_path}'"))
}

fn grouped_directory(
    operation: &Operation,
    config: &GeneratorConfig,
    group_by_tag: bool,
) -> Option<String> {
    if !group_by_tag {
        return None;
    }
    operation_tag_group(operation).or_else(|| {
        config
            .get("group_default_directory")
            .is_some_and(|value| value == "true")
            .then(|| "default".into())
    })
}

fn render_inline_security_operation(
    operation: &Operation,
    throw_on_error: bool,
    security: &str,
) -> String {
    let function_name = lower_camel_identifier(&operation.id);
    let type_name = pascal_identifier(&operation.id);
    let throw_on_error = if throw_on_error { "true" } else { "false" };
    let link_path = operation.path.replace('{', ":").replace('}', "");
    let method = operation.method.as_str();
    format!(
        "{ESLINT_HEADER}import type {{ Options, Unwrappable, RequestResult }} from './.kaji/client'\nimport type {{ {type_name}Options, {type_name}Responses }} from './{type_name}'\nimport {{ client, withUnwrap }} from './.kaji/client'\n\n/**\n * {{@link {link_path}}}\n */\nexport function {function_name}<ThrowOnError extends boolean = {throw_on_error}>(\n  options: Options<{type_name}Options, ThrowOnError>,\n): Unwrappable<RequestResult<{type_name}Responses, ThrowOnError>> {{\n  const {{ client: request = client, ...config }} = options\n\n  return withUnwrap(\n    request({{ method: '{method}', url: '{}', security: {security}, ...config, throwOnError: config.throwOnError ?? {throw_on_error} }}) as Promise<\n      RequestResult<{type_name}Responses, ThrowOnError>\n    >,\n  )\n}}\n",
        operation.path,
    )
}

fn configured_security(value: &str) -> Option<String> {
    match value {
        "bearer" => Some("[{ type: 'http', scheme: 'bearer' }]".into()),
        "oauth2" => Some("[{ type: 'oauth2' }]".into()),
        _ => None,
    }
}

fn render_validator_response(
    source: String,
    operation: &Operation,
    throw_on_error: bool,
) -> String {
    let type_name = pascal_identifier(&operation.id);
    let throw_on_error = if throw_on_error { "true" } else { "false" };
    let source = source.replace(
        "import { client, withUnwrap } from './.kaji/client'\n\n",
        &format!("import {{ client, withUnwrap }} from './.kaji/client'\nimport {{ {type_name}Response }} from './{type_name}'\n\n"),
    );
    let source = source.replace(
        &format!("url: '{}', ...config", operation.path),
        &format!(
            "url: '{}', validator: {{ response: {type_name}Response }}, ...config",
            operation.path
        ),
    );
    let compact = format!(
        "request({{ method: '{}', url: '{}', validator: {{ response: {type_name}Response }}, ...config, throwOnError: config.throwOnError ?? {throw_on_error} }}) as Promise<RequestResult<{type_name}Responses, ThrowOnError>> ,",
        operation.method.as_str(), operation.path,
    )
    .replace(">> ,", ">>,");
    let expanded = format!(
        "request({{ method: '{}', url: '{}', validator: {{ response: {type_name}Response }}, ...config, throwOnError: config.throwOnError ?? {throw_on_error} }}) as Promise<\n      RequestResult<{type_name}Responses, ThrowOnError>\n    >,",
        operation.method.as_str(),
        operation.path,
    );
    source.replace(&compact, &expanded)
}

/// Kaji's tag grouping uses the first operation tag and camel-cases it for a
/// portable directory name. Tags flow through the neutral AST as an adapter
/// annotation until they become a first-class field on every source adapter.
fn operation_tag_group(operation: &Operation) -> Option<String> {
    operation
        .annotations
        .get("tags")
        .and_then(Value::as_array)
        .and_then(|tags| tags.first())
        .and_then(Value::as_str)
        .map(lower_camel_identifier)
        .filter(|tag| !tag.is_empty())
}

fn parse_throw_on_error(config: &GeneratorConfig) -> Result<bool> {
    match config.get("throw_on_error_default").map(String::as_str) {
        None | Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(value) => bail!(
            "TypeScript client config `throw_on_error_default` must be `true` or `false`, got `{value}`"
        ),
    }
}

/// Kaji's SDK mode is configured as `sdk: { name: 'PetClient' }`. The
/// declarative Rust config flattens nested fields, while accepting ergonomic
/// snake/camel aliases for direct Rust callers.
fn sdk_class_name(config: &GeneratorConfig) -> Option<&str> {
    config
        .get("sdk.name")
        .or_else(|| config.get("sdk_name"))
        .or_else(|| config.get("sdkName"))
        .map(String::as_str)
        .filter(|name| !name.is_empty())
}

fn generate_sdk_class(
    api: &Api,
    config: &GeneratorConfig,
    class_name: &str,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> Result<Vec<GeneratedFile>> {
    let output_dir = config
        .get("output_dir")
        .map(String::as_str)
        .unwrap_or("clients")
        .trim_matches('/');
    let throw_on_error = parse_throw_on_error(config)?;
    let return_data = matches!(
        config
            .get("return_type")
            .or_else(|| config.get("returnType"))
            .map(String::as_str),
        Some("data")
    );
    let file_name = format!("{}.ts", lower_camel_identifier(class_name));
    let path = if output_dir.is_empty() {
        file_name
    } else {
        format!("{output_dir}/{file_name}")
    };
    Ok(vec![GeneratedFile::new(
        path,
        if let Some(clients) = sdk_facade_clients(config) {
            render_sdk_facade(class_name, &clients)
        } else if return_data {
            render_data_sdk_class(api, class_name, throw_on_error)
        } else {
            render_sdk_class(api, class_name, throw_on_error, security_schemes)
        },
    )?])
}

fn sdk_facade_clients(config: &GeneratorConfig) -> Option<Vec<String>> {
    let clients = config.get("sdk.facade_clients")?;
    let clients = clients
        .split(',')
        .map(str::trim)
        .filter(|name| name.ends_with("Client"))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    (!clients.is_empty()).then_some(clients)
}

fn render_sdk_facade(class_name: &str, clients: &[String]) -> String {
    let mut imports = clients.to_vec();
    imports.sort();
    let imports = imports
        .iter()
        .map(|client| {
            format!(
                "import {{ {client} }} from './{}'",
                lower_camel_identifier(client)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let fields = clients
        .iter()
        .map(|client| {
            let property = lower_camel_identifier(client.trim_end_matches("Client"));
            format!("  readonly {property}: {client}")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let initializers = clients
        .iter()
        .map(|client| {
            let property = lower_camel_identifier(client.trim_end_matches("Client"));
            format!("    this.{property} = new {client}(config)")
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{ESLINT_HEADER}import type {{ ClientConfig }} from './.kaji/client'\n{imports}\n\nexport class {class_name} {{\n{fields}\n\n  constructor(config: ClientConfig = {{}}) {{\n{initializers}\n  }}\n}}\n"
    )
}

fn render_data_operation(operation: &Operation, throw_on_error: bool) -> String {
    let function_name = lower_camel_identifier(&operation.id);
    let type_name = pascal_identifier(&operation.id);
    let link_path = operation.path.replace('{', ":").replace('}', "");
    let method = operation.method.as_str();
    let throw_on_error = if throw_on_error { "true" } else { "false" };
    format!(
        "{ESLINT_HEADER}import type {{ Options, UnwrappedResult }} from './.kaji/client'\nimport type {{ {type_name}Options, {type_name}Responses }} from './{type_name}'\nimport {{ client, unwrapResult }} from './.kaji/client'\n\n/**\n * {{@link {link_path}}}\n */\nexport function {function_name}<ThrowOnError extends boolean = {throw_on_error}>(\n  options: Options<{type_name}Options, ThrowOnError>,\n): Promise<UnwrappedResult<{type_name}Responses, ThrowOnError>> {{\n  const {{ client: request = client, ...config }} = options\n\n  return unwrapResult(\n    request({{ method: '{method}', url: '{}', ...config, throwOnError: config.throwOnError ?? {throw_on_error} }}),\n    config.throwOnError ?? {throw_on_error},\n  ) as Promise<UnwrappedResult<{type_name}Responses, ThrowOnError>>\n}}\n",
        operation.path,
    )
}

fn render_data_sdk_class(api: &Api, class_name: &str, throw_on_error: bool) -> String {
    let mut type_operations = api.operations.iter().collect::<Vec<_>>();
    type_operations.sort_by_key(|operation| pascal_identifier(&operation.id));
    let imports = type_operations
        .iter()
        .map(|operation| {
            let type_name = pascal_identifier(&operation.id);
            format!(
                "import type {{ {type_name}Options, {type_name}Responses }} from './{type_name}'"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let throw_on_error = if throw_on_error { "true" } else { "false" };
    let mut output = format!(
        "{ESLINT_HEADER}import type {{ ClientConfig, ClientInstance, Options, UnwrappedResult }} from './.kaji/client'\n{imports}\nimport {{ createClient, unwrapResult }} from './.kaji/client'\n\nexport class {class_name} {{\n  private readonly client: ClientInstance\n\n  constructor(config: ClientConfig = {{}}) {{\n    this.client = createClient(config)\n  }}\n"
    );
    for operation in &api.operations {
        let function_name = lower_camel_identifier(&operation.id);
        let type_name = pascal_identifier(&operation.id);
        let link_path = operation.path.replace('{', ":").replace('}', "");
        let method = operation.method.as_str();
        let optional_options = if options_are_required(operation) {
            ""
        } else {
            " = {}"
        };
        output.push_str(&format!(
            "\n  /**\n   * {{@link {link_path}}}\n   */\n  public {function_name}<ThrowOnError extends boolean = {throw_on_error}>(\n    options: Options<{type_name}Options, ThrowOnError>{optional_options},\n  ): Promise<UnwrappedResult<{type_name}Responses, ThrowOnError>> {{\n    const {{ client: request = this.client, ...config }} = options\n\n    return unwrapResult(\n      request({{ method: '{method}', url: '{}', ...config, throwOnError: config.throwOnError ?? {throw_on_error} }}),\n      config.throwOnError ?? {throw_on_error},\n    ) as Promise<UnwrappedResult<{type_name}Responses, ThrowOnError>>\n  }}\n",
            operation.path,
        ));
    }
    output.push_str("}\n");
    output
}

fn render_sdk_class(
    api: &Api,
    class_name: &str,
    throw_on_error: bool,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> String {
    let named_security = false;
    let mut type_operations = api.operations.iter().collect::<Vec<_>>();
    type_operations.sort_by_key(|operation| pascal_identifier(&operation.id));
    let imports = type_operations
        .iter()
        .map(|operation| {
            let type_name = pascal_identifier(&operation.id);
            format!(
                "import type {{ {type_name}Options, {type_name}Responses }} from './{type_name}'"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let throw_on_error = if throw_on_error { "true" } else { "false" };
    let mut output = format!(
        "{ESLINT_HEADER}import type {{ ClientConfig, ClientInstance, Options, Unwrappable, RequestResult }} from './.kaji/client'\n{imports}\nimport {{ createClient, withUnwrap }} from './.kaji/client'\n\nexport class {class_name} {{\n  private readonly client: ClientInstance\n\n  constructor(config: ClientConfig = {{}}) {{\n    this.client = createClient(config)\n  }}\n"
    );
    for operation in &api.operations {
        output.push('\n');
        output.push_str(&render_sdk_method(
            operation,
            throw_on_error,
            security_schemes,
            named_security,
        ));
    }
    output.push_str("}\n");
    output
}

fn render_sdk_method(
    operation: &Operation,
    throw_on_error: &str,
    security_schemes: Option<&SecuritySchemeCatalog>,
    named_security: bool,
) -> String {
    let function_name = lower_camel_identifier(&operation.id);
    let type_name = pascal_identifier(&operation.id);
    let link_path = operation.path.replace('{', ":").replace('}', "");
    let method = operation.method.as_str();
    let optional_options = if options_are_required(operation) {
        ""
    } else {
        " = {}"
    };
    let security = render_security(operation, security_schemes, named_security);
    let request = match &security {
        Some(security) => format!(
            "request({{\n        method: '{method}',\n        url: '{}',\n        security: {security},\n        ...config,\n        throwOnError: config.throwOnError ?? {throw_on_error},\n      }})",
            operation.path,
        ),
        None => format!(
            "request({{ method: '{method}', url: '{}', ...config, throwOnError: config.throwOnError ?? {throw_on_error} }})",
            operation.path,
        ),
    };
    let source = format!(
        "  /**\n   * {{@link {link_path}}}\n   */\n  public {function_name}<ThrowOnError extends boolean = {throw_on_error}>(\n    options: Options<{type_name}Options, ThrowOnError>{optional_options},\n  ): Unwrappable<RequestResult<{type_name}Responses, ThrowOnError>> {{\n    const {{ client: request = this.client, ...config }} = options\n\n    return withUnwrap(\n      {request} as Promise<\n        RequestResult<{type_name}Responses, ThrowOnError>\n      >,\n    )\n  }}\n",
    );
    if security.is_some() {
        source.replace(
            &format!(
                "}}) as Promise<\n        RequestResult<{type_name}Responses, ThrowOnError>\n      >,"
            ),
            &format!("}}) as Promise<RequestResult<{type_name}Responses, ThrowOnError>>,"),
        )
    } else {
        source
    }
}

fn options_are_required(operation: &Operation) -> bool {
    operation.request_body.as_ref().is_some_and(|body| body.required)
        || operation.parameters.iter().any(|parameter| parameter.required)
    // The pre-expanded AST only carried a lossy request type. Preserve the
    // historical conservative behavior for such operations.
        || operation.request_type.is_some()
}

fn render_operation(
    operation: &Operation,
    throw_on_error: bool,
    security_schemes: Option<&SecuritySchemeCatalog>,
    named_security: bool,
) -> String {
    if is_event_stream(operation) {
        return render_event_stream_operation(operation, throw_on_error);
    }
    if let Some(content_type) = request_content_type(operation) {
        return render_content_type_operation(operation, throw_on_error, content_type);
    }
    if let Some(security) = render_security(operation, security_schemes, named_security) {
        return render_security_operation(operation, throw_on_error, &security);
    }
    if let Some(styles) = render_parameter_styles(operation) {
        return render_styled_operation(operation, throw_on_error, &styles);
    }
    let function_name = lower_camel_identifier(&operation.id);
    let type_name = pascal_identifier(&operation.id);
    let throw_on_error = if throw_on_error { "true" } else { "false" };
    let link_path = operation.path.replace('{', ":").replace('}', "");
    let method = operation.method.as_str();

    // Kaji's printer keeps a complete request/result expression on one line
    // only when it fits its print width.  Make that decision before rendering
    // so byte parity does not depend on a host formatter.
    let source = format!(
        "{ESLINT_HEADER}import type {{ Options, Unwrappable, RequestResult }} from './.kaji/client'\nimport type {{ {type_name}Options, {type_name}Responses }} from './{type_name}'\nimport {{ client, withUnwrap }} from './.kaji/client'\n\n/**\n * {{@link {link_path}}}\n */\nexport function {function_name}<ThrowOnError extends boolean = {throw_on_error}>(\n  options: Options<{type_name}Options, ThrowOnError>,\n): Unwrappable<RequestResult<{type_name}Responses, ThrowOnError>> {{\n  const {{ client: request = client, ...config }} = options\n\n  return withUnwrap(\n    request({{ method: '{method}', url: '{}', ...config, throwOnError: config.throwOnError ?? {throw_on_error} }}) as Promise<\n      RequestResult<{type_name}Responses, ThrowOnError>\n    >,\n  )\n}}\n",
        operation.path,
    );
    let expanded = format!(
        "request({{ method: '{method}', url: '{}', ...config, throwOnError: config.throwOnError ?? {throw_on_error} }}) as Promise<\n      RequestResult<{type_name}Responses, ThrowOnError>\n    >,",
        operation.path,
    );
    let compact = format!(
        "request({{ method: '{method}', url: '{}', ...config, throwOnError: config.throwOnError ?? {throw_on_error} }}) as Promise<RequestResult<{type_name}Responses, ThrowOnError>> ,",
        operation.path,
    )
    .replace(">> ,", ">>,");
    if compact.len() + 4 <= 160 {
        source.replace(&expanded, &compact)
    } else {
        source
    }
}

/// Preserves explicit OpenAPI parameter serialization metadata for Kaji's
/// runtime. Unspecified style/explode settings intentionally remain absent so
/// the runtime can apply each location's OpenAPI defaults.
fn render_parameter_styles(operation: &Operation) -> Option<String> {
    let groups = ["path", "query", "header", "cookie"]
        .into_iter()
        .filter_map(|location| {
            let parameters = operation
                .parameters
                .iter()
                .filter(|parameter| parameter.location == location)
                .filter_map(|parameter| {
                    let style = parameter.annotations.get("style").and_then(Value::as_str);
                    let explode = parameter
                        .annotations
                        .get("explode")
                        .and_then(Value::as_bool);
                    (style.is_some() || explode.is_some()).then(|| {
                        let mut fields = Vec::new();
                        if let Some(style) = style {
                            fields.push(format!("style: '{style}'"));
                        }
                        if let Some(explode) = explode {
                            fields.push(format!("explode: {explode}"));
                        }
                        format!("{}: {{ {} }}", parameter.name, fields.join(", "))
                    })
                })
                .collect::<Vec<_>>();
            (!parameters.is_empty()).then(|| format!("{location}: {{ {} }}", parameters.join(", ")))
        })
        .collect::<Vec<_>>();
    (!groups.is_empty()).then(|| format!("{{ {} }}", groups.join(", ")))
}

fn render_styled_operation(operation: &Operation, throw_on_error: bool, styles: &str) -> String {
    let function_name = lower_camel_identifier(&operation.id);
    let type_name = pascal_identifier(&operation.id);
    let throw_on_error = if throw_on_error { "true" } else { "false" };
    let link_path = operation.path.replace('{', ":").replace('}', "");
    let method = operation.method.as_str();
    format!(
        "{ESLINT_HEADER}import type {{ Options, Unwrappable, RequestResult }} from './.kaji/client'\nimport type {{ {type_name}Options, {type_name}Responses }} from './{type_name}'\nimport {{ client, withUnwrap }} from './.kaji/client'\n\n/**\n * {{@link {link_path}}}\n */\nexport function {function_name}<ThrowOnError extends boolean = {throw_on_error}>(\n  options: Options<{type_name}Options, ThrowOnError>,\n): Unwrappable<RequestResult<{type_name}Responses, ThrowOnError>> {{\n  const {{ client: request = client, ...config }} = options\n\n  return withUnwrap(\n    request({{\n      method: '{method}',\n      url: '{}',\n      styles: {styles},\n      ...config,\n      throwOnError: config.throwOnError ?? {throw_on_error},\n    }}) as Promise<RequestResult<{type_name}Responses, ThrowOnError>>,\n  )\n}}\n",
        operation.path,
    )
}

fn render_security_operation(
    operation: &Operation,
    throw_on_error: bool,
    security: &str,
) -> String {
    let function_name = lower_camel_identifier(&operation.id);
    let type_name = pascal_identifier(&operation.id);
    let throw_on_error = if throw_on_error { "true" } else { "false" };
    let link_path = operation.path.replace('{', ":").replace('}', "");
    let method = operation.method.as_str();
    format!(
        "{ESLINT_HEADER}import type {{ Options, Unwrappable, RequestResult }} from './.kaji/client'\nimport type {{ {type_name}Options, {type_name}Responses }} from './{type_name}'\nimport {{ client, withUnwrap }} from './.kaji/client'\n\n/**\n * {{@link {link_path}}}\n */\nexport function {function_name}<ThrowOnError extends boolean = {throw_on_error}>(\n  options: Options<{type_name}Options, ThrowOnError>,\n): Unwrappable<RequestResult<{type_name}Responses, ThrowOnError>> {{\n  const {{ client: request = client, ...config }} = options\n\n  return withUnwrap(\n    request({{\n      method: '{method}',\n      url: '{}',\n      security: {security},\n      ...config,\n      throwOnError: config.throwOnError ?? {throw_on_error},\n    }}) as Promise<RequestResult<{type_name}Responses, ThrowOnError>>,\n  )\n}}\n",
        operation.path,
    )
}

/// Converts OpenAPI's OR-of-AND requirements into Kaji client's compact
/// security descriptors. The neutral AST keeps scheme names/scopes; common
/// OpenAPI names map predictably while adapters may later attach richer scheme
/// definitions without changing client rendering.
fn render_security(
    operation: &Operation,
    security_schemes: Option<&SecuritySchemeCatalog>,
    named_security: bool,
) -> Option<String> {
    // Keep the old compact output when no component catalog is available.
    // SDK generation supplies the catalog and therefore preserves OpenAPI's
    // OR-of-AND security structure below.
    if named_security && security_schemes.is_some() {
        return (!operation.security.is_empty()).then(|| {
            let alternatives = operation
                .security
                .iter()
                .map(|requirement| {
                    let schemes = requirement
                        .schemes
                        .iter()
                        .map(|(name, scopes)| {
                            render_catalog_security_scheme(name, scopes, security_schemes)
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("[{schemes}]")
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{alternatives}]")
        });
    }
    (!operation.security.is_empty()).then(|| {
        let alternatives = operation
            .security
            .iter()
            .flat_map(|requirement| requirement.schemes.keys())
            .map(|scheme| render_security_scheme(scheme, security_schemes))
            .collect::<Vec<_>>()
            .join(", ");
        format!("[{alternatives}]")
    })
}

/// Emits runtime-addressable descriptors for SDK packages. `id` is the
/// OpenAPI component key, so one client can carry several independently named
/// API keys, bearer tokens, and OAuth credentials without guessing headers.
fn render_catalog_security_scheme(
    scheme_name: &str,
    scopes: &[String],
    catalog: Option<&SecuritySchemeCatalog>,
) -> String {
    let descriptor = render_security_scheme(scheme_name, catalog);
    let scopes = if scopes.is_empty() {
        String::new()
    } else {
        format!(
            ", scopes: {}",
            serde_json::to_string(scopes).unwrap_or_else(|_| "[]".into())
        )
    };
    descriptor
        .replacen("{ ", &format!("{{ id: '{scheme_name}', "), 1)
        .replacen(" }", &format!("{scopes} }}"), 1)
}

fn render_security_scheme(scheme_name: &str, catalog: Option<&SecuritySchemeCatalog>) -> String {
    let Some(scheme) = catalog.and_then(|catalog| {
        catalog
            .schemes
            .iter()
            .find(|scheme| scheme.name == scheme_name)
    }) else {
        return render_legacy_security_scheme(scheme_name);
    };

    match &scheme.kind {
        SecuritySchemeKind::ApiKey { name, location } => {
            let name = name.as_deref().unwrap_or("Authorization");
            let location = location.as_deref().unwrap_or("header");
            format!("{{ type: 'apiKey', name: '{name}', in: '{location}' }}")
        }
        SecuritySchemeKind::Http { scheme, .. } => {
            let scheme = scheme.as_deref().unwrap_or("bearer");
            format!("{{ type: 'http', scheme: '{scheme}' }}")
        }
        SecuritySchemeKind::OAuth2 { .. } | SecuritySchemeKind::OpenIdConnect { .. } => {
            "{ type: 'oauth2' }".to_owned()
        }
        SecuritySchemeKind::Other { .. } => render_legacy_security_scheme(scheme_name),
    }
}

fn render_legacy_security_scheme(scheme_name: &str) -> String {
    let lower = scheme_name.to_ascii_lowercase();
    if lower.contains("oauth") {
        "{ type: 'oauth2' }".to_owned()
    } else if lower.contains("key") {
        format!("{{ type: 'apiKey', name: '{scheme_name}', in: 'header' }}")
    } else {
        format!("{{ type: 'http', scheme: '{scheme_name}' }}")
    }
}

/// Kaji routes Server-Sent Events through the event-stream client helper,
/// selected from the response media type rather than an operation-name rule.
fn render_event_stream_operation(operation: &Operation, throw_on_error: bool) -> String {
    let function_name = lower_camel_identifier(&operation.id);
    let type_name = pascal_identifier(&operation.id);
    let throw_on_error = if throw_on_error { "true" } else { "false" };
    let link_path = operation.path.replace('{', ":").replace('}', "");
    let method = operation.method.as_str();
    format!(
        "{ESLINT_HEADER}import type {{ Options, EventStreamResult, SuccessOf }} from './.kaji/client'\nimport type {{ {type_name}Options, {type_name}Responses }} from './{type_name}'\nimport {{ client, toEventStream }} from './.kaji/client'\n\n/**\n * {{@link {link_path}}}\n */\nexport function {function_name}<ThrowOnError extends boolean = {throw_on_error}>(\n  options: Options<{type_name}Options, ThrowOnError> = {{}},\n): Promise<EventStreamResult<SuccessOf<{type_name}Responses>>> {{\n  const {{ client: request = client, ...config }} = options\n\n  return toEventStream<SuccessOf<{type_name}Responses>>(\n    request({{ method: '{method}', url: '{}', responseType: 'stream', ...config, throwOnError: config.throwOnError ?? {throw_on_error} }}),\n  )\n}}\n",
        operation.path,
    )
}

/// Kaji keeps request bodies in `Options`, but emits a content-type hint for
/// forms so the bundled serializer can choose FormData rather than JSON.
fn render_content_type_operation(
    operation: &Operation,
    throw_on_error: bool,
    content_type: &str,
) -> String {
    let function_name = lower_camel_identifier(&operation.id);
    let type_name = pascal_identifier(&operation.id);
    let throw_on_error = if throw_on_error { "true" } else { "false" };
    let link_path = operation.path.replace('{', ":").replace('}', "");
    let method = operation.method.as_str();
    format!(
        "{ESLINT_HEADER}import type {{ Options, Unwrappable, RequestResult }} from './.kaji/client'\nimport type {{ {type_name}Options, {type_name}Responses }} from './{type_name}'\nimport {{ client, withUnwrap }} from './.kaji/client'\n\n/**\n * {{@link {link_path}}}\n */\nexport function {function_name}<ThrowOnError extends boolean = {throw_on_error}>(\n  options: Options<{type_name}Options, ThrowOnError>,\n): Unwrappable<RequestResult<{type_name}Responses, ThrowOnError>> {{\n  const {{ client: request = client, ...config }} = options\n\n  return withUnwrap(\n    request({{\n      method: '{method}',\n      url: '{}',\n      contentType: {{ request: '{content_type}' }},\n      ...config,\n      throwOnError: config.throwOnError ?? {throw_on_error},\n    }}) as Promise<RequestResult<{type_name}Responses, ThrowOnError>>,\n  )\n}}\n",
        operation.path,
    )
}

fn request_content_type(operation: &Operation) -> Option<&str> {
    operation
        .request_body
        .as_ref()?
        .media_types
        .iter()
        .find_map(|media_type| {
            matches!(
                media_type.content_type.as_str(),
                "multipart/form-data" | "application/x-www-form-urlencoded"
            )
            .then_some(media_type.content_type.as_str())
        })
}

fn is_event_stream(operation: &Operation) -> bool {
    operation.responses.iter().any(|response| {
        response
            .media_types
            .iter()
            .any(|media_type| media_type.content_type == "text/event-stream")
    })
}

fn pascal_identifier(value: &str) -> String {
    let mut output = String::new();
    let mut uppercase = true;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if uppercase {
                output.extend(character.to_uppercase());
            } else {
                output.push(character);
            }
            uppercase = false;
        } else {
            uppercase = true;
        }
    }
    if output.is_empty() {
        "Operation".into()
    } else {
        output
    }
}

fn lower_camel_identifier(value: &str) -> String {
    let pascal = pascal_identifier(value);
    let mut characters = pascal.chars();
    match characters.next() {
        Some(first) => first.to_lowercase().collect::<String>() + characters.as_str(),
        None => "operation".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{HttpMethod, SecurityRequirement, SecurityScheme, SecuritySchemeKind};

    fn operation(id: &str, method: HttpMethod, path: &str) -> Operation {
        Operation {
            id: id.into(),
            method,
            path: path.into(),
            response_type: "Pet".into(),
            request_type: None,
            annotations: Default::default(),
            ..Default::default()
        }
    }

    #[test]
    fn renders_kaji_path_parameter_docs_and_source() {
        let source = render_operation(
            &operation("getPetById", HttpMethod::Get, "/pet/{petId}"),
            true,
            None,
            false,
        );
        assert!(source.contains("{@link /pet/:petId}"));
        assert!(source.contains("method: 'GET'"));
        assert!(source.contains("GetPetByIdOptions"));
    }

    #[test]
    fn rejects_unknown_throw_on_error_value() {
        let config = GeneratorConfig::from([("throw_on_error_default".into(), "sometimes".into())]);
        assert!(parse_throw_on_error(&config).is_err());
    }

    #[test]
    fn security_catalog_preserves_http_api_key_locations_and_oauth() {
        let mut operation = operation("getSecure", HttpMethod::Get, "/secure");
        operation.security = ["bearer", "header_key", "query_key", "oauth"]
            .into_iter()
            .map(|name| SecurityRequirement {
                schemes: [(name.into(), Vec::new())].into_iter().collect(),
            })
            .collect();
        let catalog = SecuritySchemeCatalog {
            schemes: vec![
                SecurityScheme {
                    name: "bearer".into(),
                    description: None,
                    kind: SecuritySchemeKind::Http {
                        scheme: Some("bearer".into()),
                        bearer_format: Some("JWT".into()),
                    },
                },
                SecurityScheme {
                    name: "header_key".into(),
                    description: None,
                    kind: SecuritySchemeKind::ApiKey {
                        name: Some("X-API-Key".into()),
                        location: Some("header".into()),
                    },
                },
                SecurityScheme {
                    name: "query_key".into(),
                    description: None,
                    kind: SecuritySchemeKind::ApiKey {
                        name: Some("api_key".into()),
                        location: Some("query".into()),
                    },
                },
                SecurityScheme {
                    name: "oauth".into(),
                    description: None,
                    kind: SecuritySchemeKind::OAuth2 {
                        flows: Vec::new(),
                        metadata_url: None,
                    },
                },
            ],
        };

        let source = render_operation(&operation, true, Some(&catalog), true);
        // SDK generation keeps OpenAPI's OR-of-AND alternatives rather than
        // flattening every scheme into one ambiguous list. The component key
        // is retained as the runtime credential lookup key.
        assert!(source.contains("security: [[{ id: 'bearer', type: 'http', scheme: 'bearer'"));
        assert!(
            source.contains("[{ id: 'header_key', type: 'apiKey', name: 'X-API-Key', in: 'header'")
        );
        assert!(
            source.contains("[{ id: 'query_key', type: 'apiKey', name: 'api_key', in: 'query'")
        );
        assert!(source.contains("id: 'oauth', type: 'oauth2'"));
    }
}
