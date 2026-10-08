//! Local OpenAPI contract mock server and request log.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, bail};
use poolster_core::Api;
use serde::Serialize;

use super::mock_http::{
    mock_path_matches, mock_path_parameters, mock_path_specificity, mock_response_body,
    mock_target_parts, scenario_content_type, write_mock_response,
};
use super::{MockServe, compiler_path};

#[derive(Clone, Serialize)]
struct MockRequest {
    id: u64,
    method: String,
    path: String,
    operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scenario: Option<String>,
    status: u16,
    body: Option<String>,
}

#[derive(Default)]
struct MockRequests {
    next_id: u64,
    entries: Vec<MockRequest>,
}

pub(super) fn serve_mock(options: MockServe) -> Result<()> {
    let source = std::fs::canonicalize(&options.source)
        .with_context(|| format!("cannot read OpenAPI source {}", options.source.display()))?;
    if !source.is_file() {
        bail!("OpenAPI source must be a file")
    }
    let temporary = tempfile::tempdir().context("cannot create compiler working directory")?;
    let helper = compiler_path(options.compiler)?;
    let status = Command::new(&helper)
        .arg("--out")
        .arg(temporary.path())
        .arg(&source)
        .status()
        .with_context(|| format!("cannot start OpenAPI compiler {}", helper.display()))?;
    if !status.success() {
        bail!("OpenAPI compiler failed ({status})")
    }
    let api = Arc::new(poolster_core::adapter::openapi_sidecar::load_operations(
        temporary.path(),
        "API".into(),
        "0.1.0".into(),
    )?);
    // Fail before listening when a scenario is malformed. Falling back to a
    // happy-path response would hide a broken test contract.
    poolster_core::extract_mock_scenarios(&api)?;
    let listener = TcpListener::bind(("127.0.0.1", options.port))
        .with_context(|| format!("cannot listen on http://127.0.0.1:{}", options.port))?;
    let requests = Arc::new(Mutex::new(MockRequests::default()));
    println!("Poolster dynamic mock: http://127.0.0.1:{}", options.port);
    println!(
        "Request log: http://127.0.0.1:{}/_poolster/requests",
        options.port
    );
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_mock_request(stream, &api, &requests) {
                    eprintln!("poolster mock: {error:#}");
                }
            }
            Err(error) => eprintln!("poolster mock: accept connection: {error}"),
        }
    }
    Ok(())
}

fn handle_mock_request(
    mut stream: TcpStream,
    api: &Api,
    requests: &Arc<Mutex<MockRequests>>,
) -> Result<()> {
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .context("configure mock request timeout")?;
    let mut reader = BufReader::new(&mut stream);
    let mut start = String::new();
    if reader.read_line(&mut start)? == 0 {
        return Ok(());
    }
    let mut parts = start.split_whitespace();
    let method = parts
        .next()
        .context("invalid HTTP request method")?
        .to_owned();
    let target = parts
        .next()
        .context("invalid HTTP request target")?
        .to_owned();
    let mut content_length = 0usize;
    let mut headers = BTreeMap::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        if line == "\r\n" || line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim().to_owned();
            let value = value.trim().to_owned();
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.parse().unwrap_or(0);
            }
            headers.insert(name, value);
        }
    }
    let mut bytes = vec![0; content_length.min(1024 * 1024)];
    reader.read_exact(&mut bytes)?;
    drop(reader);
    let (path, query) = mock_target_parts(&target);

    if method == "GET" && matches!(path, "/_poolster/requests" | "/_kaji/requests") {
        let entries = requests
            .lock()
            .expect("mock request log poisoned")
            .entries
            .clone();
        return write_mock_response(
            &mut stream,
            200,
            "application/json",
            &serde_json::to_vec(&entries)?,
            &BTreeMap::new(),
        );
    }
    if method == "GET" && matches!(path, "/_poolster/health" | "/_kaji/health") {
        return write_mock_response(
            &mut stream,
            200,
            "application/json",
            br#"{"ok":true}"#,
            &BTreeMap::new(),
        );
    }
    if method == "OPTIONS" {
        return write_mock_response(&mut stream, 204, "text/plain", b"", &BTreeMap::new());
    }

    // OpenAPI permits a literal route alongside a parameterized sibling, for
    // example `/users/me` and `/users/{id}`. Prefer the most literal match so
    // the result does not accidentally depend on declaration order.
    let operation = api
        .operations
        .iter()
        .filter(|operation| {
            operation.method.as_str().eq_ignore_ascii_case(&method)
                && mock_path_matches(&operation.path, path)
        })
        .max_by_key(|operation| mock_path_specificity(&operation.path));
    let request_id = next_mock_id(requests);
    let path_parameters = operation
        .map(|operation| mock_path_parameters(&operation.path, path))
        .unwrap_or_default();
    let json_body = serde_json::from_slice(&bytes).ok();
    let scenario = operation
        .map(poolster_core::extract_operation_mock_scenarios)
        .transpose()?
        .and_then(|scenarios| {
            scenarios.into_iter().find(|scenario| {
                poolster_core::mock_scenario_matches(
                    scenario,
                    &headers,
                    &query,
                    &path_parameters,
                    json_body.as_ref(),
                )
            })
        });
    let scenario_name = scenario.as_ref().map(|scenario| scenario.name.clone());
    let (status, content_type, response, response_headers) = match (operation, scenario) {
        (_, Some(scenario)) => {
            if let Some(delay_ms) = scenario.response.delay_ms {
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            }
            let content_type = scenario_content_type(&scenario.response.headers)
                .unwrap_or_else(|| "application/json".into());
            (
                scenario.response.status,
                content_type,
                scenario.response.body,
                scenario.response.headers,
            )
        }
        (Some(operation), None) => {
            let (status, content_type, response) =
                poolster_core::httpmock::mock_dynamic_response(api, operation, request_id);
            (
                status,
                content_type.unwrap_or_else(|| "application/json".into()),
                Some(response),
                BTreeMap::new(),
            )
        }
        (None, None) => (
            404,
            "application/json".into(),
            Some(serde_json::json!({ "error": "No OpenAPI operation matches this request" })),
            BTreeMap::new(),
        ),
    };
    let response = mock_response_body(&content_type, response)?;
    let body =
        (!bytes.is_empty()).then(|| String::from_utf8_lossy(&bytes).chars().take(4096).collect());
    let entry = MockRequest {
        id: request_id,
        method,
        path: target,
        operation: operation.map(|operation| operation.id.clone()),
        scenario: scenario_name,
        status,
        body,
    };
    let mut log = requests.lock().expect("mock request log poisoned");
    log.entries.push(entry);
    if log.entries.len() > 200 {
        log.entries.remove(0);
    }
    write_mock_response(
        &mut stream,
        status,
        &content_type,
        &response,
        &response_headers,
    )
}

fn next_mock_id(requests: &Arc<Mutex<MockRequests>>) -> u64 {
    let mut log = requests.lock().expect("mock request log poisoned");
    log.next_id += 1;
    log.next_id
}
