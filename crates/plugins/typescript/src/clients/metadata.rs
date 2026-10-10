use super::*;

/// Preserves explicit OpenAPI parameter serialization metadata for Poolster's
/// runtime. Unspecified style/explode settings intentionally remain absent so
/// the runtime can apply each location's OpenAPI defaults.
pub(crate) fn style_property_name(name: &str) -> String {
    let mut chars = name.chars();
    let valid = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
    if valid {
        name.to_owned()
    } else {
        serde_json::to_string(name).expect("string serialization")
    }
}

pub(crate) fn render_parameter_styles(operation: &Operation) -> Option<String> {
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
                    let content_type = poolster_core::openapi32::parameter_content(parameter)
                        .ok()
                        .and_then(|content| {
                            content.first().map(|media| media.content_type.clone())
                        });
                    (style.is_some() || explode.is_some() || content_type.is_some()).then(|| {
                        let mut fields = Vec::new();
                        if let Some(content_type) = &content_type {
                            fields.push(format!(
                                "contentType: {}",
                                serde_json::to_string(content_type).expect("media type")
                            ));
                        }
                        if let Some(style) = style {
                            fields.push(format!("style: '{style}'"));
                        }
                        if let Some(explode) = explode {
                            fields.push(format!("explode: {explode}"));
                        }
                        format!(
                            "{}: {{ {} }}",
                            style_property_name(&parameter.name),
                            fields.join(", ")
                        )
                    })
                })
                .collect::<Vec<_>>();
            (!parameters.is_empty()).then(|| format!("{location}: {{ {} }}", parameters.join(", ")))
        })
        .collect::<Vec<_>>();
    (!groups.is_empty()).then(|| format!("{{ {} }}", groups.join(", ")))
}

/// Carries OpenAPI's multipart/urlencoded Encoding Object to the transport.
/// The compiler stores it as an annotation so non-TypeScript targets do not
/// need a JavaScript-shaped form-data type in the shared AST.
pub(crate) fn render_form_encodings(operation: &Operation) -> Option<String> {
    let mut encodings = operation
        .annotations
        .get("poolster.request_body_encodings")?
        .clone();
    if let Some(media_types) = encodings.as_object_mut() {
        for fields in media_types.values_mut().filter_map(Value::as_object_mut) {
            for encoding in fields.values_mut().filter_map(Value::as_object_mut) {
                for key in ["style", "explode", "contentType", "allowReserved"] {
                    if encoding.get(key).is_some_and(Value::is_null) {
                        encoding.remove(key);
                    }
                }
            }
        }
    }
    serde_json::to_string(&encodings).ok()
}

/// Converts declared OpenAPI OR-of-AND requirements without guessing scheme kinds.
pub(crate) fn render_security(
    operation: &Operation,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> Option<String> {
    (!operation.security.is_empty()).then(|| {
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
    })
}

/// Emits runtime-addressable descriptors for SDK packages. `id` is the
/// OpenAPI component key, so one client can carry several independently named
/// API keys, bearer tokens, and OAuth credentials without guessing headers.
pub(crate) fn render_catalog_security_scheme(
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

pub(crate) fn render_security_scheme(
    scheme_name: &str,
    catalog: Option<&SecuritySchemeCatalog>,
) -> String {
    let scheme = catalog
        .and_then(|catalog| {
            catalog
                .schemes
                .iter()
                .find(|scheme| scheme.name == scheme_name)
        })
        .expect("security catalog validated before rendering");

    match &scheme.kind {
        SecuritySchemeKind::ApiKey { name, location } => {
            let name = name.as_deref().expect("validated API key name");
            let location = location.as_deref().expect("validated API key location");
            format!("{{ type: 'apiKey', name: '{name}', in: '{location}' }}")
        }
        SecuritySchemeKind::Http { scheme, .. } => {
            let scheme = scheme.as_deref().expect("validated HTTP scheme");
            format!("{{ type: 'http', scheme: '{scheme}' }}")
        }
        SecuritySchemeKind::OAuth2 { .. } | SecuritySchemeKind::OpenIdConnect { .. } => {
            "{ type: 'oauth2' }".to_owned()
        }
        SecuritySchemeKind::Other { .. } => {
            unreachable!("unsupported security scheme rejected before rendering")
        }
    }
}
