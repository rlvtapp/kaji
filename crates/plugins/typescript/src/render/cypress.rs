//! Cypress auxiliary rendering.
use super::*;

/// Emits a Cypress smoke suite. It deliberately exercises request routing, not
/// business assertions, so generated tests stay useful before users add data.
#[derive(Default)]
pub struct TypeScriptCypress;

impl TypeScriptCypress {
    pub fn generate(&self, api: &Api, config: &ArtifactOptions) -> Result<Vec<GeneratedFile>> {
        let prepared = crate::auxiliary_layout::prepare(api);
        let api = &prepared;
        let options = &config.cypress_options;
        anyhow::ensure!(
            options.timeout_ms > 0,
            "Cypress timeout_ms must be positive"
        );
        for id in options.operation_overrides.keys() {
            anyhow::ensure!(
                api.operations.iter().any(|operation| operation
                    .annotations
                    .get("poolster.aux.operation_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(&operation.id)
                    == id),
                "Cypress override refers to unknown operation {id}"
            );
        }
        let mut output = format!(
            "{NOTICE}\n/// <reference types=\"cypress\" />\nexport {{}};\n\nconst baseUrl = Cypress.env('API_BASE_URL') ?? {};\n\ndescribe({}, () => {{\n",
            js_string(
                options
                    .base_url
                    .as_deref()
                    .unwrap_or("http://localhost:3000")
            ),
            js_string(&format!("{} API", api.name))
        );
        for operation in &api.operations {
            let local = options
                .operation_overrides
                .get(
                    operation
                        .annotations
                        .get("poolster.aux.operation_id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or(&operation.id),
                )
                .cloned()
                .unwrap_or_default();
            let enabled = local.enabled.unwrap_or(
                options.include_mutations
                    || matches!(
                        operation.method,
                        poolster_core::HttpMethod::Get
                            | poolster_core::HttpMethod::Head
                            | poolster_core::HttpMethod::Options
                    ),
            );
            let name = js_string(&format!(
                "calls {} {}",
                operation.method.as_str(),
                operation.path
            ));
            if !enabled {
                let _ = writeln!(
                    output,
                    "  it.skip({name}, () => {{}}); // Enable this operation explicitly for a disposable test API."
                );
                continue;
            }
            let mut path = local.path.clone().unwrap_or_else(|| operation.path.clone());
            let mut query = local.query.clone();
            let mut headers = options.headers.clone();
            headers.extend(local.headers.clone());
            let mut unresolved = Vec::new();
            for parameter in operation
                .parameters
                .iter()
                .filter(|parameter| parameter.required || parameter.location == "path")
            {
                if parameter.location == "query" && query.contains_key(&parameter.name)
                    || parameter.location == "header"
                        && headers
                            .keys()
                            .any(|name| name.eq_ignore_ascii_case(&parameter.name))
                {
                    continue;
                }
                if parameter.location == "path" && local.path.is_some() {
                    continue;
                }
                let sample = parameter
                    .schema
                    .as_ref()
                    .and_then(|schema| {
                        poolster_core::samples::schema_samples(
                            api,
                            schema,
                            poolster_core::samples::SampleOptions::default(),
                        )
                        .samples
                        .into_iter()
                        .next()
                    })
                    .map(|sample| sample.value);
                if let Some(value) = sample {
                    let text = value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string());
                    match parameter.location.as_str() {
                        "path" => {
                            path = path
                                .replace(&format!("{{{}}}", parameter.name), &encode_path(&text))
                        }
                        "query" => {
                            query.insert(parameter.name.clone(), value);
                        }
                        "header" => {
                            headers.insert(parameter.name.clone(), text);
                        }
                        _ => unresolved.push(parameter.name.clone()),
                    }
                } else {
                    unresolved.push(parameter.name.clone());
                }
            }
            let body = local.body.clone().or_else(|| {
                operation
                    .request_schema()
                    .and_then(|schema| {
                        poolster_core::samples::schema_samples(
                            api,
                            schema,
                            poolster_core::samples::SampleOptions::default(),
                        )
                        .samples
                        .into_iter()
                        .next()
                    })
                    .map(|sample| sample.value)
            });
            if operation
                .request_body
                .as_ref()
                .is_some_and(|body| body.required)
                && body.is_none()
            {
                unresolved.push("request body".into());
            }
            if !unresolved.is_empty() {
                let _ = writeln!(
                    output,
                    "  it.skip({name}, () => {{}}); // Supply operation overrides for {}",
                    unresolved.join(", ")
                );
                continue;
            }
            let statuses = if local.expected_statuses.is_empty() {
                operation
                    .responses
                    .iter()
                    .filter_map(|response| response.status.parse::<u16>().ok())
                    .filter(|status| (200..300).contains(status))
                    .collect::<Vec<_>>()
            } else {
                local.expected_statuses.clone()
            };
            anyhow::ensure!(
                statuses.iter().all(|status| (100..=599).contains(status)),
                "Invalid Cypress expected status for {}",
                operation.id
            );
            let _ = writeln!(output, "  it({name}, () => {{");
            let body = body
                .map(|body| format!(", body: {body}"))
                .unwrap_or_default();
            let _ = writeln!(
                output,
                "    cy.request({{ method: {}, url: baseUrl + {}, qs: {}, headers: {{ ...{}, ...(Cypress.env('API_HEADERS') ?? {{}}) }}, timeout: {}, failOnStatusCode: false{body} }}).its('status').should({});",
                js_string(operation.method.as_str()),
                js_string(&path),
                serde_json::to_string(&query)?,
                serde_json::to_string(&headers)?,
                options.timeout_ms,
                if statuses.is_empty() {
                    "'be.within', 200, 299".into()
                } else {
                    format!("'be.oneOf', {}", serde_json::to_string(&statuses)?)
                }
            );
            output.push_str("  });\n");
        }
        output.push_str("});\n");
        anyhow::ensure!(config.max_file_bytes > 0, "max_file_bytes must be positive");
        if crate::auxiliary_layout::uses_modules(api, config, output.len(), true)? {
            return crate::auxiliary_layout::cypress(api, config);
        }
        Ok(vec![GeneratedFile::new(
            extra_output_path(config, "cypress/e2e", "api.cy.ts"),
            output,
        )?])
    }
}

fn encode_path(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|&byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}
