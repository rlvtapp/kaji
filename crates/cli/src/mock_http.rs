//! HTTP routing and response helpers for the local contract mock.

use std::collections::BTreeMap;
use std::io::Write;
use std::net::TcpStream;

use anyhow::Result;

pub(super) fn mock_path_matches(template: &str, path: &str) -> bool {
    template
        .split('/')
        .zip(path.split('/'))
        .all(|(template, actual)| {
            if template.starts_with('{') && template.ends_with('}') {
                !actual.is_empty()
            } else {
                template == actual
            }
        })
        && template.split('/').count() == path.split('/').count()
}

pub(super) fn mock_path_specificity(template: &str) -> usize {
    template
        .split('/')
        .filter(|segment| {
            !(segment.is_empty() || segment.starts_with('{') && segment.ends_with('}'))
        })
        .count()
}

pub(super) fn mock_target_parts(target: &str) -> (&str, BTreeMap<String, String>) {
    let (path, raw_query) = target.split_once('?').unwrap_or((target, ""));
    (path, mock_query_parameters(raw_query))
}

fn mock_query_parameters(raw_query: &str) -> BTreeMap<String, String> {
    raw_query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            (mock_percent_decode(name), mock_percent_decode(value))
        })
        .collect()
}

fn mock_percent_decode(value: &str) -> String {
    let mut decoded = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => decoded.push(b' '),
            b'%' if index + 2 < bytes.len() => {
                let hex = |value: u8| match value {
                    b'0'..=b'9' => Some(value - b'0'),
                    b'a'..=b'f' => Some(value - b'a' + 10),
                    b'A'..=b'F' => Some(value - b'A' + 10),
                    _ => None,
                };
                if let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2])) {
                    decoded.push(high * 16 + low);
                    index += 2;
                } else {
                    decoded.push(bytes[index]);
                }
            }
            byte => decoded.push(byte),
        }
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

pub(super) fn mock_path_parameters(template: &str, path: &str) -> BTreeMap<String, String> {
    template
        .split('/')
        .zip(path.split('/'))
        .filter_map(|(template, value)| {
            template
                .strip_prefix('{')
                .and_then(|name| name.strip_suffix('}'))
                .map(|name| (name.to_owned(), mock_percent_decode(value)))
        })
        .collect()
}

pub(super) fn scenario_content_type(headers: &BTreeMap<String, String>) -> Option<String> {
    headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
        .map(|(_, value)| value.clone())
}

pub(super) fn mock_response_body(
    content_type: &str,
    body: Option<serde_json::Value>,
) -> Result<Vec<u8>> {
    let Some(body) = body else {
        return Ok(Vec::new());
    };
    if content_type.to_ascii_lowercase().contains("json") {
        return Ok(serde_json::to_vec(&body)?);
    }
    if let Some(text) = body.as_str() {
        return Ok(text.as_bytes().to_vec());
    }
    Ok(serde_json::to_vec(&body)?)
}

pub(super) fn write_mock_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
    headers: &BTreeMap<String, String>,
) -> Result<()> {
    let mut extra_headers = String::new();
    for (name, value) in headers {
        if !name.eq_ignore_ascii_case("content-type") {
            use std::fmt::Write as _;
            writeln!(extra_headers, "{name}: {value}\r")?;
        }
    }
    write!(
        stream,
        "HTTP/1.1 {status} {}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n{extra_headers}Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: content-type, authorization\r\nConnection: close\r\n\r\n",
        mock_status_text(status),
        body.len(),
    )?;
    stream.write_all(body)?;
    Ok(())
}

fn mock_status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Mock Response",
    }
}
