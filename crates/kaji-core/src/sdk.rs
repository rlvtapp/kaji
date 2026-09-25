//! Language-first SDK generation profiles.
//!
//! This layer deliberately has no product-specific knowledge. Any caller that
//! has normalized an OpenAPI document into [`Api`] can request one or more
//! SDKs. A product API is only an input document
//! plus a set of profiles.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

use crate::plugins::rust::{RustModels, RustPackage, RustReqwest};
use crate::plugins::typescript::{
    TypeScriptAxios, TypeScriptFetch, TypeScriptModels, TypeScriptPackage,
};
use crate::plugins::typescript_clients::{StructuredTypeScriptAxios, StructuredTypeScriptFetch};
use crate::plugins::typescript_models::StructuredTypeScriptModels;
use crate::{
    Api, CodegenPlugin, GeneratedFile, GeneratedTree, GeneratorConfig, Operation,
    SecuritySchemeCatalog, generate,
};

/// Stable SDK languages exposed by the Rust-native generation API.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SdkLanguage {
    Rust,
    TypeScript,
}

/// A transport implementation selected within one language profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SdkTransport {
    Reqwest,
    Fetch,
    Axios,
}

/// Output layout for a language profile. Structured is TypeScript-only and is
/// the default there because it preserves separate models, clients, and
/// runtime files without changing other language targets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SdkStyle {
    Native,
    #[default]
    Structured,
}

/// Whether a TypeScript SDK exposes only generated exports or also an
/// instantiated product-client facade.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SdkSurface {
    /// Models and direct operation functions only.
    Raw,
    /// Models, direct operation functions, and a configured SDK client.
    #[default]
    Client,
}

/// Public instantiated-client layout. TypeScript retains direct function
/// exports in both modes; native language plugins use the same choice to emit
/// either their flat idiomatic client or a resource-namespaced facade.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SdkClientStyle {
    /// `client.createMessage(...)` (or its native-language equivalent).
    Flat,
    /// `client.messages.create(...)` (or its native-language equivalent),
    /// using OpenAPI tags first and stable path-derived namespaces when a spec
    /// has no tags.
    #[default]
    Namespaced,
}

/// One independently generated SDK package.
///
/// `output_dir` is mandatory so multiple language targets can be generated
/// together without colliding. Configuration intentionally stays typed here;
/// config-file adapters can deserialize into this shape without adding a JS
/// runtime to generation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SdkProfile {
    pub language: SdkLanguage,
    pub output_dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_name: Option<String>,
    /// Public client class name for class-oriented SDK targets. When omitted,
    /// it is derived from the OpenAPI title (for example `KajiEmail`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_name: Option<String>,
    #[serde(default)]
    pub client_style: SdkClientStyle,
    #[serde(default)]
    pub surface: SdkSurface,
    #[serde(default)]
    pub transports: Vec<SdkTransport>,
    #[serde(default)]
    pub style: SdkStyle,
    /// Split structured output by the first OpenAPI tag. This prevents operation/schema name
    /// collisions without forcing one giant file. Set this to `false` for a
    /// flat package layout.
    #[serde(default = "default_group_by_tag")]
    pub group_by_tag: bool,
}

fn default_group_by_tag() -> bool {
    true
}

impl SdkProfile {
    pub fn rust(output_dir: impl Into<String>) -> Self {
        Self {
            language: SdkLanguage::Rust,
            output_dir: output_dir.into(),
            package_name: None,
            client_name: None,
            client_style: SdkClientStyle::Namespaced,
            transports: vec![SdkTransport::Reqwest],
            style: SdkStyle::Native,
            surface: SdkSurface::Client,
            group_by_tag: false,
        }
    }

    pub fn typescript(output_dir: impl Into<String>) -> Self {
        Self {
            language: SdkLanguage::TypeScript,
            output_dir: output_dir.into(),
            package_name: None,
            client_name: None,
            client_style: SdkClientStyle::Namespaced,
            transports: vec![SdkTransport::Fetch],
            style: SdkStyle::Structured,
            surface: SdkSurface::Client,
            group_by_tag: true,
        }
    }
}

/// Generates every requested SDK from a single language-neutral API model.
pub fn generate_sdks(api: &Api, profiles: &[SdkProfile]) -> Result<GeneratedTree> {
    generate_sdks_with_security_catalog(api, profiles, None)
}

/// Generates SDK packages with the optional reusable OpenAPI security-scheme
/// catalog.  The catalog is intentionally a separate argument: callers with
/// a pre-existing `Api` retain the exact historical output, while the Docs
/// Compiler sidecar can opt into named, per-operation credentials.
pub fn generate_sdks_with_security_catalog(
    api: &Api,
    profiles: &[SdkProfile],
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> Result<GeneratedTree> {
    let mut result = GeneratedTree::default();
    for profile in profiles {
        let config = profile_config(profile)?;
        let tree = match profile.language {
            SdkLanguage::Rust => {
                require_exact_transports(profile, &[SdkTransport::Reqwest])?;
                let models = RustModels;
                let client = RustReqwest;
                let package = RustPackage;
                generate(
                    api,
                    &[
                        (&models, config.clone()),
                        (&client, config.clone()),
                        (&package, config),
                    ],
                )?
            }
            SdkLanguage::TypeScript => {
                if profile.transports.is_empty() {
                    bail!("TypeScript SDK profiles need at least one transport")
                }
                if profile
                    .transports
                    .iter()
                    .any(|transport| matches!(transport, SdkTransport::Reqwest))
                {
                    bail!("Reqwest is only available for Rust SDK profiles")
                }
                if profile.style == SdkStyle::Structured {
                    require_one_typescript_transport(profile)?;
                    generate_structured_typescript_sdk(api, profile, config, security_schemes)?
                } else {
                    let models = TypeScriptModels;
                    let package = TypeScriptPackage;
                    let fetch = TypeScriptFetch;
                    let axios = TypeScriptAxios;
                    let mut plugins: Vec<(&dyn CodegenPlugin, GeneratorConfig)> =
                        vec![(&models, config.clone()), (&package, config.clone())];
                    if profile.transports.contains(&SdkTransport::Fetch) {
                        plugins.push((&fetch, config.clone()));
                    }
                    if profile.transports.contains(&SdkTransport::Axios) {
                        plugins.push((&axios, config));
                    }
                    generate(api, &plugins)?
                }
            }
        };
        for (path, contents) in tree.iter() {
            result.insert(GeneratedFile::new(path, contents)?)?;
        }
    }
    Ok(result)
}

fn require_one_typescript_transport(profile: &SdkProfile) -> Result<()> {
    if profile.transports.len() != 1
        || !matches!(
            profile.transports[0],
            SdkTransport::Fetch | SdkTransport::Axios
        )
    {
        bail!("structured TypeScript profiles need exactly one of fetch or axios")
    }
    Ok(())
}

fn generate_structured_typescript_sdk(
    api: &Api,
    profile: &SdkProfile,
    mut config: GeneratorConfig,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> Result<GeneratedTree> {
    // Kaji's tag-directory layout otherwise puts every untagged operation in
    // `default`. Kaji's SDK surface uses the first meaningful path segment as
    // a stable resource namespace, matching the native targets.
    let mut sdk_api = api.clone();
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
    let mut type_config = config.clone();
    type_config.insert("output_dir".into(), models_dir);
    config.insert("output_dir".into(), clients_dir);
    config.insert("runtime_dir".into(), ".kaji".into());
    if profile.group_by_tag {
        type_config.insert("group_type".into(), "tag".into());
        config.insert("group_type".into(), "tag".into());
        config.insert("group_default_directory".into(), "true".into());
        config.insert("type_import_prefix".into(), "../../models".into());
    } else {
        config.insert("type_import_prefix".into(), "../models".into());
    }
    let types = StructuredTypeScriptModels;
    let fetch = StructuredTypeScriptFetch;
    let axios = StructuredTypeScriptAxios;
    let mut tree = generate(&sdk_api, &[(&types, type_config)])?;
    let client_files = match profile.transports[0] {
        SdkTransport::Fetch => security_schemes.map_or_else(
            || fetch.generate(&sdk_api, &config),
            |catalog| fetch.generate_with_named_security_catalog(&sdk_api, &config, catalog),
        ),
        SdkTransport::Axios => security_schemes.map_or_else(
            || axios.generate(&sdk_api, &config),
            |catalog| axios.generate_with_named_security_catalog(&sdk_api, &config, catalog),
        ),
        SdkTransport::Reqwest => unreachable!("validated above"),
    }?;
    for file in client_files {
        tree.insert(file)?;
    }
    tree.insert(GeneratedFile::new(
        format!("{root}/.kaji/client.ts"),
        kaji_runtime(profile.transports[0], security_schemes),
    )?)?;
    let client_name = (profile.surface == SdkSurface::Client).then(|| {
        profile
            .client_name
            .clone()
            .unwrap_or_else(|| sdk_client_name(&sdk_api.name))
    });
    if let Some(client_name) = &client_name {
        tree.insert(GeneratedFile::new(
            format!("{root}/client.ts"),
            kaji_sdk_client(
                &sdk_api,
                client_name,
                profile.group_by_tag,
                profile.client_style,
                ".kaji",
            ),
        )?)?;
    }
    tree.insert_custom(GeneratedFile::new(
        format!("{root}/custom/index.ts"),
        "// This module is created once and never overwritten by Kaji.\n// Add stable helpers, exports, or product-specific wrappers here.\nexport {}\n",
    )?)?;
    tree.insert(GeneratedFile::new(
        format!("{root}/index.ts"),
        kaji_barrel(&sdk_api, profile.group_by_tag, client_name.as_deref()),
    )?)?;
    tree.insert(GeneratedFile::new(
        format!("{root}/package.json"),
        kaji_package(&sdk_api, profile.transports[0])?,
    )?)?;
    tree.insert(GeneratedFile::new(
        format!("{root}/README.md"),
        kaji_readme(&sdk_api, profile),
    )?)?;
    tree.insert(GeneratedFile::new(
        format!("{root}/tsconfig.json"),
        "{\n  \"compilerOptions\": { \"declaration\": true, \"module\": \"ESNext\", \"moduleResolution\": \"Bundler\", \"outDir\": \"dist\", \"strict\": true, \"target\": \"ES2022\" },\n  \"include\": [\"**/*.ts\"]\n}\n",
    )?)?;
    Ok(tree)
}

fn kaji_barrel(api: &Api, group_by_tag: bool, client_name: Option<&str>) -> String {
    let mut output = String::new();
    output.push_str("export * from './custom'\n");
    if let Some(client_name) = client_name {
        output.push_str(&format!("export {{ {client_name} }} from './client'\n"));
    }
    for operation in &api.operations {
        let name = pascal_identifier(&operation.id);
        let function = lower_camel_identifier(&operation.id);
        let group = if group_by_tag {
            operation_tag_directory(operation)
        } else {
            String::new()
        };
        let operation_path = if group.is_empty() {
            format!("./models/{name}")
        } else {
            format!("./models/{group}/{name}")
        };
        let client_path = if group.is_empty() {
            format!("./clients/{function}")
        } else {
            format!("./clients/{group}/{function}")
        };
        output.push_str(&format!("export type * from '{operation_path}'\n"));
        output.push_str(&format!("export {{ {function} }} from '{client_path}'\n"));
    }
    for schema in &api.schemas {
        output.push_str(&format!(
            "export type * from './models/{}'\n",
            pascal_identifier(&schema.name)
        ));
    }
    output
}

fn kaji_sdk_client(
    api: &Api,
    class_name: &str,
    group_by_tag: bool,
    style: SdkClientStyle,
    runtime_dir: &str,
) -> String {
    match style {
        SdkClientStyle::Flat => kaji_flat_sdk_client(api, class_name, group_by_tag, runtime_dir),
        SdkClientStyle::Namespaced => {
            kaji_namespaced_sdk_client(api, class_name, group_by_tag, runtime_dir)
        }
    }
}

fn kaji_flat_sdk_client(
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
        let group = if group_by_tag {
            operation_tag_directory(operation)
        } else {
            String::new()
        };
        let path = if group.is_empty() {
            format!("./clients/{function}")
        } else {
            format!("./clients/{group}/{function}")
        };
        output.push_str(&format!("import {{ {function} }} from '{path}'\n"));
    }
    if has_pagination(api) {
        output.push_str(pagination_helpers());
    }
    output.push_str(&format!(
        "\nexport class {class_name} {{\n  private readonly client: ClientInstance\n"
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
        "\n  constructor(config: ClientConfig = {}) {\n    this.client = createClient(config)\n",
    );
    for operation in &api.operations {
        let function = lower_camel_identifier(&operation.id);
        output.push_str(&format!(
            "    this.{function} = ((options: Parameters<typeof {function}>[0]) => {function}({{ ...options, client: this.client }})) as typeof {function}\n"
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

fn kaji_namespaced_sdk_client(
    api: &Api,
    class_name: &str,
    group_by_tag: bool,
    runtime_dir: &str,
) -> String {
    let mut groups: BTreeMap<String, Vec<(&Operation, String)>> = BTreeMap::new();
    for operation in &api.operations {
        let namespace = sdk_namespace(operation);
        let method = sdk_method_name(operation, &namespace);
        groups
            .entry(namespace)
            .or_default()
            .push((operation, method));
    }

    let mut output = String::from(&format!(
        "import type {{ ClientConfig, ClientInstance }} from './{runtime_dir}/client'\nimport {{ createClient }} from './{runtime_dir}/client'\n"
    ));
    for operation in &api.operations {
        let function = lower_camel_identifier(&operation.id);
        let group = if group_by_tag {
            operation_tag_directory(operation)
        } else {
            String::new()
        };
        let path = if group.is_empty() {
            format!("./clients/{function}")
        } else {
            format!("./clients/{group}/{function}")
        };
        output.push_str(&format!("import {{ {function} }} from '{path}'\n"));
    }
    if has_pagination(api) {
        output.push_str(pagination_helpers());
    }

    let mut client_types = Vec::new();
    for (namespace, operations) in &groups {
        let namespace_type = format!("{}Client", pascal_identifier(namespace));
        client_types.push((namespace.clone(), namespace_type.clone()));
        output.push_str(&format!("\nclass {namespace_type} {{\n"));
        let mut names = std::collections::BTreeSet::new();
        for (operation, proposed_name) in operations {
            let function = lower_camel_identifier(&operation.id);
            let name = if names.insert(proposed_name.clone()) {
                proposed_name.clone()
            } else {
                function.clone()
            };
            output.push_str(&format!("  readonly {name}: typeof {function}\n"));
            if pagination(operation).is_some() {
                output.push_str(&format!(
                    "  readonly {name}Pages: (options: Parameters<typeof {function}>[0]) => AsyncIterable<Awaited<ReturnType<typeof {function}>>>\n"
                ));
            }
        }
        output.push_str("\n  constructor(private readonly client: ClientInstance) {\n");
        let mut names = std::collections::BTreeSet::new();
        for (operation, proposed_name) in operations {
            let function = lower_camel_identifier(&operation.id);
            let name = if names.insert(proposed_name.clone()) {
                proposed_name.clone()
            } else {
                function.clone()
            };
            output.push_str(&format!(
                "    this.{name} = ((options: Parameters<typeof {function}>[0]) => {function}({{ ...options, client: this.client }})) as typeof {function}\n"
            ));
            if let Some(pagination) = pagination(operation) {
                output.push_str(&render_pagination_iterator(&name, &function, &pagination));
            }
        }
        output.push_str("  }\n}\n");
    }

    output.push_str(&format!("\nexport class {class_name} {{\n"));
    for (namespace, namespace_type) in &client_types {
        output.push_str(&format!("  readonly {namespace}: {namespace_type}\n"));
    }
    output.push_str(
        "\n  constructor(config: ClientConfig = {}) {\n    const client = createClient(config)\n",
    );
    for (namespace, namespace_type) in &client_types {
        output.push_str(&format!(
            "    this.{namespace} = new {namespace_type}(client)\n"
        ));
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
    let extension = operation
        .annotations
        .get("x-kaji-pagination")
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
        Some("offsetLimit") => {
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
    if let Pagination::Url(pagination) = pagination {
        return render_url_pagination_iterator(public_name, function, pagination);
    }
    let header = format!(
        "    {{\n      const paginationClient = this.client\n      this.{public_name}Pages = async function* (options: Parameters<typeof {function}>[0]) {{\n        let current: unknown = options\n        while (true) {{\n          const response = await {function}({{ ...(current as Record<string, unknown>), client: paginationClient }} as Parameters<typeof {function}>[0])\n          yield response\n"
    );
    let footer = "        }\n      }\n    }\n";
    let body = match pagination {
        Pagination::Cursor(pagination) => {
            let update = if pagination.input_location == "body" {
                format!(
                    "kajiWithBodyValue(current, {:?}, cursor)",
                    pagination
                        .input_body_path
                        .as_deref()
                        .expect("validated request body cursor")
                )
            } else {
                format!(
                    "kajiWithValue(current, {:?}, {:?}, cursor)",
                    pagination.input_location, pagination.input_name
                )
            };
            format!(
                "          const cursor = kajiJsonPath(response, {:?})\n          if (cursor === undefined || cursor === null || cursor === '') return\n          const next = {update}\n          if (next === undefined) return\n          current = next\n",
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
) -> String {
    format!(
        "    {{\n      const paginationClient = this.client\n      this.{public_name}Pages = async function* (options: Parameters<typeof {function}>[0]) {{\n        let current: unknown = options\n        let nextUrl: string | undefined\n        while (true) {{\n          const request: ClientInstance = nextUrl === undefined\n            ? paginationClient\n            : (operation) => paginationClient({{ ...operation, paginationUrl: nextUrl }})\n          const requestOptions = nextUrl === undefined\n            ? (current as Record<string, unknown>)\n            : {{ ...(current as Record<string, unknown>), query: undefined }}\n          const response = await {function}({{ ...requestOptions, client: request }} as Parameters<typeof {function}>[0])\n          yield response\n          nextUrl = kajiPaginationUrl(response, {:?})\n          if (nextUrl === undefined) return\n        }}\n      }}\n    }}\n",
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
            "          const currentValue = Number({current_value})\n          const nextValue = currentValue + 1\n          const numPages = Number(kajiJsonPath(response, {:?}))\n          if (!Number.isFinite(currentValue) || !Number.isFinite(numPages) || nextValue > numPages) return\n          const next = {update}\n          if (next === undefined) return\n          current = next\n",
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
        "          const items = kajiJsonPath(response, {:?})\n          if (!Array.isArray(items)) return\n          const configuredLimit = {limit}\n          if (items.length === 0 || (Number.isFinite(configuredLimit) && items.length < configuredLimit)) return\n          const currentValue = Number({current_value})\n          if (!Number.isFinite(currentValue)) return\n          const nextValue = {increment}\n          const next = {update}\n          if (next === undefined) return\n          current = next\n",
        results_path,
    ));
    output
}

fn pagination_input_value(input: &PaginationInput) -> String {
    if input.location == "body" {
        format!(
            "kajiBodyValue(current, {:?})",
            input
                .body_path
                .as_deref()
                .expect("validated request body pagination input")
        )
    } else {
        format!(
            "kajiOptionValue(current, {:?}, {:?})",
            input.location, input.name
        )
    }
}

fn pagination_input_update(input: &PaginationInput, value: &str) -> String {
    if input.location == "body" {
        format!(
            "kajiWithBodyValue(current, {:?}, {value})",
            input
                .body_path
                .as_deref()
                .expect("validated request body pagination input")
        )
    } else {
        format!(
            "kajiWithValue(current, {:?}, {:?}, {value})",
            input.location, input.name
        )
    }
}

fn pagination_helpers() -> &'static str {
    "\nconst kajiJsonPath = (value: unknown, path: string): unknown => {\n  if (!path.startsWith('$')) return undefined\n  let current: unknown = value\n  for (const segment of path.slice(1).split('.').filter(Boolean)) {\n    const match = /^([^[]+)(?:\\[(-?\\d+)\\])?$/.exec(segment)\n    if (!match || current === null || typeof current !== 'object') return undefined\n    current = (current as Record<string, unknown>)[match[1]]\n    if (match[2] !== undefined) {\n      if (!Array.isArray(current)) return undefined\n      const index = Number(match[2])\n      current = current[index < 0 ? current.length + index : index]\n    }\n  }\n  return current\n}\nconst kajiPaginationUrl = (response: unknown, path: string): string | undefined => {\n  const value = kajiJsonPath(response, path)\n  return typeof value === 'string' && value.trim() ? value : undefined\n}\nconst kajiOptionValue = (options: unknown, location: string, name: string): unknown => {\n  const current = (options ?? {}) as Record<string, unknown>\n  const section = current[location] as Record<string, unknown> | undefined\n  return section?.[name]\n}\nconst kajiWithValue = (options: unknown, location: string, name: string, value: unknown): Record<string, unknown> => {\n  const current = (options ?? {}) as Record<string, unknown>\n  const section = (current[location] ?? {}) as Record<string, unknown>\n  return { ...current, [location]: { ...section, [name]: value }\n}\n// `bodyPath` is an explicit RFC 6901 JSON Pointer. This helper creates new\n// objects/arrays only along that declared path; it never mutates caller input\n// and refuses to invent a missing array shape.\nconst kajiJsonPointer = (path: string): string[] | undefined => {\n  if (!path.startsWith('/') || path.includes('//')) return undefined\n  return path.slice(1).split('/').map((part) => part.split('~1').join('/').split('~0').join('~'))\n}\nconst kajiBodyValue = (options: unknown, path: string): unknown => {\n  const current = (options ?? {}) as Record<string, unknown>\n  const pointer = kajiJsonPointer(path)\n  if (!pointer) return undefined\n  let value: unknown = current.body\n  for (const key of pointer) {\n    if (value === null || typeof value !== 'object') return undefined\n    value = Array.isArray(value) ? value[Number(key)] : (value as Record<string, unknown>)[key]\n  }\n  return value\n}\nconst kajiWithBodyValue = (options: unknown, path: string, value: unknown): Record<string, unknown> | undefined => {\n  const current = (options ?? {}) as Record<string, unknown>\n  const pointer = kajiJsonPointer(path)\n  if (!pointer || pointer.length === 0) return undefined\n  const update = (node: unknown, index: number): unknown | undefined => {\n    const key = pointer[index]\n    if (Array.isArray(node)) {\n      if (!/^\\d+$/.test(key)) return undefined\n      const position = Number(key)\n      if (!Number.isSafeInteger(position) || position < 0 || position >= node.length) return undefined\n      const copy = node.slice()\n      const next = index + 1 === pointer.length ? value : update(node[position], index + 1)\n      if (next === undefined) return undefined\n      copy[position] = next\n      return copy\n    }\n    if (node !== null && typeof node === 'object') {\n      const record = node as Record<string, unknown>\n      const next = index + 1 === pointer.length ? value : update(record[key] ?? {}, index + 1)\n      if (next === undefined) return undefined\n      return { ...record, [key]: next }\n    }\n    // An absent optional object can be created, but scalar and array shapes\n    // remain unrepresentable without a schema-directed declaration.\n    if (node === undefined || node === null) return update({}, index)\n    return undefined\n  }\n  const body = update(current.body ?? {}, 0)\n  return body === undefined ? undefined : { ...current, body }\n}\n"
}

fn sdk_client_name(api_name: &str) -> String {
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

fn operation_tag_directory(operation: &crate::Operation) -> String {
    sdk_namespace(operation)
}

fn operation_tag_directory_if_present(operation: &crate::Operation) -> Option<String> {
    operation
        .annotations
        .get("tags")
        .and_then(serde_json::Value::as_array)
        .and_then(|tags| tags.first())
        .and_then(serde_json::Value::as_str)
        .map(lower_camel_identifier)
        .filter(|tag| !tag.is_empty())
}

fn kaji_package(api: &Api, transport: SdkTransport) -> Result<String> {
    let package_name = kaji_package_name(api, transport);
    let mut package = serde_json::json!({
        "name": package_name,
        "version": package_version(&api.version),
        "type": "module",
        "sideEffects": false,
        "exports": { ".": { "types": "./dist/index.d.ts", "import": "./dist/index.js" } },
        "files": ["dist"],
        "scripts": { "build": "tsc -p tsconfig.json" },
        "devDependencies": { "typescript": "^5.0.0" }
    });
    if transport == SdkTransport::Axios {
        package["peerDependencies"] = serde_json::json!({ "axios": "^1.0.0" });
    }
    Ok(format!("{}\n", serde_json::to_string_pretty(&package)?))
}

fn kaji_package_name(api: &Api, transport: SdkTransport) -> String {
    format!(
        "@kaji/{}-{}",
        package_slug(&api.name),
        match transport {
            SdkTransport::Fetch => "fetch",
            SdkTransport::Axios => "axios",
            SdkTransport::Reqwest => unreachable!(),
        }
    )
}

fn kaji_readme(api: &Api, profile: &SdkProfile) -> String {
    let package_name = kaji_package_name(api, profile.transports[0]);
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
    format!(
        "# {} TypeScript SDK\n\nGenerated by Kaji.\n\n```sh\nnpm install {package_name}\n```\n\n```ts\n{client}\n```\n\nSee [STYLE_GUIDE.md](STYLE_GUIDE.md) for the selected client surface.\n",
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

fn kaji_runtime(
    transport: SdkTransport,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> String {
    let security_types = render_security_types(security_schemes);
    match transport {
        SdkTransport::Fetch => {
            format!(
                "{}\n{}",
                security_types,
                r#"export interface RetryConfig { maxAttempts?: number; initialDelayMs?: number; maxDelayMs?: number }
export interface RequestHookContext { method: string; url: string; body?: unknown; path?: Record<string, unknown>; query: Record<string, unknown>; headers: Headers }
export interface ResponseHookContext { request: RequestHookContext; status: number; response: Response }
export interface ClientHooks { beforeRequest?: (request: RequestHookContext) => void | Promise<void>; afterResponse?: (response: ResponseHookContext) => void | Promise<void>; onError?: (error: unknown, request: RequestHookContext) => void | Promise<void> }
export interface ClientConfig { baseUrl?: string; apiKey?: string; apiKeyHeader?: string; apiKeyPrefix?: string; auth?: SecurityCredentials; headers?: HeadersInit; fetch?: typeof globalThis.fetch; retry?: RetryConfig | false; hooks?: ClientHooks }
export type RequestConfig = { method: string; url: string; body?: unknown; path?: Record<string, unknown>; query?: Record<string, unknown>; headers?: HeadersInit; throwOnError?: boolean; security?: unknown; contentType?: { request?: string }; responseType?: 'stream'; styles?: unknown; paginationUrl?: string }
export type ClientInstance = (request: RequestConfig) => Promise<unknown>
export type Options<T, ThrowOnError extends boolean> = T & { client?: ClientInstance; throwOnError?: ThrowOnError }
export type RequestResult<T, _ThrowOnError extends boolean> = T
export type Unwrappable<T> = Promise<T> & { unwrap(): Promise<T> }
export type SuccessOf<T> = T
export type EventStreamResult<T> = AsyncIterable<T>
const resolveUrl = (template: string, path?: Record<string, unknown>, query?: Record<string, unknown>) => {
  const url = template.replace(/\{([^}]+)\}/g, (_match, key) => encodeURIComponent(String(path?.[key] ?? `{${key}}`)))
  const params = new URLSearchParams()
  for (const [key, value] of Object.entries(query ?? {})) {
    if (value === undefined || value === null) continue
    for (const item of Array.isArray(value) ? value : [value]) params.append(key, String(item))
  }
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
const requestBody = (body: unknown, headers: Headers) => {
  if (body === undefined || body === null || typeof body === 'string' || body instanceof FormData || body instanceof URLSearchParams || body instanceof Blob) return body as BodyInit | undefined
  if (!headers.has('content-type')) headers.set('content-type', 'application/json')
  return JSON.stringify(body)
}
const applySecurity = (headers: Headers, query: Record<string, unknown>, security: unknown, credentials?: SecurityCredentials) => {
  if (!Array.isArray(security)) return
  const alternatives = (Array.isArray(security[0]) ? security : [security]) as Array<Array<SecurityDescriptor>>
  const values = credentials ?? {}
  const selected = alternatives.find((alternative) => alternative.every((scheme) => !scheme.id || values[scheme.id]))
  if (!selected) return
  for (const scheme of selected) {
    const value = scheme.id ? values[scheme.id] : undefined
    if (!value) continue
    if (scheme.type === 'apiKey') {
      if (scheme.in === 'query') query[scheme.name ?? scheme.id] = value
      else headers.set(scheme.name ?? scheme.id, value)
    } else headers.set('authorization', `${scheme.scheme === 'basic' ? 'Basic' : 'Bearer'} ${value}`)
  }
}
const retryableStatus = (status: number) => status === 408 || status === 429 || status === 500 || status === 502 || status === 503 || status === 504
const retryAllowed = (method: string, headers: Headers) => ['GET', 'PUT', 'PATCH', 'DELETE'].includes(method.toUpperCase()) || (method.toUpperCase() === 'POST' && headers.has('idempotency-key'))
const retryDelay = async (attempt: number, retry: RetryConfig, retryAfter?: string | null) => {
  const retryAfterMs = retryAfter && /^\d+(?:\.\d+)?$/.test(retryAfter) ? Number(retryAfter) * 1000 : undefined
  const exponential = (retry.initialDelayMs ?? 250) * 2 ** attempt
  const delay = Math.min(retryAfterMs ?? exponential, retry.maxDelayMs ?? 8_000)
  await new Promise<void>((resolve) => setTimeout(resolve, delay))
}
export const createClient = (config: ClientConfig = {}): ClientInstance => async ({ method, url, body, path, query, headers, throwOnError: _throwOnError, security, responseType, paginationUrl }) => {
  const mergedHeaders = new Headers(config.headers)
  if (config.apiKey) mergedHeaders.set(config.apiKeyHeader ?? 'authorization', `${config.apiKeyPrefix ?? 'Bearer '}${config.apiKey}`)
  new Headers(headers).forEach((value, key) => mergedHeaders.set(key, value))
  const resolvedQuery = { ...(query ?? {}) }
  applySecurity(mergedHeaders, resolvedQuery, security, config.auth)
  const requestUrl = paginationUrl ? resolvePaginationUrl(paginationUrl, config.baseUrl) : `${config.baseUrl ?? ''}${resolveUrl(url, path, resolvedQuery)}`
  const request = { method, url: requestUrl, body, path, query: resolvedQuery, headers: mergedHeaders }
  await config.hooks?.beforeRequest?.(request)
  const retry = config.retry === false ? undefined : config.retry ?? {}
  const maxAttempts = retry && retryAllowed(method, mergedHeaders) ? Math.max(1, retry.maxAttempts ?? 3) : 1
  let response!: Response
  for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
    try {
      response = await (config.fetch ?? globalThis.fetch)(requestUrl, { method, body: requestBody(body, mergedHeaders), headers: mergedHeaders })
      if (attempt + 1 < maxAttempts && retryableStatus(response.status)) {
        await retryDelay(attempt, retry ?? {}, response.headers.get('retry-after'))
        continue
      }
      break
    } catch (error) {
      if (attempt + 1 >= maxAttempts) {
        await config.hooks?.onError?.(error, request)
        throw error
      }
      await retryDelay(attempt, retry ?? {})
    }
  }
  if (!response.ok && _throwOnError !== false) {
    const error = new ApiError(response.status, await response.text())
    await config.hooks?.onError?.(error, request)
    throw error
  }
  await config.hooks?.afterResponse?.({ request, status: response.status, response: response.clone() })
  if (responseType === 'stream') return response
  if (response.status === 204) return undefined
  const contentType = response.headers.get('content-type') ?? ''
  if (contentType.includes('application/json') || contentType.includes('+json')) return response.json()
  if (contentType.startsWith('text/')) return response.text()
  return response.arrayBuffer()
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
export const client = createClient()
export const withUnwrap = <T>(promise: Promise<T>): Unwrappable<T> => Object.assign(promise, { unwrap: () => promise })
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
export interface ClientConfig { baseUrl?: string; apiKey?: string; apiKeyHeader?: string; apiKeyPrefix?: string; auth?: SecurityCredentials; headers?: Record<string, string>; client?: AxiosInstance; retry?: RetryConfig | false; hooks?: ClientHooks }
export type RequestConfig = { method: string; url: string; body?: unknown; path?: Record<string, unknown>; query?: Record<string, unknown>; headers?: Record<string, string>; throwOnError?: boolean; security?: unknown; contentType?: { request?: string }; responseType?: 'stream'; styles?: unknown; paginationUrl?: string }
export type ClientInstance = (request: RequestConfig) => Promise<unknown>
export type Options<T, ThrowOnError extends boolean> = T & { client?: ClientInstance; throwOnError?: ThrowOnError }
export type RequestResult<T, _ThrowOnError extends boolean> = T
export type Unwrappable<T> = Promise<T> & { unwrap(): Promise<T> }
export type SuccessOf<T> = T
export type EventStreamResult<T> = AsyncIterable<T>
const resolvePath = (template: string, path?: Record<string, unknown>) => template.replace(/\{([^}]+)\}/g, (_match, key) => encodeURIComponent(String(path?.[key] ?? `{${key}}`)))
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
const applySecurity = (headers: Record<string, string>, query: Record<string, unknown>, security: unknown, credentials?: SecurityCredentials) => {
  if (!Array.isArray(security)) return
  const alternatives = (Array.isArray(security[0]) ? security : [security]) as Array<Array<SecurityDescriptor>>
  const values = credentials ?? {}
  const selected = alternatives.find((alternative) => alternative.every((scheme) => !scheme.id || values[scheme.id]))
  if (!selected) return
  for (const scheme of selected) {
    const value = scheme.id ? values[scheme.id] : undefined
    if (!value) continue
    if (scheme.type === 'apiKey') {
      if (scheme.in === 'query') query[scheme.name ?? scheme.id] = value
      else headers[scheme.name ?? scheme.id] = value
    } else headers.authorization = `${scheme.scheme === 'basic' ? 'Basic' : 'Bearer'} ${value}`
  }
}
const retryableStatus = (status: number) => status === 408 || status === 429 || status === 500 || status === 502 || status === 503 || status === 504
const retryAllowed = (method: string, headers: Record<string, string>) => ['GET', 'PUT', 'PATCH', 'DELETE'].includes(method.toUpperCase()) || (method.toUpperCase() === 'POST' && Object.keys(headers).some((key) => key.toLowerCase() === 'idempotency-key'))
const retryDelay = async (attempt: number, retry: RetryConfig, retryAfter?: string) => {
  const retryAfterMs = retryAfter && /^\d+(?:\.\d+)?$/.test(retryAfter) ? Number(retryAfter) * 1000 : undefined
  const exponential = (retry.initialDelayMs ?? 250) * 2 ** attempt
  const delay = Math.min(retryAfterMs ?? exponential, retry.maxDelayMs ?? 8_000)
  await new Promise<void>((resolve) => setTimeout(resolve, delay))
}
export const createClient = (config: ClientConfig = {}): ClientInstance => {
  const headers = { ...config.headers }
  if (config.apiKey) headers[config.apiKeyHeader ?? 'authorization'] = `${config.apiKeyPrefix ?? 'Bearer '}${config.apiKey}`
  const instance = config.client ?? axios.create({ baseURL: config.baseUrl, headers })
  return async ({ method, url, body, path, query, headers: requestHeaders, throwOnError: _throwOnError, security, responseType, paginationUrl }) => {
    const resolvedHeaders = { ...headers, ...requestHeaders }
    const resolvedQuery = { ...(query ?? {}) }
    applySecurity(resolvedHeaders, resolvedQuery, security, config.auth)
    const requestUrl = paginationUrl ? resolvePaginationUrl(paginationUrl, config.baseUrl) : resolvePath(url, path)
    const request = { method, url: requestUrl, body, path, query: resolvedQuery, headers: resolvedHeaders }
    await config.hooks?.beforeRequest?.(request)
    const retry = config.retry === false ? undefined : config.retry ?? {}
    const maxAttempts = retry && retryAllowed(method, resolvedHeaders) ? Math.max(1, retry.maxAttempts ?? 3) : 1
    for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
      try {
        const response = await instance.request({ method, url: requestUrl, data: body, params: resolvedQuery, headers: resolvedHeaders, responseType: responseType === 'stream' ? 'stream' : undefined, validateStatus: () => true })
        if (attempt + 1 < maxAttempts && retryableStatus(response.status)) {
          await retryDelay(attempt, retry ?? {}, response.headers['retry-after'])
          continue
        }
        if (response.status >= 400 && _throwOnError !== false) throw new ApiError(response.status, response.data)
        await config.hooks?.afterResponse?.({ request, status: response.status, headers: response.headers as Record<string, unknown>, data: response.data })
        return response.data
      } catch (error) {
        if (error instanceof ApiError || attempt + 1 >= maxAttempts) {
          await config.hooks?.onError?.(error, request)
          throw error
        }
        await retryDelay(attempt, retry ?? {})
      }
    }
    throw new Error('Kaji retry loop completed without a response')
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
export const client = createClient()
export const withUnwrap = <T>(promise: Promise<T>): Unwrappable<T> => Object.assign(promise, { unwrap: () => promise })
"#
            )
        }
        SdkTransport::Reqwest => unreachable!("not a TypeScript transport"),
    }
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
        "export interface SecurityCredentials {{\n{fields}\n}}\nexport type SecurityDescriptor = {{ id?: string; type: 'apiKey' | 'http' | 'oauth2'; name?: string; in?: 'header' | 'query' | 'cookie'; scheme?: string; scopes?: string[] }}\nexport class ApiError extends Error {{\n  constructor(public readonly status: number, public readonly body: unknown) {{ super(`Request failed: ${{status}}`) }}\n}}"
    )
}

/// Loads any completed OpenAPI sidecar output and generates the requested SDK
/// profiles. This is the production bridge from the existing Go OpenAPI
/// parser to language-first SDK generation; it contains no API or product
/// naming assumptions.
pub fn generate_openapi_sdks(
    sidecar_output: &Path,
    name: impl Into<String>,
    version: impl Into<String>,
    profiles: &[SdkProfile],
) -> Result<GeneratedTree> {
    let api = crate::adapter::openapi_sidecar::load_operations(
        sidecar_output,
        name.into(),
        version.into(),
    )?;
    let security_schemes_path = sidecar_output.join("security-schemes.json");
    let security_schemes = security_schemes_path
        .exists()
        .then(|| crate::adapter::openapi_sidecar::load_security_schemes(sidecar_output))
        .transpose()?;
    generate_sdks_with_security_catalog(&api, profiles, security_schemes.as_ref())
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

fn profile_config(profile: &SdkProfile) -> Result<GeneratorConfig> {
    let output_dir = profile.output_dir.trim_matches('/');
    if output_dir.is_empty() || output_dir == "." {
        bail!("SDK profiles need a non-empty output_dir")
    }
    let mut config = GeneratorConfig::from([("output_dir".into(), output_dir.into())]);
    config.insert(
        "sdk_surface".into(),
        match profile.surface {
            SdkSurface::Raw => "raw",
            SdkSurface::Client => "client",
        }
        .into(),
    );
    config.insert(
        "client_style".into(),
        match profile.client_style {
            SdkClientStyle::Flat => "flat",
            SdkClientStyle::Namespaced => "namespaced",
        }
        .into(),
    );
    if let Some(package_name) = &profile.package_name {
        config.insert("package_name".into(), package_name.clone());
        config.insert("crate_name".into(), package_name.clone());
    }
    let clients = profile
        .transports
        .iter()
        .filter_map(|transport| match transport {
            SdkTransport::Fetch => Some("fetch"),
            SdkTransport::Axios => Some("axios"),
            SdkTransport::Reqwest => None,
        })
        .collect::<Vec<_>>();
    if !clients.is_empty() {
        config.insert("clients".into(), clients.join(","));
    }
    Ok(config)
}

fn require_exact_transports(profile: &SdkProfile, allowed: &[SdkTransport]) -> Result<()> {
    if profile.transports.is_empty()
        || profile
            .transports
            .iter()
            .any(|transport| !allowed.contains(transport))
    {
        bail!("Rust SDK profiles currently support the reqwest transport")
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::{
        HttpMethod, Operation, OperationMediaType, OperationRequestBody, Schema, SchemaKind,
        SchemaValue, SecurityRequirement, SecurityScheme, SecuritySchemeKind,
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
                response_type: "MessageList".into(),
                security: vec![SecurityRequirement {
                    schemes: [("kajiApiKey".into(), Vec::new())].into_iter().collect(),
                }],
                ..Operation::default()
            }],
            ..Api::default()
        };
        let catalog = SecuritySchemeCatalog {
            schemes: vec![SecurityScheme {
                name: "kajiApiKey".into(),
                description: None,
                kind: SecuritySchemeKind::ApiKey {
                    name: Some("X-API-Key".into()),
                    location: Some("header".into()),
                },
            }],
        };

        let tree = generate_sdks_with_security_catalog(
            &api,
            &[SdkProfile::typescript("sdk/typescript")],
            Some(&catalog),
        )
        .unwrap();
        let runtime = tree.get("sdk/typescript/.kaji/client.ts").unwrap();
        let operation = tree
            .get("sdk/typescript/clients/messages/listMessages.ts")
            .unwrap();
        assert!(runtime.contains("\"kajiApiKey\"?: string"));
        assert!(runtime.contains("applySecurity"));
        assert!(operation.contains("id: 'kajiApiKey'"));
        assert!(operation.contains("name: 'X-API-Key', in: 'header'"));
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
        assert!(iterator.contains("kajiJsonPath(response, \"$.data.results\")"));
        assert!(iterator.contains("currentValue + items.length"));
        assert!(iterator.contains("kajiWithValue(current, \"query\", \"offset\", nextValue)"));
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
                "x-kaji-pagination".into(),
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
        assert!(iterator.contains("kajiWithBodyValue(current, \"/filters/page/cursor\", cursor)"));
        assert!(iterator.contains("const next = kajiWithBodyValue"));
        assert!(pagination_helpers().contains("kajiJsonPointer"));

        let invalid = Operation {
            annotations: BTreeMap::from([(
                "x-kaji-pagination".into(),
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
        assert!(rendered.contains("kajiBodyValue(current, \"/page/offset\")"));
        assert!(rendered.contains("kajiWithBodyValue(current, \"/page/offset\", nextValue)"));
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
        assert!(iterator.contains("kajiPaginationUrl(response, \"$.links.next\")"));
        assert!(!iterator.contains("url: nextUrl"));

        let fetch_runtime = kaji_runtime(SdkTransport::Fetch, None);
        assert!(fetch_runtime.contains("paginationUrl?: string"));
        assert!(fetch_runtime.contains("Pagination URL must use the configured API origin"));
        assert!(
            fetch_runtime
                .contains("applySecurity(mergedHeaders, resolvedQuery, security, config.auth)")
        );

        let axios_runtime = kaji_runtime(SdkTransport::Axios, None);
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
    fn one_openapi_model_generates_isolated_rust_and_typescript_sdks() {
        let api = Api {
            name: "Example API".into(),
            version: "1.0.0".into(),
            schemas: vec![Schema::new("Message", SchemaValue::new(SchemaKind::String))],
            operations: vec![Operation {
                id: "sendMessage".into(),
                method: HttpMethod::Post,
                path: "/messages".into(),
                response_type: "Message".into(),
                request_type: Some("Message".into()),
                ..Operation::default()
            }],
            ..Api::default()
        };
        let tree = generate_sdks(
            &api,
            &[
                SdkProfile::rust("sdk/rust"),
                SdkProfile {
                    transports: vec![SdkTransport::Fetch, SdkTransport::Axios],
                    style: SdkStyle::Native,
                    ..SdkProfile::typescript("sdk/typescript")
                },
            ],
        )
        .unwrap();
        assert!(tree.get("sdk/rust/src/client.rs").is_some());
        assert!(tree.get("sdk/typescript/fetch.ts").is_some());
        assert!(tree.get("sdk/typescript/axios.ts").is_some());
        assert!(tree.get("sdk/typescript/package.json").is_some());
    }

    #[test]
    fn profiles_reject_a_transport_for_the_wrong_language() {
        let error = generate_sdks(
            &Api::default(),
            &[SdkProfile {
                language: SdkLanguage::Rust,
                output_dir: "sdk/rust".into(),
                package_name: None,
                client_name: None,
                client_style: SdkClientStyle::Flat,
                surface: SdkSurface::Raw,
                transports: vec![SdkTransport::Fetch],
                style: SdkStyle::Native,
                group_by_tag: false,
            }],
        )
        .unwrap_err();
        assert!(error.to_string().contains("reqwest"));
    }

    #[test]
    fn sidecar_openapi_output_uses_the_same_language_profiles() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("operations.json"),
            r#"{"GET /messages":"get_messages.json"}"#,
        )
        .unwrap();
        fs::write(
            directory.path().join("operations-order.json"),
            r#"["GET /messages"]"#,
        )
        .unwrap();
        fs::create_dir(directory.path().join("operations")).unwrap();
        fs::write(
            directory.path().join("operations/get_messages.json"),
            r#"{"path":"/messages","method":"GET","responses":[]}"#,
        )
        .unwrap();
        let tree = generate_openapi_sdks(
            directory.path(),
            "Any API",
            "1.0.0",
            &[SdkProfile::typescript("sdk/typescript")],
        )
        .unwrap();
        assert!(
            tree.get("sdk/typescript/clients/messages/getMessages.ts")
                .is_some()
        );
        let client = tree.get("sdk/typescript/client.ts").unwrap();
        assert!(client.contains("export class Any"));
        assert!(client.contains("class MessagesClient"));
        assert!(client.contains("readonly get: typeof getMessages"));
        assert!(!client.contains("bind()"));
    }

    #[test]
    fn package_versions_accept_openapi_date_versions() {
        assert_eq!(package_version("2026-09-19"), "2026.9.19");
        assert_eq!(package_version("1.2.3"), "1.2.3");
        assert_eq!(package_version("latest"), "0.1.0");
    }

    #[test]
    fn sdk_client_names_strip_the_openapi_api_suffix() {
        assert_eq!(sdk_client_name("Kaji Email API"), "KajiEmail");
        assert_eq!(sdk_client_name("Mistral API"), "Mistral");
    }
}
