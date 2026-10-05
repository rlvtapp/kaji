//! A small, dependency-free MCP stdio server for an OpenAPI-derived API.
//!
//! It deliberately keeps credentials outside the generated contract: callers
//! pass authorization in a tool call's `headers` object, while the process
//! receives only the explicitly configured API origin.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::{self, BufRead, Read, Write};
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use kaji_core::{Api, Operation, SchemaKind, SchemaValue};
use serde_json::{Map, Value, json};

const MAX_RESPONSE_BYTES: u64 = 1_048_576;

pub fn serve(api: Api, base_url: &str) -> Result<()> {
    let server = Server::new(api, base_url)?;
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.context("read MCP stdin")?;
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                write_message(
                    &mut stdout,
                    error_response(Value::Null, -32700, &error.to_string()),
                )?;
                continue;
            }
        };
        if let Some(response) = server.handle(request) {
            write_message(&mut stdout, response)?;
        }
    }
    Ok(())
}

/// Serves Kaji itself as MCP tools. This is intentionally separate from the
/// OpenAPI-derived API server above: it lets an agent generate an SDK without
/// granting it an API base URL or conflating generator controls with API calls.
pub fn serve_generator() -> Result<()> {
    let server = GeneratorServer;
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.context("read MCP stdin")?;
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                write_message(
                    &mut stdout,
                    error_response(Value::Null, -32700, &error.to_string()),
                )?;
                continue;
            }
        };
        if let Some(response) = server.handle(request) {
            write_message(&mut stdout, response)?;
        }
    }
    Ok(())
}

struct GeneratorServer;

impl GeneratorServer {
    fn handle(&self, request: Value) -> Option<Value> {
        let id = request.get("id").cloned();
        let Some(method) = request.get("method").and_then(Value::as_str) else {
            return id.map(|id| error_response(id, -32600, "JSON-RPC request requires method"));
        };
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": request.pointer("/params/protocolVersion").and_then(Value::as_str).unwrap_or("2025-03-26"),
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "kaji-generator", "version": env!("CARGO_PKG_VERSION") },
            })),
            "tools/list" => Ok(json!({ "tools": [
                {
                    "name": "kaji_languages",
                    "description": "List Kaji's maintained SDK language targets.",
                    "inputSchema": { "type": "object", "additionalProperties": false },
                },
                {
                    "name": "kaji_generate",
                    "description": "Generate SDKs from a local OpenAPI file. Files under output are overwritten; custom files are preserved.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "source": { "type": "string", "description": "Local OpenAPI 2.0/3.x file" },
                            "output": { "type": "string", "description": "Generated SDK output directory" },
                            "languages": { "type": "array", "items": { "type": "string", "enum": ["rust", "typescript", "go", "python", "php", "symfony", "java", "csharp", "dotnet", "elixir", "ruby", "swift"] }, "minItems": 1 },
                            "name": { "type": "string" },
                            "sdkVersion": { "type": "string" },
                            "clientStyle": { "type": "string", "enum": ["namespaced", "flat"] },
                            "typescriptTransport": { "type": "string", "enum": ["fetch", "axios"] },
                        },
                        "required": ["source", "output", "languages"],
                        "additionalProperties": false,
                    },
                }
            ] })),
            "tools/call" => self.call(request.get("params").cloned().unwrap_or(Value::Null)),
            _ => Err(anyhow::anyhow!("method {method:?} is not supported")),
        };
        id.map(|id| match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(error) if method == "tools/call" => {
                json!({ "jsonrpc": "2.0", "id": id, "result": tool_error(&format!("{error:#}")) })
            }
            Err(error) => error_response(id, -32601, &format!("{error:#}")),
        })
    }

    fn call(&self, params: Value) -> Result<Value> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .context("tools/call requires params.name")?;
        match name {
            "kaji_languages" => Ok(json!({
                "content": [{ "type": "text", "text": "rust, typescript, go, python, php, symfony, java, csharp, dotnet (legacy alias), elixir, ruby, swift; Terraform requires an explicit config-file resource mapping." }],
            })),
            "kaji_generate" => {
                self.generate(params.get("arguments").cloned().unwrap_or(Value::Null))
            }
            _ => bail!("unknown generator tool {name:?}"),
        }
    }

    fn generate(&self, arguments: Value) -> Result<Value> {
        let arguments = arguments
            .as_object()
            .context("kaji_generate arguments must be an object")?;
        let source = required_string(arguments, "source")?;
        let output = required_string(arguments, "output")?;
        if !Path::new(source).is_file() {
            bail!("source is not a readable local file: {source}")
        }
        let languages = arguments
            .get("languages")
            .and_then(Value::as_array)
            .context("kaji_generate requires languages as a non-empty array")?;
        let languages = languages
            .iter()
            .map(|value| value.as_str().context("language must be a string"))
            .collect::<Result<Vec<_>>>()?;
        if languages.is_empty() {
            bail!("kaji_generate requires at least one language")
        }
        let allowed = [
            "rust",
            "typescript",
            "go",
            "python",
            "php",
            "symfony",
            "java",
            "csharp",
            "dotnet",
            "elixir",
            "ruby",
            "swift",
        ];
        if let Some(unknown) = languages
            .iter()
            .find(|language| !allowed.contains(language))
        {
            bail!("unsupported language {unknown:?}")
        }
        let executable = std::env::current_exe().context("locate kaji executable")?;
        let mut command = Command::new(executable);
        command
            .arg("generate")
            .arg(source)
            .arg("--output")
            .arg(output)
            .arg("--language")
            .arg(languages.join(","));
        for (argument, option) in [
            ("name", "--name"),
            ("sdkVersion", "--sdk-version"),
            ("clientStyle", "--client-style"),
            ("typescriptTransport", "--typescript-transport"),
        ] {
            if let Some(value) = arguments.get(argument) {
                command.arg(option).arg(
                    value
                        .as_str()
                        .with_context(|| format!("{argument} must be a string"))?,
                );
            }
        }
        let result = command.output().context("run kaji generate")?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        if !result.status.success() {
            bail!("kaji generate failed: {text}")
        }
        Ok(json!({ "content": [{ "type": "text", "text": text }] }))
    }
}

fn required_string<'a>(arguments: &'a Map<String, Value>, name: &str) -> Result<&'a str> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .with_context(|| format!("kaji_generate requires a non-empty {name}"))
}

fn write_message(writer: &mut impl Write, message: Value) -> Result<()> {
    serde_json::to_writer(&mut *writer, &message).context("encode MCP response")?;
    writer.write_all(b"\n").context("write MCP response")?;
    writer.flush().context("flush MCP response")?;
    Ok(())
}

struct Server {
    base_url: String,
    api: Api,
    tools: Vec<Tool>,
}

struct Tool {
    name: String,
    operation_index: usize,
}

impl Server {
    fn new(api: Api, base_url: &str) -> Result<Self> {
        let parsed = reqwest::Url::parse(base_url).context("invalid MCP API base URL")?;
        if !matches!(parsed.scheme(), "http" | "https") {
            bail!("MCP API base URL must use http or https")
        }
        let mut used = BTreeSet::new();
        let tools = api
            .operations
            .iter()
            .enumerate()
            .map(|(operation_index, operation)| {
                let mut name = tool_name(&operation.id);
                if !used.insert(name.clone()) {
                    name = format!("{}_{}", name, operation.method.as_str().to_lowercase());
                    let mut suffix = 2;
                    while !used.insert(name.clone()) {
                        name = format!(
                            "{}_{}_{suffix}",
                            tool_name(&operation.id),
                            operation.method.as_str().to_lowercase()
                        );
                        suffix += 1;
                    }
                }
                Tool {
                    name,
                    operation_index,
                }
            })
            .collect();
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            api,
            tools,
        })
    }

    fn handle(&self, request: Value) -> Option<Value> {
        let id = request.get("id").cloned();
        let Some(method) = request.get("method").and_then(Value::as_str) else {
            return id.map(|id| error_response(id, -32600, "JSON-RPC request requires method"));
        };
        // MCP notifications, including `notifications/initialized`, do not
        // receive a JSON-RPC response.
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": request.pointer("/params/protocolVersion").and_then(Value::as_str).unwrap_or("2025-03-26"),
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "kaji", "version": env!("CARGO_PKG_VERSION") },
            })),
            "tools/list" => Ok(json!({ "tools": self.tool_definitions() })),
            "tools/call" => self.call_tool(request.get("params").cloned().unwrap_or(Value::Null)),
            _ => Err(anyhow::anyhow!("method {method:?} is not supported")),
        };
        id.map(|id| match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(error) if method == "tools/call" => json!({
                "jsonrpc": "2.0", "id": id,
                "result": tool_error(&format!("{error:#}")),
            }),
            Err(error) => error_response(id, -32601, &format!("{error:#}")),
        })
    }

    fn tool_definitions(&self) -> Vec<Value> {
        self.tools
            .iter()
            .map(|tool| {
                let operation = &self.api.operations[tool.operation_index];
                json!({
                    "name": tool.name,
                    "description": operation_description(operation),
                    "inputSchema": input_schema(operation, &self.api),
                })
            })
            .collect()
    }

    fn call_tool(&self, params: Value) -> Result<Value> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .context("tools/call requires params.name")?;
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let tool = self
            .tools
            .iter()
            .find(|tool| tool.name == name)
            .with_context(|| format!("unknown tool {name:?}"))?;
        let operation = &self.api.operations[tool.operation_index];
        let request = prepare_request(operation, &self.base_url, &arguments)?;
        let client = reqwest::blocking::Client::new();
        let method = reqwest::Method::from_bytes(operation.method.as_str().as_bytes())
            .context("invalid generated HTTP method")?;
        let mut call = client.request(method, &request.url);
        for (name, value) in request.headers {
            call = call.header(name, value);
        }
        if let Some(body) = request.body {
            call = match body {
                RequestBody::Text(body) => call.body(body),
                RequestBody::Multipart(fields) => {
                    let form = fields.into_iter().fold(
                        reqwest::blocking::multipart::Form::new(),
                        |form, (name, value)| form.text(name, value),
                    );
                    call.multipart(form)
                }
            };
        }
        let mut response = call.send().context("call API")?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_owned();
        let mut bytes = Vec::new();
        response
            .by_ref()
            .take(MAX_RESPONSE_BYTES + 1)
            .read_to_end(&mut bytes)
            .context("read API response")?;
        let truncated = bytes.len() as u64 > MAX_RESPONSE_BYTES;
        if truncated {
            bytes.truncate(MAX_RESPONSE_BYTES as usize);
        }
        let text = response_text(&bytes, &content_type, truncated);
        let prefix = format!(
            "HTTP {} {}\n",
            status.as_u16(),
            status.canonical_reason().unwrap_or("")
        );
        Ok(json!({
            "content": [{ "type": "text", "text": format!("{prefix}{text}") }],
            "isError": !status.is_success(),
        }))
    }
}

struct PreparedRequest {
    url: String,
    headers: BTreeMap<String, String>,
    body: Option<RequestBody>,
}

enum RequestBody {
    Text(String),
    Multipart(Vec<(String, String)>),
}

fn prepare_request(
    operation: &Operation,
    base_url: &str,
    arguments: &Value,
) -> Result<PreparedRequest> {
    let arguments = arguments
        .as_object()
        .context("tools/call params.arguments must be an object")?;
    let path = argument_group(arguments, "path");
    let query = argument_group(arguments, "query");
    let supplied_headers = argument_group(arguments, "headers");
    let cookies = argument_group(arguments, "cookies");

    for parameter in &operation.parameters {
        if !parameter.required {
            continue;
        }
        let group = match parameter.location.as_str() {
            "path" => &path,
            "query" => &query,
            "header" => &supplied_headers,
            "cookie" => &cookies,
            _ => continue,
        };
        if !group.contains_key(&parameter.name) {
            bail!(
                "required {} parameter {:?} is missing",
                parameter.location,
                parameter.name
            )
        }
    }

    let mut route = operation.path.clone();
    for (name, value) in path {
        route = route.replace(&format!("{{{name}}}"), &percent_encode(&wire_value(&value)));
    }
    if route.contains('{') || route.contains('}') {
        bail!("not every path placeholder has a value")
    }
    if operation
        .request_body
        .as_ref()
        .is_some_and(|body| body.required)
        && !arguments.contains_key("body")
    {
        bail!("required request body is missing")
    }
    let query_pairs = query
        .into_iter()
        .flat_map(|(name, value)| values_for_query(name, value))
        .collect::<Vec<_>>();
    let mut url = format!("{}{}", base_url.trim_end_matches('/'), route);
    if !query_pairs.is_empty() {
        url.push('?');
        url.push_str(
            &query_pairs
                .iter()
                .map(|(name, value)| format!("{}={}", percent_encode(name), percent_encode(value)))
                .collect::<Vec<_>>()
                .join("&"),
        );
    }
    let mut headers = supplied_headers
        .into_iter()
        .map(|(name, value)| (name, wire_value(&value)))
        .collect::<BTreeMap<_, _>>();
    if !cookies.is_empty() {
        headers.insert(
            "cookie".into(),
            cookies
                .into_iter()
                .map(|(name, value)| format!("{name}={}", percent_encode(&wire_value(&value))))
                .collect::<Vec<_>>()
                .join("; "),
        );
    }
    let body = if let Some(body) = arguments.get("body") {
        let requested_content_type = arguments
            .get("contentType")
            .and_then(Value::as_str)
            .or_else(|| header_value(&headers, "content-type"))
            .or_else(|| {
                operation
                    .request_body
                    .as_ref()
                    .and_then(|body| body.media_types.first())
                    .map(|media| media.content_type.as_str())
            })
            .unwrap_or("application/json")
            .to_owned();
        if let Some(declared) = operation
            .request_body
            .as_ref()
            .map(|body| &body.media_types)
        {
            if !declared.is_empty()
                && !declared.iter().any(|media| {
                    media
                        .content_type
                        .eq_ignore_ascii_case(&requested_content_type)
                })
            {
                bail!("contentType {requested_content_type:?} is not declared for this operation")
            }
        }
        if !headers
            .keys()
            .any(|name| name.eq_ignore_ascii_case("content-type"))
        {
            headers.insert("content-type".into(), requested_content_type.clone());
        }
        Some(encode_body(body, &requested_content_type)?)
    } else {
        None
    };
    Ok(PreparedRequest { url, headers, body })
}

fn header_value<'a>(headers: &'a BTreeMap<String, String>, name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn encode_body(body: &Value, content_type: &str) -> Result<RequestBody> {
    if content_type.eq_ignore_ascii_case("application/x-www-form-urlencoded") {
        let fields = body
            .as_object()
            .context("urlencoded body must be an object")?;
        return Ok(RequestBody::Text(
            fields
                .iter()
                .flat_map(|(name, value)| values_for_query(name.clone(), value.clone()))
                .map(|(name, value)| {
                    format!("{}={}", percent_encode(&name), percent_encode(&value))
                })
                .collect::<Vec<_>>()
                .join("&"),
        ));
    }
    if content_type.eq_ignore_ascii_case("multipart/form-data") {
        let fields = body
            .as_object()
            .context("multipart body must be an object")?;
        return Ok(RequestBody::Multipart(
            fields
                .iter()
                .flat_map(|(name, value)| values_for_query(name.clone(), value.clone()))
                .collect(),
        ));
    }
    if content_type.contains("json") || content_type.ends_with("+json") {
        return Ok(RequestBody::Text(serde_json::to_string(body)?));
    }
    match body {
        Value::String(value) => Ok(RequestBody::Text(value.clone())),
        _ => bail!("non-JSON request body must be a string"),
    }
}

fn argument_group(arguments: &Map<String, Value>, name: &str) -> BTreeMap<String, Value> {
    arguments
        .get(name)
        .and_then(Value::as_object)
        .map(|values| {
            values
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn values_for_query(name: String, value: Value) -> Vec<(String, String)> {
    match value {
        Value::Array(values) => values
            .into_iter()
            .map(|value| (name.clone(), wire_value(&value)))
            .collect(),
        value => vec![(name, wire_value(&value))],
    }
}

fn wire_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        value => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(char::from(byte));
            }
            byte => write!(&mut encoded, "%{byte:02X}").expect("write to String cannot fail"),
        }
    }
    encoded
}

fn response_text(bytes: &[u8], content_type: &str, truncated: bool) -> String {
    let mut text = if content_type.contains("json") {
        serde_json::from_slice::<Value>(bytes)
            .and_then(|value| serde_json::to_string_pretty(&value))
            .unwrap_or_else(|_| String::from_utf8_lossy(bytes).into_owned())
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    if truncated {
        text.push_str("\n\n[Kaji truncated the response at 1 MiB]");
    }
    text
}

fn input_schema(operation: &Operation, api: &Api) -> Value {
    let mut properties = Map::new();
    for location in ["path", "query", "header", "cookie"] {
        let parameters = operation
            .parameters
            .iter()
            .filter(|parameter| parameter.location == location)
            .collect::<Vec<_>>();
        if parameters.is_empty() {
            continue;
        }
        let fields = parameters
            .iter()
            .map(|parameter| {
                let mut schema = parameter
                    .schema
                    .as_ref()
                    .map(|schema| schema_to_json_schema(schema, api, &mut BTreeSet::new()))
                    .unwrap_or_else(|| json!({}));
                if let Some(description) = &parameter.description {
                    schema["description"] = Value::String(description.clone());
                }
                (parameter.name.clone(), schema)
            })
            .collect::<Map<String, Value>>();
        let required = parameters
            .iter()
            .filter(|parameter| parameter.required)
            .map(|parameter| Value::String(parameter.name.clone()))
            .collect::<Vec<_>>();
        properties.insert(
            match location {
                "header" => "headers",
                "cookie" => "cookies",
                location => location,
            }
            .into(),
            json!({ "type": "object", "properties": fields, "required": required }),
        );
    }
    if let Some(request_body) = &operation.request_body {
        let variants = request_body
            .media_types
            .iter()
            .filter_map(|media| {
                media.schema.as_ref().map(|schema| {
                    let mut schema = schema_to_json_schema(schema, api, &mut BTreeSet::new());
                    schema["description"] =
                        Value::String(format!("{} request body", media.content_type));
                    schema
                })
            })
            .collect::<Vec<_>>();
        properties.insert(
            "body".into(),
            if variants.is_empty() {
                json!({ "description": "Request body" })
            } else if variants.len() == 1 {
                variants.into_iter().next().expect("one body schema")
            } else {
                json!({ "oneOf": variants })
            },
        );
        let content_types = request_body
            .media_types
            .iter()
            .map(|media| Value::String(media.content_type.clone()))
            .collect::<Vec<_>>();
        if !content_types.is_empty() {
            properties.insert(
                "contentType".into(),
                json!({ "type": "string", "enum": content_types, "description": "Media type for body" }),
            );
        }
    }
    json!({ "type": "object", "properties": properties })
}

fn schema_to_json_schema(
    schema: &SchemaValue,
    api: &Api,
    active_references: &mut BTreeSet<String>,
) -> Value {
    let mut result = match &schema.kind {
        SchemaKind::Any => json!({}),
        SchemaKind::Null => json!({ "type": "null" }),
        SchemaKind::Boolean => json!({ "type": "boolean" }),
        SchemaKind::Integer => json!({ "type": "integer" }),
        SchemaKind::Number => json!({ "type": "number" }),
        SchemaKind::String => json!({ "type": "string" }),
        SchemaKind::Array { items } => {
            json!({ "type": "array", "items": schema_to_json_schema(items, api, active_references) })
        }
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            let properties = fields
                .iter()
                .map(|field| {
                    (
                        field.name.clone(),
                        schema_to_json_schema(&field.value, api, active_references),
                    )
                })
                .collect::<Map<String, Value>>();
            let required = fields
                .iter()
                .filter(|field| field.required)
                .map(|field| Value::String(field.name.clone()))
                .collect::<Vec<_>>();
            let mut object = json!({ "type": "object", "properties": properties });
            if !required.is_empty() {
                object["required"] = Value::Array(required);
            }
            match additional_properties {
                kaji_core::AdditionalProperties::Forbidden => {
                    object["additionalProperties"] = Value::Bool(false)
                }
                kaji_core::AdditionalProperties::Any => {
                    object["additionalProperties"] = Value::Bool(true)
                }
                kaji_core::AdditionalProperties::Schema { value } => {
                    object["additionalProperties"] =
                        schema_to_json_schema(value, api, active_references)
                }
                kaji_core::AdditionalProperties::Unspecified => {}
            }
            object
        }
        SchemaKind::Reference { reference } => {
            let name = reference.rsplit('/').next().unwrap_or(reference);
            if !active_references.insert(name.into()) {
                json!({})
            } else {
                let resolved = api
                    .schemas
                    .iter()
                    .find(|candidate| candidate.name == name)
                    .map(|candidate| {
                        schema_to_json_schema(&candidate.value, api, active_references)
                    })
                    .unwrap_or_else(|| json!({}));
                active_references.remove(name);
                resolved
            }
        }
        SchemaKind::OneOf { variants } => {
            json!({ "oneOf": variants.iter().map(|variant| schema_to_json_schema(variant, api, active_references)).collect::<Vec<_>>() })
        }
        SchemaKind::AnyOf { variants } => {
            json!({ "anyOf": variants.iter().map(|variant| schema_to_json_schema(variant, api, active_references)).collect::<Vec<_>>() })
        }
        SchemaKind::AllOf { variants } => {
            json!({ "allOf": variants.iter().map(|variant| schema_to_json_schema(variant, api, active_references)).collect::<Vec<_>>() })
        }
        SchemaKind::Not { schema } => {
            json!({ "not": schema_to_json_schema(schema, api, active_references) })
        }
    };
    if let Some(format) = &schema.format {
        result["format"] = Value::String(format.clone());
    }
    if let Some(description) = &schema.description {
        result["description"] = Value::String(description.clone());
    }
    if !schema.enum_values.is_empty() {
        result["enum"] = Value::Array(schema.enum_values.clone());
    }
    if let Some(value) = &schema.const_value {
        result["const"] = value.clone();
    }
    for (key, value) in &schema.constraints {
        result[key] = value.clone();
    }
    if schema.nullable {
        result = json!({ "anyOf": [result, { "type": "null" }] });
    }
    result
}

fn operation_description(operation: &Operation) -> String {
    let summary = operation
        .annotations
        .get("summary")
        .and_then(Value::as_str)
        .or_else(|| {
            operation
                .annotations
                .get("description")
                .and_then(Value::as_str)
        });
    match summary {
        Some(summary) => format!(
            "{} {} — {summary}",
            operation.method.as_str(),
            operation.path
        ),
        None => format!("{} {}", operation.method.as_str(), operation.path),
    }
}

fn tool_name(id: &str) -> String {
    let name = id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    let name = name.trim_matches('_');
    if name.is_empty() {
        "operation".into()
    } else {
        name.into()
    }
}

fn tool_error(message: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": message }], "isError": true })
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaji_core::{
        HttpMethod, OperationMediaType, OperationParameter, OperationRequestBody, SchemaKind,
    };

    fn operation() -> Operation {
        Operation {
            id: "Get a pet".into(),
            method: HttpMethod::Get,
            path: "/pets/{petId}".into(),
            parameters: vec![
                OperationParameter {
                    name: "petId".into(),
                    location: "path".into(),
                    required: true,
                    schema: None,
                    description: None,
                    annotations: BTreeMap::new(),
                },
                OperationParameter {
                    name: "include".into(),
                    location: "query".into(),
                    required: false,
                    schema: None,
                    description: None,
                    annotations: BTreeMap::new(),
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn creates_safe_tool_names_and_input_schema() {
        let server = Server::new(
            Api {
                operations: vec![operation()],
                ..Default::default()
            },
            "https://api.example.test",
        )
        .unwrap();
        let tool = &server.tool_definitions()[0];
        assert_eq!(tool["name"], "get_a_pet");
        assert_eq!(
            tool["inputSchema"]["properties"]["path"]["required"],
            json!(["petId"])
        );
    }

    #[test]
    fn builds_encoded_url_from_grouped_arguments() {
        let request = prepare_request(
            &operation(),
            "https://api.example.test/",
            &json!({
                "path": { "petId": "a/b" },
                "query": { "include": ["owner", "toys"] },
            }),
        )
        .unwrap();
        assert_eq!(
            request.url,
            "https://api.example.test/pets/a%2Fb?include=owner&include=toys"
        );
    }

    #[test]
    fn emits_schema_aware_media_inputs_and_encodes_form_bodies() {
        let mut operation = operation();
        operation.request_body = Some(OperationRequestBody {
            required: true,
            description: None,
            media_types: vec![
                OperationMediaType {
                    content_type: "application/json".into(),
                    schema: Some(SchemaValue::new(SchemaKind::Object {
                        fields: vec![],
                        additional_properties: kaji_core::AdditionalProperties::Any,
                    })),
                },
                OperationMediaType {
                    content_type: "application/x-www-form-urlencoded".into(),
                    schema: Some(SchemaValue::new(SchemaKind::Object {
                        fields: vec![],
                        additional_properties: kaji_core::AdditionalProperties::Any,
                    })),
                },
            ],
        });
        let api = Api {
            operations: vec![operation.clone()],
            ..Default::default()
        };
        let schema = input_schema(&operation, &api);
        assert_eq!(
            schema["properties"]["contentType"]["enum"],
            json!(["application/json", "application/x-www-form-urlencoded"])
        );
        let request = prepare_request(
            &operation,
            "https://api.example.test",
            &json!({
                "path": { "petId": "pet_1" },
                "contentType": "application/x-www-form-urlencoded",
                "body": { "tag": ["friendly", "indoor"] },
            }),
        )
        .unwrap();
        assert_eq!(
            request.headers.get("content-type").map(String::as_str),
            Some("application/x-www-form-urlencoded")
        );
        let Some(RequestBody::Text(body)) = request.body else {
            panic!("expected urlencoded body")
        };
        assert_eq!(body, "tag=friendly&tag=indoor");
    }

    #[test]
    fn generator_mcp_exposes_local_generation_tools() {
        let server = GeneratorServer;
        let response = server
            .handle(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))
            .expect("response");
        let tools = response
            .pointer("/result/tools")
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "kaji_languages");
        assert_eq!(tools[1]["name"], "kaji_generate");
        assert_eq!(
            tools[1]["inputSchema"]["required"],
            json!(["source", "output", "languages"])
        );
        assert!(
            tools[1]["inputSchema"]["properties"]["languages"]["items"]["enum"]
                .as_array()
                .unwrap()
                .contains(&json!("csharp"))
        );
    }
}
