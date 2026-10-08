use super::*;
use kaji_core::{
    Operation, OperationMediaType, OperationParameter, SecurityRequirement, SecuritySchemeCatalog,
    SecuritySchemeKind,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn id(value: &str) -> String {
    let hex = Sha256::digest(value.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    format!(
        "{}-{}-5{}-a{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[13..16],
        &hex[17..20],
        &hex[20..32]
    )
}
fn variable(value: &str) -> String {
    format!("credential_{}", id(value).replace('-', ""))
}
fn sensitive(name: &str) -> bool {
    let key = name.to_ascii_lowercase().replace(['-', '_'], "");
    [
        "password",
        "secret",
        "token",
        "apikey",
        "privatekey",
        "authorization",
        "cookie",
    ]
    .iter()
    .any(|word| key.contains(word))
}
fn diag(d: &mut Vec<Diagnostic>, op: &str, severity: &str, code: &str, message: impl Into<String>) {
    d.push(Diagnostic {
        operation: Some(op.into()),
        severity: severity.into(),
        code: code.into(),
        message: message.into(),
    });
}
fn scrub_names(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (name, child) in map {
                if sensitive(name) {
                    *child = Value::String("<redacted>".into());
                } else {
                    scrub_names(child)
                }
            }
        }
        Value::Array(items) => {
            for child in items {
                scrub_names(child)
            }
        }
        _ => {}
    }
}
fn scrub(
    api: &Api,
    schema: &SchemaValue,
    value: &mut Value,
    request: bool,
    depth: usize,
) -> Result<()> {
    if depth > 32 {
        bail!("example redaction depth exceeds 32");
    }
    if schema.write_only || schema.extensions.get("x-sensitive") == Some(&Value::Bool(true)) {
        *value = Value::String("<redacted>".into());
        return Ok(());
    }
    let resolved = resolve(api, schema)?;
    if !std::ptr::eq(schema, resolved) {
        return scrub(api, resolved, value, request, depth + 1);
    }
    match (&schema.kind, value) {
        (
            SchemaKind::Object {
                fields,
                additional_properties,
            },
            Value::Object(map),
        ) => {
            for field in fields {
                if request && field.value.read_only {
                    map.remove(&field.name);
                    continue;
                }
                if let Some(value) = map.get_mut(&field.name) {
                    scrub(api, &field.value, value, request, depth + 1)?;
                }
            }
            if let kaji_core::AdditionalProperties::Schema { value: schema } = additional_properties
            {
                for (key, value) in map {
                    if !fields.iter().any(|field| field.name == *key) {
                        scrub(api, schema, value, request, depth + 1)?;
                    }
                }
            }
        }
        (SchemaKind::Array { items }, Value::Array(values)) => {
            for value in values {
                scrub(api, items, value, request, depth + 1)?;
            }
        }
        (
            SchemaKind::OneOf { variants }
            | SchemaKind::AnyOf { variants }
            | SchemaKind::AllOf { variants },
            value,
        ) => {
            for schema in variants {
                scrub(api, schema, value, request, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn declared(
    operation: &Operation,
    key: &str,
    media: &str,
    status: Option<&str>,
) -> Result<Option<Value>> {
    for example in operation
        .annotations
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if example.get("content_type").and_then(Value::as_str) == Some(media)
            && status.is_none_or(|s| example.get("status").and_then(Value::as_str) == Some(s))
        {
            if let Some(text) = example.get("example_json").and_then(Value::as_str) {
                return Ok(Some(serde_json::from_str(text)?));
            }
        }
    }
    Ok(None)
}
fn sample(
    api: &Api,
    op: &Operation,
    media: &OperationMediaType,
    status: Option<&str>,
    examples: Option<&RequestExamples>,
    d: &mut Vec<Diagnostic>,
) -> Result<Value> {
    let custom = examples
        .and_then(|e| match status {
            None => e.requests.get(&(op.id.clone(), media.content_type.clone())),
            Some(s) => e
                .responses
                .get(&(op.id.clone(), s.into(), media.content_type.clone())),
        })
        .cloned();
    let mut value = if let Some(value) = custom {
        value
    } else if let Some(value) = declared(
        op,
        if status.is_some() {
            "kaji.docs.response_examples"
        } else {
            "kaji.docs.request_examples"
        },
        &media.content_type,
        status,
    )? {
        value
    } else if let Some(schema) = &media.schema {
        let report = kaji_core::samples::schema_samples(
            api,
            schema,
            kaji_core::samples::SampleOptions {
                max_samples: 1,
                ..Default::default()
            },
        );
        for message in report.diagnostics {
            diag(d, &op.id, "warning", "sample-bound", message);
        }
        match report.samples.first() {
            Some(sample) => sample.value.clone(),
            None => {
                diag(
                    d,
                    &op.id,
                    "error",
                    "missing-example",
                    "No bounded representative example; supply RequestExamples",
                );
                Value::Null
            }
        }
    } else {
        Value::Null
    };
    scrub_names(&mut value);
    if let Some(schema) = &media.schema {
        if let Err(error) = scrub(api, schema, &mut value, status.is_none(), 0) {
            diag(d, &op.id, "error", "redaction", error.to_string());
            value = Value::String("<redacted: unresolved schema>".into());
        }
    }
    Ok(value)
}
pub(crate) fn example_contract(api: &Api) -> Result<RequestExamples> {
    let mut contract = RequestExamples::default();
    let mut diagnostics = Vec::new();
    for op in &api.operations {
        for media in op
            .request_body
            .as_ref()
            .into_iter()
            .flat_map(|body| &body.media_types)
        {
            contract.requests.insert(
                (op.id.clone(), media.content_type.clone()),
                sample(api, op, media, None, None, &mut diagnostics)?,
            );
        }
        for response in &op.responses {
            for media in &response.media_types {
                contract.responses.insert(
                    (
                        op.id.clone(),
                        response.status.clone(),
                        media.content_type.clone(),
                    ),
                    sample(
                        api,
                        op,
                        media,
                        Some(&response.status),
                        None,
                        &mut diagnostics,
                    )?,
                );
            }
        }
    }
    // Provider must not hide unsupported examples from its consumer.
    if diagnostics.iter().any(|d| d.severity == "error") {
        bail!(
            "request examples unavailable: {}",
            diagnostics
                .iter()
                .map(|d| d.message.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    Ok(contract)
}
fn text(value: &Value) -> String {
    if let Value::String(text) = value {
        text.clone()
    } else {
        value.to_string()
    }
}
fn scalar(value: &Value) -> bool {
    !value.is_object() && !value.is_array()
}
fn parameter_value(
    api: &Api,
    op: &Operation,
    p: &OperationParameter,
    d: &mut Vec<Diagnostic>,
) -> Value {
    if sensitive(&p.name)
        || p.schema.as_ref().is_some_and(|s| {
            s.write_only || s.extensions.get("x-sensitive") == Some(&Value::Bool(true))
        })
    {
        return Value::String("<redacted>".into());
    }
    let mut value = p
        .annotations
        .get("example")
        .cloned()
        .or_else(|| p.schema.as_ref().and_then(|s| s.default.clone()))
        .unwrap_or_else(|| {
            p.schema
                .as_ref()
                .and_then(|schema| {
                    kaji_core::samples::schema_samples(
                        api,
                        schema,
                        kaji_core::samples::SampleOptions {
                            max_samples: 1,
                            ..Default::default()
                        },
                    )
                    .samples
                    .first()
                    .map(|s| s.value.clone())
                })
                .unwrap_or(json!("example"))
        });
    scrub_names(&mut value);
    if let Some(schema) = &p.schema {
        if scrub(api, schema, &mut value, true, 0).is_err() {
            diag(
                d,
                &op.id,
                "error",
                "parameter-redaction",
                format!("Cannot resolve parameter {} schema", p.name),
            );
            return json!("<redacted>");
        }
    }
    value
}
fn pairs(name: &str, value: &Value, style: &str, explode: bool) -> Option<Vec<(String, String)>> {
    match value {
        Value::Array(values) if values.iter().all(scalar) => {
            if style == "form" && explode {
                Some(values.iter().map(|v| (name.into(), text(v))).collect())
            } else {
                let delimiter = match style {
                    "form" | "simple" => ",",
                    "spaceDelimited" => " ",
                    "pipeDelimited" => "|",
                    _ => return None,
                };
                Some(vec![(
                    name.into(),
                    values.iter().map(text).collect::<Vec<_>>().join(delimiter),
                )])
            }
        }
        Value::Object(map) if map.values().all(scalar) => {
            if style == "deepObject" && explode {
                Some(
                    map.iter()
                        .map(|(key, value)| (format!("{name}[{key}]"), text(value)))
                        .collect(),
                )
            } else if style == "form" && explode {
                Some(
                    map.iter()
                        .map(|(key, value)| (key.clone(), text(value)))
                        .collect(),
                )
            } else if matches!(style, "form" | "simple") {
                Some(vec![(
                    name.into(),
                    map.iter()
                        .map(|(key, value)| {
                            if explode {
                                format!("{key}={}", text(value))
                            } else {
                                format!("{key},{}", text(value))
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(","),
                )])
            } else {
                None
            }
        }
        value if scalar(value) && matches!(style, "form" | "simple") => {
            Some(vec![(name.into(), text(value))])
        }
        _ => None,
    }
}
fn credential(vars: &mut BTreeMap<String, bool>, name: &str) -> String {
    let name = variable(name);
    vars.insert(name.clone(), true);
    format!("{{{{{name}}}}}")
}
// Security serialization updates independent wire/header/query/cookie and diagnostic sinks.
#[allow(clippy::too_many_arguments)]
fn security(
    op: &Operation,
    requirement: &SecurityRequirement,
    catalog: Option<&SecuritySchemeCatalog>,
    headers: &mut Vec<Value>,
    query: &mut Vec<Value>,
    cookies: &mut Vec<String>,
    vars: &mut BTreeMap<String, bool>,
    d: &mut Vec<Diagnostic>,
) -> Value {
    let mut auth = json!({"type":"noauth"});
    let mut authorization = false;
    for (name, scopes) in &requirement.schemes {
        let scheme =
            catalog.and_then(|catalog| catalog.schemes.iter().find(|scheme| scheme.name == *name));
        match scheme.map(|s| &s.kind) {
            Some(SecuritySchemeKind::ApiKey {
                name: Some(key),
                location: Some(location),
            }) => {
                let value = credential(vars, name);
                match location.as_str() {
                    "header" => headers.push(json!({"key":key,"value":value})),
                    "query" => query.push(json!({"key":key,"value":value})),
                    "cookie" => cookies.push(format!("{key}={value}")),
                    _ => diag(
                        d,
                        &op.id,
                        "error",
                        "api-key-location",
                        format!("Unsupported API key location {location}"),
                    ),
                }
            }
            Some(SecuritySchemeKind::Http {
                scheme: Some(kind), ..
            }) if kind.eq_ignore_ascii_case("basic") || kind.eq_ignore_ascii_case("bearer") => {
                if authorization {
                    diag(
                        d,
                        &op.id,
                        "error",
                        "authorization-and",
                        "Multiple Authorization schemes in an AND requirement cannot share one header",
                    );
                    continue;
                }
                authorization = true;
                auth = if kind.eq_ignore_ascii_case("basic") {
                    json!({"type":"basic","basic":[{"key":"username","value":credential(vars,&format!("{name}:username")),"type":"string"},{"key":"password","value":credential(vars,&format!("{name}:password")),"type":"string"}]})
                } else {
                    json!({"type":"bearer","bearer":[{"key":"token","value":credential(vars,name),"type":"string"}]})
                };
            }
            Some(SecuritySchemeKind::OAuth2 { .. } | SecuritySchemeKind::OpenIdConnect { .. }) => {
                if authorization {
                    diag(
                        d,
                        &op.id,
                        "error",
                        "authorization-and",
                        "OAuth plus another Authorization scheme cannot share one header",
                    );
                    continue;
                }
                authorization = true;
                auth = json!({"type":"bearer","bearer":[{"key":"token","value":credential(vars,name),"type":"string"}]});
                diag(
                    d,
                    &op.id,
                    "warning",
                    "oauth-manual",
                    format!(
                        "Acquire {name} bearer token separately; required scopes: {}. Discovery/login/refresh scripts are not generated.",
                        scopes.join(", ")
                    ),
                );
            }
            _ => diag(
                d,
                &op.id,
                "error",
                "security-scheme",
                format!("Missing or unsupported security scheme {name}"),
            ),
        }
    }
    auth
}
fn server(
    op: &Operation,
    settings: &Settings,
    defaults: &mut BTreeMap<String, String>,
    vars: &mut BTreeMap<String, bool>,
    d: &mut Vec<Diagnostic>,
) -> String {
    let mut url = settings.base_url.clone().unwrap_or_else(|| {
        op.annotations
            .get("kaji.openapi.servers")
            .and_then(Value::as_array)
            .and_then(|s| s.first())
            .and_then(|s| s.get("url"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .into()
    });
    if settings.base_url.is_none() {
        if let Some(servers) = op
            .annotations
            .get("kaji.openapi.servers")
            .and_then(Value::as_array)
        {
            if servers.len() > 1 {
                diag(
                    d,
                    &op.id,
                    "warning",
                    "server-choice",
                    "Selected first resolved server; override package base_url to choose another",
                );
            }
            if let Some(server) = servers.first() {
                for item in server
                    .get("variables")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if let Some(name) = item.get("name").and_then(Value::as_str) {
                        let replacement = if sensitive(name) {
                            credential(vars, &format!("server:{name}"))
                        } else {
                            item.get("default").map(text).unwrap_or_default()
                        };
                        url = url.replace(&format!("{{{name}}}"), &replacement);
                    }
                }
            }
        }
    }
    if url.is_empty() {
        diag(
            d,
            &op.id,
            "warning",
            "base-url",
            "No server available; fill base_url before sending requests",
        );
    }
    if url.contains('@') || url.contains('?') || url.contains('#') || url.contains('{') {
        diag(
            d,
            &op.id,
            "error",
            "unsafe-server",
            "Server contains credentials/query/fragment or unresolved variables; supply a plain base_url",
        );
        url.clear();
    }
    if !url.is_empty() && !url.starts_with("https://") && !url.starts_with("http://") {
        diag(
            d,
            &op.id,
            "error",
            "relative-server",
            "Relative server needs explicit absolute package base_url",
        );
        url.clear();
    }
    let key = if settings.base_url.is_some() || url.is_empty() {
        "base_url".into()
    } else {
        format!("base_url_{}", id(&url).replace('-', ""))
    };
    vars.insert(key.clone(), false);
    defaults.insert(key.clone(), url.trim_end_matches('/').into());
    format!("{{{{{key}}}}}")
}
fn body(
    api: &Api,
    op: &Operation,
    media: &OperationMediaType,
    examples: Option<&RequestExamples>,
    d: &mut Vec<Diagnostic>,
) -> Result<Value> {
    let value = sample(api, op, media, None, examples, d)?;
    let content = media.content_type.split(';').next().unwrap_or("");
    if op
        .annotations
        .get("kaji.request_body_encodings")
        .and_then(|v| v.get(&media.content_type))
        .and_then(Value::as_object)
        .is_some_and(|v| !v.is_empty())
    {
        diag(
            d,
            &op.id,
            "error",
            "form-encoding",
            "Per-part content types/headers and custom form encodings require manual mapping",
        );
    }
    if content == "application/json" || content.ends_with("+json") {
        return Ok(
            json!({"mode":"raw","raw":serde_json::to_string_pretty(&value)?,"options":{"raw":{"language":"json"}}}),
        );
    }
    if content == "application/x-www-form-urlencoded" || content == "multipart/form-data" {
        let schema = media.schema.as_ref().and_then(|s| resolve(api, s).ok());
        let fields = match schema.map(|s| &s.kind) {
            Some(SchemaKind::Object { fields, .. }) => Some(fields),
            _ => None,
        };
        if !value.is_object() {
            diag(
                d,
                &op.id,
                "error",
                "form-shape",
                "Form bodies require an object sample",
            );
            return Ok(
                json!({"mode":if content=="multipart/form-data"{"formdata"}else{"urlencoded"},"formdata":[],"urlencoded":[]}),
            );
        }
        let mut entries = Vec::new();
        for (name, value) in value.as_object().unwrap() {
            let field = fields.and_then(|fields| fields.iter().find(|f| f.name == *name));
            let optional = field.is_none_or(|f| !f.required);
            if field
                .and_then(|f| resolve(api, &f.value).ok())
                .is_some_and(|s| s.format.as_deref() == Some("binary"))
                && content == "multipart/form-data"
            {
                entries.push(json!({"key":name,"type":"file","src":"","disabled":optional,"description":"Select a local file before sending"}));
                continue;
            }
            if let Some(values) = pairs(name, value, "form", true) {
                for (key, value) in values {
                    entries
                        .push(json!({"key":key,"value":value,"type":"text","disabled":optional}));
                }
            } else {
                diag(
                    d,
                    &op.id,
                    "error",
                    "form-value",
                    format!("Nested form field {name} requires explicit encoding"),
                );
                entries.push(json!({"key":name,"value":"<incomplete nested form>","type":"text","disabled":true}));
            }
        }
        return Ok(if content == "multipart/form-data" {
            json!({"mode":"formdata","formdata":entries})
        } else {
            json!({"mode":"urlencoded","urlencoded":entries})
        });
    }
    if content == "application/octet-stream"
        || media
            .schema
            .as_ref()
            .and_then(|s| resolve(api, s).ok())
            .is_some_and(|s| s.format.as_deref() == Some("binary"))
    {
        return Ok(json!({"mode":"file","file":{"src":""}}));
    }
    if content.starts_with("text/") && value.is_string() {
        return Ok(json!({"mode":"raw","raw":value.as_str().unwrap()}));
    }
    diag(
        d,
        &op.id,
        "error",
        "request-media",
        format!(
            "Unsupported request media {}: supply a manual mapping",
            media.content_type
        ),
    );
    Ok(json!({"mode":"raw","raw":"<incomplete: unsupported media>"}))
}
pub(crate) fn collection(
    api: &Api,
    catalog: Option<&SecuritySchemeCatalog>,
    settings: &Settings,
    examples: Option<&RequestExamples>,
    group_by_tag: bool,
) -> Result<CollectionDocument> {
    let mut diagnostics = Vec::new();
    let mut variables = BTreeMap::new();
    let mut defaults = BTreeMap::new();
    let mut folders = BTreeMap::<String, Vec<Value>>::new();
    let mut ids = BTreeSet::new();
    let mut operations = api.operations.iter().collect::<Vec<_>>();
    operations.sort_by(|a, b| a.id.cmp(&b.id));
    let mut names = BTreeSet::new();
    for op in operations {
        if op.id.is_empty() || !names.insert(&op.id) {
            bail!(
                "Postman operations must have unique nonempty ids: {:?}",
                op.id
            );
        }
        let base = server(
            op,
            settings,
            &mut defaults,
            &mut variables,
            &mut diagnostics,
        );
        let media: Vec<Option<&OperationMediaType>> = match &op.request_body {
            Some(body) if !body.media_types.is_empty() => {
                body.media_types.iter().map(Some).collect()
            }
            Some(_) => {
                diag(
                    &mut diagnostics,
                    &op.id,
                    "error",
                    "request-media",
                    "Request body has no media representations",
                );
                vec![None]
            }
            None => vec![None],
        };
        let public = SecurityRequirement {
            schemes: BTreeMap::new(),
        };
        let alternatives = if op.security.is_empty() {
            vec![&public]
        } else {
            op.security.iter().collect::<Vec<_>>()
        };
        if media.len().saturating_mul(alternatives.len()) > 256 {
            bail!(
                "Postman operation {} exceeds 256 media/auth alternatives",
                op.id
            );
        }
        for representation in &media {
            for alternative in &alternatives {
                let start = diagnostics.len();
                let mut headers = Vec::new();
                let mut query = Vec::new();
                let mut url_variables = Vec::new();
                let mut cookies = Vec::new();
                let mut path = op.path.clone();
                for p in &op.parameters {
                    let value = parameter_value(api, op, p, &mut diagnostics);
                    let style = p
                        .annotations
                        .get("style")
                        .and_then(Value::as_str)
                        .unwrap_or(if p.location == "query" || p.location == "cookie" {
                            "form"
                        } else {
                            "simple"
                        });
                    let explode = p
                        .annotations
                        .get("explode")
                        .and_then(Value::as_bool)
                        .unwrap_or(style == "form" || style == "deepObject");
                    if p.annotations.get("allowReserved") == Some(&Value::Bool(true)) {
                        diag(
                            &mut diagnostics,
                            &op.id,
                            "error",
                            "allow-reserved",
                            format!(
                                "Parameter {} requires reserved-character URL handling; Postman automatic encoding is not equivalent",
                                p.name
                            ),
                        );
                    }
                    let serialized = pairs(&p.name, &value, style, explode).unwrap_or_else(|| {
                        diag(
                            &mut diagnostics,
                            &op.id,
                            "error",
                            "parameter-style",
                            format!(
                                "Unsupported {} parameter {} style {style} or nested shape",
                                p.location, p.name
                            ),
                        );
                        vec![(p.name.clone(), "<incomplete serialization>".into())]
                    });
                    match p.location.as_str() {
                        "query" => {
                            for (key, value) in serialized {
                                query.push(json!({"key":key,"value":value,"disabled":!p.required,"description":p.description}));
                            }
                        }
                        "header" => {
                            if serialized.len() != 1 {
                                diag(
                                    &mut diagnostics,
                                    &op.id,
                                    "error",
                                    "header-array",
                                    "Header serialization requires one value",
                                );
                            }
                            headers.push(json!({"key":p.name,"value":serialized.iter().map(|(_,v)|v.as_str()).collect::<Vec<_>>().join(","),"disabled":!p.required,"description":p.description}));
                        }
                        "path" => {
                            if !path.contains(&format!("{{{}}}", p.name)) {
                                diag(
                                    &mut diagnostics,
                                    &op.id,
                                    "error",
                                    "path-parameter",
                                    format!(
                                        "Path parameter {} has no matching placeholder",
                                        p.name
                                    ),
                                );
                            }
                            path =
                                path.replace(&format!("{{{}}}", p.name), &format!(":{}", p.name));
                            url_variables.push(json!({"key":p.name,"value":serialized.iter().map(|(_,v)|v.as_str()).collect::<Vec<_>>().join(","),"description":p.description}));
                        }
                        "cookie" => {
                            for (name, value) in serialized {
                                if p.required {
                                    cookies.push(format!("{name}={value}"));
                                } else {
                                    headers.push(json!({"key":"Cookie","value":format!("{name}={value}"),"disabled":true,"description":"Optional cookie; combine with other Cookie fields if enabled"}));
                                }
                            }
                        }
                        other => diag(
                            &mut diagnostics,
                            &op.id,
                            "error",
                            "parameter-location",
                            format!("Unsupported parameter location {other}"),
                        ),
                    }
                }
                if path.contains('{') || path.contains('}') {
                    diag(
                        &mut diagnostics,
                        &op.id,
                        "error",
                        "path-parameter",
                        "Unresolved path placeholder lacks a declared parameter",
                    );
                }
                let auth = security(
                    op,
                    alternative,
                    catalog,
                    &mut headers,
                    &mut query,
                    &mut cookies,
                    &mut variables,
                    &mut diagnostics,
                );
                if !cookies.is_empty() {
                    headers.push(json!({"key":"Cookie","value":cookies.join("; ")}));
                }
                if let Some(media) = representation {
                    if !media.content_type.starts_with("multipart/form-data") {
                        headers.push(json!({"key":"Content-Type","value":media.content_type}));
                    }
                }
                let mut description = op
                    .annotations
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                if op.annotations.get("deprecated") == Some(&Value::Bool(true)) {
                    description.push_str("\nDEPRECATED operation.");
                }
                let mut seen_headers = std::collections::BTreeSet::new();
                for header in &headers {
                    if header["disabled"] == true {
                        continue;
                    }
                    let key = header["key"].as_str().unwrap_or("").to_ascii_lowercase();
                    if !seen_headers.insert(key.clone())
                        || (key == "authorization" && auth["type"] != "noauth")
                    {
                        diag(
                            &mut diagnostics,
                            &op.id,
                            "error",
                            "header-collision",
                            format!(
                                "Multiple active values/helpers target header {key}; choose an explicit request strategy"
                            ),
                        );
                    }
                }
                let suffix = query
                    .iter()
                    .filter(|p| p["disabled"] != true)
                    .map(|p| {
                        format!(
                            "{}={}",
                            url_component(p["key"].as_str().unwrap_or("")),
                            url_component(p["value"].as_str().unwrap_or(""))
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("&");
                let raw = if suffix.is_empty() {
                    format!("{base}{path}")
                } else {
                    format!("{base}{path}?{suffix}")
                };
                let mut request = json!({"method":op.method.as_str(),"header":headers,"auth":auth,"url":{"raw":raw,"host":[base],"path":path.trim_start_matches('/').split('/').collect::<Vec<_>>(),"query":query,"variable":url_variables},"description":description});
                if let Some(media) = representation {
                    request["body"] = body(api, op, media, examples, &mut diagnostics)?;
                    if op.request_body.as_ref().is_some_and(|body| !body.required) {
                        request["body"]["disabled"] = json!(true);
                    }
                }
                let mut responses = Vec::new();
                for response in &op.responses {
                    let representations: Vec<Option<&OperationMediaType>> =
                        if response.media_types.is_empty() {
                            vec![None]
                        } else {
                            response.media_types.iter().map(Some).collect()
                        };
                    for media in representations {
                        let mut response_body = String::new();
                        let mut response_headers = Vec::new();
                        if let Some(media) = media {
                            response_headers
                                .push(json!({"key":"Content-Type","value":media.content_type}));
                            let content = media.content_type.split(';').next().unwrap_or("");
                            if content == "text/event-stream"
                                || content == "application/octet-stream"
                                || media
                                    .schema
                                    .as_ref()
                                    .and_then(|s| resolve(api, s).ok())
                                    .is_some_and(|s| s.format.as_deref() == Some("binary"))
                            {
                                diag(
                                    &mut diagnostics,
                                    &op.id,
                                    "warning",
                                    "response-stream",
                                    "Streaming/binary saved responses are placeholders, not execution assertions",
                                );
                                response_body =
                                    "<stream or binary content: inspect live response separately>"
                                        .into();
                            } else {
                                let value = sample(
                                    api,
                                    op,
                                    media,
                                    Some(&response.status),
                                    examples,
                                    &mut diagnostics,
                                )?;
                                response_body = if content == "application/json"
                                    || content.ends_with("+json")
                                {
                                    serde_json::to_string_pretty(&value)?
                                } else if value.is_string() {
                                    text(&value)
                                } else {
                                    diag(
                                        &mut diagnostics,
                                        &op.id,
                                        "error",
                                        "response-media",
                                        format!(
                                            "Unsupported structured {} response",
                                            media.content_type
                                        ),
                                    );
                                    "<incomplete response representation>".into()
                                };
                            }
                        }
                        let mut saved = json!({"name":format!("{} {}",response.status,media.map(|m|m.content_type.as_str()).unwrap_or("empty")),"originalRequest":request,"status":response.description.as_deref().unwrap_or(&response.status),"header":response_headers,"body":response_body});
                        if let Ok(code) = response.status.parse::<u16>() {
                            if (100..=599).contains(&code) {
                                saved["code"] = json!(code);
                            } else {
                                diag(
                                    &mut diagnostics,
                                    &op.id,
                                    "error",
                                    "response-status",
                                    "HTTP status outside 100..599",
                                );
                            }
                        } else {
                            diag(
                                &mut diagnostics,
                                &op.id,
                                "warning",
                                "response-status",
                                format!(
                                    "Symbolic response {} retained as a named example without an invented numeric code",
                                    response.status
                                ),
                            );
                        }
                        responses.push(saved);
                    }
                }
                for key in ["kaji.docs.request_examples", "kaji.docs.response_examples"] {
                    if op
                        .annotations
                        .get(key)
                        .and_then(Value::as_array)
                        .is_some_and(|a| a.len() > 1)
                    {
                        diag(
                            &mut diagnostics,
                            &op.id,
                            "warning",
                            "example-choice",
                            format!(
                                "{key}: first example per media/status selected; provide RequestExamples for a chosen fixture"
                            ),
                        );
                    }
                }
                let notes = diagnostics[start..]
                    .iter()
                    .map(|d| format!("{} [{}]: {}", d.severity, d.code, d.message))
                    .collect::<Vec<_>>()
                    .join("\n");
                if !notes.is_empty() {
                    let old = request["description"].as_str().unwrap_or("");
                    request["description"] =
                        json!(format!("{old}\nKaji mapping diagnostics:\n{notes}"));
                }
                let key = format!(
                    "{}:{}:{}",
                    op.id,
                    representation
                        .map(|m| m.content_type.as_str())
                        .unwrap_or(""),
                    serde_json::to_string(alternative)?
                );
                let item_id = id(&key);
                if !ids.insert(item_id.clone()) {
                    bail!("Duplicate Postman request identity for {}", op.id);
                }
                let label = if media.len() == 1 && alternatives.len() == 1 {
                    op.id.clone()
                } else {
                    format!(
                        "{} [{}; {}]",
                        op.id,
                        representation
                            .map(|m| m.content_type.as_str())
                            .unwrap_or("no body"),
                        if alternative.schemes.is_empty() {
                            "public".into()
                        } else {
                            alternative
                                .schemes
                                .keys()
                                .cloned()
                                .collect::<Vec<_>>()
                                .join(" + ")
                        }
                    )
                };
                let group = if group_by_tag {
                    op.annotations
                        .get("tags")
                        .and_then(Value::as_array)
                        .and_then(|a| a.first())
                        .and_then(Value::as_str)
                        .unwrap_or("API")
                        .to_owned()
                } else {
                    op.path
                        .trim_start_matches('/')
                        .split('/')
                        .find(|s| !s.starts_with('{') && !s.is_empty())
                        .unwrap_or("API")
                        .to_owned()
                };
                folders.entry(group).or_default().push(
                    json!({"id":item_id,"name":label,"request":request,"response":responses}),
                );
            }
        }
    }
    diagnostics.sort_by(|a, b| {
        (&a.operation, &a.code, &a.message).cmp(&(&b.operation, &b.code, &b.message))
    });
    diagnostics.dedup();
    let document = json!({"info":{"_postman_id":id(&format!("collection:{}",api.name)),"name":settings.name.as_deref().unwrap_or(&api.name),"schema":"https://schema.getpostman.com/json/collection/v2.1.0/collection.json","description":"Generated by Kaji. Review diagnostics.json; structural examples are bounded fixtures, not complete schema validation. Credentials remain blank."},"item":folders.into_iter().map(|(name,item)|json!({"name":name,"item":item})).collect::<Vec<_>>(),"variable":variables.iter().map(|(name,secret)|json!({"key":name,"value":if *secret {""}else{defaults.get(name).map(String::as_str).unwrap_or("")},"type":"string"})).collect::<Vec<_>>()});
    Ok(CollectionDocument {
        document,
        diagnostics,
        variables,
    })
}

fn url_component(value: &str) -> String {
    // Preserve generated Postman variable references; encode literal wire values.
    if value.starts_with("{{") && value.ends_with("}}") {
        return value.into();
    }
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

/// Split already scrubbed generated folders, retaining shared portable variables.
/// Numeric filenames avoid path traversal and case-folding collisions in tag names.
pub(crate) fn split_collections(document: &Value) -> Result<Vec<(String, Value)>> {
    let folders = document
        .get("item")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("Postman collection items must be an array"))?;
    folders
        .iter()
        .enumerate()
        .map(|(index, folder)| {
            let name = folder
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("Postman generated folder has no name"))?;
            let mut split = document.clone();
            let parent = document["info"]["name"].as_str().unwrap_or("API");
            split["info"]["name"] = json!(format!("{parent} — {name}"));
            split["info"]["_postman_id"] = json!(id(&format!(
                "split:{}:{name}",
                document["info"]["_postman_id"].as_str().unwrap_or(parent)
            )));
            split["item"] = json!([folder]);
            Ok((format!("collections/group-{index:04}.json"), split))
        })
        .collect()
}
