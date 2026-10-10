//! HTTP client runtime rendering.
use super::*;

pub(crate) fn render_client_files(api: &Api, config: &RenderOptions) -> Result<Vec<GeneratedFile>> {
    let prepared = prepare_api(api);
    let api = &prepared;
    let mut files = Vec::new();
    files.push(GeneratedFile::new(
        "src/client/mod.rs",
        render_client_runtime(api, resource_client_enabled(config)),
    )?);
    files.extend(render_operation_files(api, config)?);
    if resource_client_enabled(config) {
        files.extend(render_resource_files(api, config)?);
    }
    append_source_layout_diagnostics(
        &mut files,
        ".poolster/source-layout-operation-diagnostics.json",
    )?;
    Ok(files)
}

pub(crate) fn render_client_runtime(api: &Api, include_resources: bool) -> String {
    let has_pagination = api
        .operations
        .iter()
        .any(|operation| rust_pagination(operation).is_some());
    let mut output = format!(
        "{NOTICE}\nuse reqwest::Method;\nuse serde::Serialize;\n#[allow(unused_imports)]\nuse crate::models::*;\n\n/// The raw HTTP response retained when a response cannot be decoded or is not declared by the OpenAPI document.\n#[derive(Debug)]\npub struct ApiResponse {{\n    pub status: reqwest::StatusCode,\n    pub headers: reqwest::header::HeaderMap,\n    pub body: Vec<u8>,\n}}\n\nimpl ApiResponse {{\n    pub fn text(&self) -> String {{ String::from_utf8_lossy(&self.body).into_owned() }}\n\n    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {{\n        serde_json::from_slice(&self.body)\n    }}\n}}\n\n#[derive(Clone)]\npub struct Client {{\n    base_url: String,\n    http: reqwest::Client,\n    bearer_token: Option<String>,\n}}\n\n#[allow(dead_code)]\nfn poolster_query_value<T: Serialize>(value: &T) -> String {{\n    match serde_json::to_value(value).unwrap_or(serde_json::Value::Null) {{\n        serde_json::Value::String(value) => value,\n        serde_json::Value::Number(value) => value.to_string(),\n        serde_json::Value::Bool(value) => value.to_string(),\n        value => value.to_string(),\n    }}\n}}\n\nfn poolster_path_segment(value: &str) -> String {{\n    let mut encoded = String::new();\n    for byte in value.bytes() {{\n        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {{\n            encoded.push(byte as char);\n        }} else {{\n            use std::fmt::Write as _;\n            let _ = write!(encoded, \"%{{byte:02X}}\");\n        }}\n    }}\n    encoded\n}}\n\n"
    );
    // Keep the base renderer compact while augmenting its generated runtime
    // with configuration that applies uniformly to every operation.
    output = output.replacen(
        "use reqwest::Method;\n",
        "use std::{sync::Arc, time::Duration};\n\nuse reqwest::Method;\n",
        1,
    );
    output = output.replacen(
        "\nfn poolster_path_segment",
        "\n#[allow(dead_code)] // Only operations with path parameters call this helper.\nfn poolster_path_segment",
        1,
    );
    output = output.replacen(
        "    bearer_token: Option<String>,\n",
        "    bearer_token: Option<String>,\n    retry: RetryConfig,\n    hooks: Option<Arc<dyn ClientHooks>>,\n    token_provider: Option<Arc<dyn BearerTokenProvider>>,\n    call_options: CallOptions,\n",
        1,
    );
    output = output.replacen(
        "\nfn poolster_query_value",
        "\n/// Retry policy applied to safe generated requests. The default makes three attempts.\n#[derive(Clone, Debug)]\npub struct RetryConfig {\n    pub max_attempts: usize,\n    pub initial_delay: Duration,\n    pub max_delay: Duration,\n}\n\nimpl Default for RetryConfig {\n    fn default() -> Self {\n        Self { max_attempts: 3, initial_delay: Duration::from_millis(250), max_delay: Duration::from_secs(8) }\n    }\n}\n\n/// Metadata passed to lifecycle hooks. Request headers and bodies are omitted to avoid exposing credentials.\n#[derive(Clone, Debug)]\npub struct RequestInfo {\n    pub method: Method,\n    pub url: String,\n}\n\n/// The final HTTP response observed by lifecycle hooks.\n#[derive(Clone, Debug)]\npub struct ResponseInfo {\n    pub request: RequestInfo,\n    pub status: reqwest::StatusCode,\n    pub headers: reqwest::header::HeaderMap,\n}\n\n/// Optional package-level observability and policy callbacks. Hooks run once for the final outcome, not every retry attempt.\npub trait ClientHooks: Send + Sync {\n    fn before_request(&self, _request: &RequestInfo) {}\n    fn after_response(&self, _response: &ResponseInfo) {}\n    fn on_error(&self, _request: &RequestInfo, _error: &str) {}\n}\n\nfn poolster_query_value",
        1,
    );
    output.push_str(r#"
#[allow(dead_code)]
fn poolster_retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    if let Some(delay) = headers.get("retry-after-ms").and_then(|value| value.to_str().ok()).and_then(|value| value.trim().parse::<f64>().ok()).and_then(|ms| Duration::try_from_secs_f64(ms / 1000.0).ok()) {
        return Some(delay);
    }
    let value = headers.get(reqwest::header::RETRY_AFTER)?.to_str().ok()?.trim();
    if let Ok(seconds) = value.parse::<u64>() { return Some(Duration::from_secs(seconds)); }
    httpdate::parse_http_date(value).ok().map(|time| time.duration_since(std::time::SystemTime::now()).unwrap_or(Duration::ZERO))
}
"#);
    output.push_str(r#"
#[allow(dead_code)]
fn poolster_query_pairs<T: Serialize>(name: &str, value: &T, style: &str, explode: bool) -> Vec<(String, String)> {
    let value = serde_json::to_value(value).unwrap_or(serde_json::Value::Null);
    match value {
        serde_json::Value::Array(values) => {
            if style == "form" && explode { values.iter().map(|value| (name.to_owned(), poolster_query_value(value))).collect() }
            else {
                let delimiter = match style { "spaceDelimited" => " ", "pipeDelimited" => "|", _ => "," };
                vec![(name.to_owned(), values.iter().map(poolster_query_value).collect::<Vec<_>>().join(delimiter))]
            }
        }
        value => vec![(name.to_owned(), poolster_query_value(&value))],
    }
}
"#);
    if has_pagination {
        output.push_str(render_pagination_runtime());
    }
    output.push_str(include_str!("../templates/token_provider_contract.rs.tmpl"));
    output.push_str(include_str!("../templates/multipart_runtime.rs.tmpl"));
    output.push_str(include_str!("../templates/call_options_runtime.rs.tmpl"));
    output.push_str(include_str!("../templates/sequential_json.rs.tmpl"));
    output.push_str(
        "impl Client {\n    pub fn new(base_url: impl Into<String>) -> Self {\n        Self { base_url: base_url.into().trim_end_matches('/').to_owned(), http: reqwest::Client::new(), bearer_token: None }\n    }\n\n    /// Configures a bearer token for operations that declare OpenAPI security.\n    /// Other credential kinds are intentionally not guessed by this generated client.\n    pub fn with_bearer_token(mut self, token: impl Into<String>) -> Self {\n        self.bearer_token = Some(token.into());\n        self\n    }\n\n",
    );
    output = output.replacen(
        "Self { base_url: base_url.into().trim_end_matches('/').to_owned(), http: reqwest::Client::new(), bearer_token: None }",
        "Self { base_url: base_url.into().trim_end_matches('/').to_owned(), http: reqwest::Client::new(), bearer_token: None, retry: RetryConfig::default(), hooks: None, token_provider: None, call_options: CallOptions::default() }",
        1,
    );
    output = output.replacen(
        "    /// Configures a bearer token for operations that declare OpenAPI security.\n",
        "    /// Replaces the conservative default retry policy. Set `max_attempts` to one to disable retries.\n    pub fn with_retry(mut self, retry: RetryConfig) -> Self {\n        self.retry = retry;\n        self\n    }\n\n    /// Adds package-level lifecycle hooks without changing generated operation signatures.\n    pub fn with_hooks(mut self, hooks: Arc<dyn ClientHooks>) -> Self {\n        self.hooks = Some(hooks);\n        self\n    }\n\n    fn poolster_before_request(&self, request: &RequestInfo) {\n        if let Some(hooks) = &self.hooks { hooks.before_request(request); }\n    }\n\n    fn poolster_after_response(&self, request: &RequestInfo, response: &reqwest::Response) {\n        if let Some(hooks) = &self.hooks {\n            hooks.after_response(&ResponseInfo { request: request.clone(), status: response.status(), headers: response.headers().clone() });\n        }\n    }\n\n    fn poolster_on_error(&self, request: &RequestInfo, error: &str) {\n        if let Some(hooks) = &self.hooks { hooks.on_error(request, error); }\n    }\n\n    fn poolster_retry_delay(&self, completed_attempts: usize, retry_after: Option<Duration>) -> Duration {\n        if let Some(retry_after) = retry_after { return retry_after.min(self.retry.max_delay); }\n        let factor = 1_u32 << completed_attempts.saturating_sub(1).min(16);\n        self.retry.initial_delay.saturating_mul(factor).min(self.retry.max_delay)\n    }\n\n    /// Configures a bearer token for operations that declare OpenAPI security.\n",
        1,
    );
    output.push_str(include_str!("../templates/token_provider_runtime.rs.tmpl"));
    output.push_str(include_str!("../templates/call_options_methods.rs.tmpl"));
    output.push_str("    /// Substitute the request executor without changing operation signatures.\n    pub fn with_transport(mut self, transport: Arc<dyn crate::transport::Transport>) -> Self { self.transport = transport; self }\n}\n");
    output = output.replace(
        "    http: reqwest::Client,",
        "    http: reqwest::Client,\n    transport: Arc<dyn crate::transport::Transport>,",
    );
    output = output.replace("http: reqwest::Client::new(), bearer_token:", "http: reqwest::Client::new(), transport: Arc::new(crate::transport::DefaultTransport::default()), bearer_token:");
    output.push_str("\nmod operations;\npub use operations::*;\n");
    if include_resources {
        output.push_str("mod resources;\npub use resources::*;\n");
    }
    output.replace(
        "\nfn poolster_query_value",
        "\n#[allow(dead_code)]\nfn poolster_query_value",
    )
}
