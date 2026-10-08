//! Msw auxiliary rendering.
use super::*;

/// Emits MSW v2 handlers for every declared operation.
#[derive(Default)]
pub struct TypeScriptMsw;

impl TypeScriptMsw {
    pub fn generate(&self, api: &Api, config: &ArtifactOptions) -> Result<Vec<GeneratedFile>> {
        let prepared = crate::auxiliary_layout::prepare(api);
        let api = &prepared;
        let mut output = format!(
            "{NOTICE}\nimport {{ http, HttpResponse, delay }} from 'msw';\n\nexport const handlers = [\n"
        );
        for operation in &api.operations {
            let _ = writeln!(
                output,
                "  http.all({}, async ({{ request, params }}) => {{",
                js_string(&openapi_path_to_msw(&operation.path))
            );
            let _ = writeln!(
                output,
                "    if (request.method !== {}) return undefined;",
                js_string(operation.method.as_str())
            );
            let scenarios = kaji_core::extract_operation_mock_scenarios(operation)?;
            if !scenarios.is_empty() {
                output.push_str("    const url = new URL(request.url);\n    const selected = request.headers.get('x-kaji-mock-scenario');\n");
                if scenarios
                    .iter()
                    .any(|scenario| scenario.when.body.is_some())
                {
                    output.push_str(
                        "    const body = await request.clone().json().catch(() => undefined);\n",
                    );
                    output.push_str("    const canonical = (value: unknown): string => JSON.stringify(value, (_key, item) => item && typeof item === 'object' && !Array.isArray(item) ? Object.fromEntries(Object.entries(item).sort(([a], [b]) => a.localeCompare(b))) : item);\n");
                }
                for scenario in scenarios {
                    let mut conditions = Vec::new();
                    for (name, value) in scenario.when.headers {
                        conditions.push(format!(
                            "request.headers.get({}) === {}",
                            js_string(&name),
                            js_string(&value)
                        ));
                    }
                    for (name, value) in scenario.when.query {
                        conditions.push(format!(
                            "url.searchParams.get({}) === {}",
                            js_string(&name),
                            js_string(&value)
                        ));
                    }
                    for (name, value) in scenario.when.path {
                        conditions.push(format!(
                            "String(params[{}]) === {}",
                            js_string(&name),
                            js_string(&value)
                        ));
                    }
                    if let Some(body) = scenario.when.body {
                        conditions.push(format!("canonical(body) === canonical({body})"));
                    }
                    let predicate = if conditions.is_empty() {
                        "true".into()
                    } else {
                        conditions.join(" && ")
                    };
                    let _ = writeln!(
                        output,
                        "    if (selected === {} || (selected === null && ({predicate}))) {{",
                        js_string(&scenario.name)
                    );
                    if let Some(ms) = scenario.response.delay_ms {
                        let _ = writeln!(output, "      await delay({ms});");
                    }
                    let response = response_expression(
                        operation.method.as_str(),
                        scenario.response.status,
                        &scenario.response.headers,
                        scenario.response.body.as_ref(),
                        None,
                    )?;
                    let _ = writeln!(output, "      return {response};\n    }}");
                }
                output.push_str("    if (selected !== null) return new HttpResponse('Unknown mock scenario', { status: 400 });\n");
            }
            let response = operation
                .responses
                .iter()
                .find(|response| response.status.starts_with('2'));
            let status = response
                .and_then(|response| response.status.parse::<u16>().ok())
                .unwrap_or(200);
            let media = response.and_then(|response| response.media_types.first());
            let sample = media
                .and_then(|media| media.schema.as_ref())
                .and_then(|schema| {
                    kaji_core::samples::schema_samples(
                        api,
                        schema,
                        kaji_core::samples::SampleOptions::default(),
                    )
                    .samples
                    .into_iter()
                    .next()
                })
                .map(|sample| sample.value);
            let expression = response_expression(
                operation.method.as_str(),
                status,
                &Default::default(),
                sample.as_ref(),
                media.map(|media| media.content_type.as_str()),
            )?;
            let _ = writeln!(output, "    return {expression};\n  }}),");
        }
        output.push_str("];\n");
        anyhow::ensure!(config.max_file_bytes > 0, "max_file_bytes must be positive");
        if crate::auxiliary_layout::uses_modules(api, config, output.len(), true)? {
            return crate::auxiliary_layout::msw(api, config);
        }
        Ok(vec![GeneratedFile::new(
            extra_output_path(config, "typescript", "msw.ts"),
            output,
        )?])
    }
}

fn openapi_path_to_msw(path: &str) -> String {
    let mut output = String::new();
    for character in path.chars() {
        match character {
            '{' => output.push(':'),
            '}' => {}
            _ => output.push(character),
        }
    }
    output
}

fn response_expression(
    method: &str,
    status: u16,
    headers: &std::collections::BTreeMap<String, String>,
    body: Option<&serde_json::Value>,
    media: Option<&str>,
) -> Result<String> {
    anyhow::ensure!(
        (200..=599).contains(&status),
        "MSW Fetch responses require a status from 200 to 599, got {status}"
    );
    let mut headers = headers.clone();
    let content_type = media.unwrap_or("application/json");
    if body.is_some()
        && !headers
            .keys()
            .any(|key| key.eq_ignore_ascii_case("content-type"))
    {
        headers.insert("content-type".into(), content_type.into());
    }
    let body = if method == "HEAD" || matches!(status, 204 | 205 | 304) {
        "null".into()
    } else {
        body.map(|body| {
            let wire = if content_type.contains("json") {
                body.to_string()
            } else {
                body.as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| body.to_string())
            };
            js_string(&wire)
        })
        .unwrap_or_else(|| "null".into())
    };
    Ok(format!(
        "new HttpResponse({body}, {{ status: {status}, headers: {} }})",
        serde_json::to_string(&headers)?
    ))
}
