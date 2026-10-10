//! Config implementation for generated typescript-cli packages.
use super::*;

pub(super) fn render_config(
    command_name: &str,
    base_url: Option<&str>,
    oauth: Option<&OAuthConfig>,
    catalog: Option<&SecuritySchemeCatalog>,
) -> Result<Value> {
    let schemes = catalog
        .map(|catalog| {
            catalog
                .schemes
                .iter()
                .map(|scheme| {
                    let value = match &scheme.kind {
                        SecuritySchemeKind::ApiKey { name, location } => json!({
                            "type": "apiKey", "name": name, "location": location,
                        }),
                        SecuritySchemeKind::Http { scheme, .. } => {
                            json!({ "type": "http", "scheme": scheme })
                        }
                        SecuritySchemeKind::OAuth2 { flows, .. } => json!({
                            "type": "oauth2",
                            "flows": flows.iter().map(|flow| json!({
                                "type": flow.flow_type,
                                "authorizationUrl": flow.authorization_url,
                                "tokenUrl": flow.token_url,
                                "scopes": flow.scopes,
                            })).collect::<Vec<_>>(),
                        }),
                        SecuritySchemeKind::OpenIdConnect { discovery_url } => {
                            json!({ "type": "openIdConnect", "discoveryUrl": discovery_url })
                        }
                        SecuritySchemeKind::Other { type_name } => {
                            json!({ "type": "other", "name": type_name })
                        }
                    };
                    (scheme.name.clone(), value)
                })
                .collect::<serde_json::Map<String, Value>>()
        })
        .unwrap_or_default();
    let oauth = oauth.map(|oauth| {
        let inferred = oauth
            .security_scheme
            .as_ref()
            .and_then(|name| schemes.get(name))
            .and_then(|scheme| scheme.get("flows"))
            .and_then(Value::as_array)
            .and_then(|flows| flows.first());
        json!({
            "securityScheme": oauth.security_scheme,
            "clientId": oauth.client_id,
            "scopes": oauth.scopes,
            "preferredFlow": oauth.preferred_flow.as_deref().unwrap_or("device"),
            "authorizationUrl": oauth.authorization_url.as_deref().or_else(|| inferred.and_then(|flow| flow.get("authorizationUrl")).and_then(Value::as_str)),
            "deviceAuthorizationUrl": oauth.device_authorization_url,
            "tokenUrl": oauth.token_url.as_deref().or_else(|| inferred.and_then(|flow| flow.get("tokenUrl")).and_then(Value::as_str)),
            "redirectUri": oauth.redirect_uri.as_deref().unwrap_or("http://127.0.0.1:8765/callback"),
        })
    });
    if let Some(oauth) = &oauth {
        if oauth["clientId"].as_str().is_none_or(str::is_empty) {
            bail!("typescript-cli oauth.client_id cannot be empty");
        }
        if oauth["tokenUrl"].as_str().is_none_or(str::is_empty) {
            bail!(
                "typescript-cli OAuth requires token_url or an OAuth2 OpenAPI security scheme with tokenUrl"
            );
        }
    }
    Ok(json!({
        "commandName": command_name,
        "baseUrl": base_url,
        "baseUrlEnvironment": format!("{}_BASE_URL", env_name(command_name)),
        "tokenEnvironment": format!("{}_TOKEN", env_name(command_name)),
        "oauth": oauth,
        "schemes": schemes,
    }))
}
