//! Extensions implementation for generated rust-cli packages.
use super::*;

pub(super) fn readme(api: &Api, command: &str) -> String {
    format!(
        "# {command}\n\nGenerated Rust CLI for {}.\n\n```sh\ncargo install --path .\n{command} --help\n{command} auth set-token \"$API_TOKEN\"\n```\n\nCommands are grouped from paths: `/admin/users` with `listUsers` becomes `{command} admin users list`. `src/poolster_extension.rs` is preserved on regeneration for login, authentication, request, and response hooks. Return `AuthenticationResult::Handled` after adding custom OAuth, SSO, keychain, or signing credentials; return `UseOpenApi` to apply the declared OpenAPI security scheme.\n",
        api.name
    )
}
pub(super) fn extension_template() -> &'static str {
    r#"// This file is yours. Poolster preserves it on regeneration.
use anyhow::Result;
use reqwest::header::HeaderMap;

#[allow(dead_code)] // User extensions opt into the fields they need.
pub struct AuthContext<'a> { pub profile: &'a str, pub command: &'a [&'a str], pub operation_id: &'a str }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthenticationResult { Handled, UseOpenApi }
pub trait Extension {
    fn login(&self, _context: &AuthContext<'_>) -> Result<Option<String>> { Ok(None) }
    fn authenticate(&self, _headers: &mut HeaderMap, _query: &mut Vec<(&'static str, String)>, _context: &AuthContext<'_>) -> Result<AuthenticationResult> { Ok(AuthenticationResult::UseOpenApi) }
    fn before_request(&self, _headers: &mut HeaderMap, _query: &mut Vec<(&str, String)>, _context: &AuthContext<'_>) -> Result<()> { Ok(()) }
    fn after_response(&self, _status: u16, _body: &str, _context: &AuthContext<'_>) -> Result<()> { Ok(()) }
}
#[derive(Default)] pub struct PoolsterExtension;
impl Extension for PoolsterExtension {}
"#
}
