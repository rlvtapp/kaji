# Rust API CLI

| Task | Section |
| --- | --- |
| Run interactively or in CI | [Terminal modes](#terminal-experience) |
| Find command documentation | [Generated references](#generated-references) |
| Add custom authentication | [Extensions](#authentication-and-customization) |
| Configure API keys | [Profiles](#api-keys-and-profiles) |

Use the `rust-cli` target when an API needs a single native executable rather than a
Node.js runtime. It derives nested commands from OpenAPI paths: an operation `listUsers`
on `/admin/users` becomes `admin users list`.

```json
{
  "language": "rust-cli",
  "path": "relevate-email",
  "name": "relevate-email-cli",
  "plugins": [{
    "name": "cli",
    "command_name": "relevate-email",
    "base_url": "https://api.relevate.email/v1"
  }]
}
```

Build the generated project with `cargo install --path .`. Query and path parameters
become flags; simple object request bodies become body flags, with `--data` and
`--data-file` available for arbitrary JSON.

## Terminal experience

The generated executable is interactive only when both standard input and output are
terminals. It prompts for missing required parameters, simple body fields, and a missing
base URL; `auth set-token` and `auth set-key` accept a masked credential prompt when no
positional credential is given. Completed requests use concise status styling.

Pipes, CI, and `--json` stay non-interactive: every value must be supplied as a flag or
environment value, successful responses are emitted verbatim for machine consumption,
and no prompt can block an agent. For nested request bodies, terminal mode requests a
single `--data` JSON value rather than trying to infer a form.

## Generated references

Poolster emits factual OpenAPI-derived command references at
`references/<command-group>.md`, containing operations, parameters, and simple
request-body fields. It deliberately does not generate `SKILL.md`: a real agent skill
needs provider-authored workflow, safety, and product guidance.

## Authentication and customization

`auth set-token` stores a local bearer token and `<COMMAND>_TOKEN` supplies an ephemeral
token in CI. Poolster creates `src/poolster_extension.rs` once and preserves it
when the CLI is regenerated.

### Implement custom authentication

Implement `Extension::authenticate` to add custom OAuth, SSO, keychain, or signing
credentials, then return `AuthenticationResult::Handled`. Return
`AuthenticationResult::UseOpenApi` to apply the declared OpenAPI security using
the configured environment or profile credentials. `before_request` and
`after_response` are available for logging, tracing, and audit events.

The generated `auth login` command delegates to `login`. Replace the generated
empty `impl Extension for PoolsterExtension` with your implementation:

```rust
impl Extension for PoolsterExtension {
    fn authenticate(
        &self,
        headers: &mut HeaderMap,
        _query: &mut Vec<(&'static str, String)>,
        _context: &AuthContext<'_>,
    ) -> anyhow::Result<AuthenticationResult> {
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", read_enterprise_token()?).parse()?,
        );
        Ok(AuthenticationResult::Handled)
    }
}
```

Returning `UseOpenApi` is useful when an extension only adds context such as a
tenant header and should still use the configured security scheme.

## API keys and profiles

`apiKey` security schemes are read from OpenAPI and placed in their declared header,
query, or cookie location. `auth set-key` infers the scheme when the API declares
exactly one API key; use `--scheme <OpenAPI scheme>` only for an ambiguous API. Then
select the profile you want to use:

```sh
acme auth set-key "$ACME_KEY" --profile production
acme auth use production
acme auth profiles
ACME_APIKEY_TOKEN="$ACME_KEY" acme projects list
```

The CLI also accepts `<COMMAND>_TOKEN` as a generic CI fallback. `auth use` selects a
local default profile; `--profile <name>` takes precedence for a single command.
