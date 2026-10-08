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

Kaji emits factual OpenAPI-derived command references at
`references/<command-group>.md`, containing operations, parameters, and simple
request-body fields. It deliberately does not generate `SKILL.md`: a real agent skill
needs provider-authored workflow, safety, and product guidance.

## Authentication and customization

`auth set-token` stores a local bearer token and `<COMMAND>_TOKEN` supplies an ephemeral
token in CI. Kaji creates `src/kaji_extension.rs` and `src/kaji_auth.rs` only once, so
both are preserved when the CLI is regenerated. The latter's `ExtensionV1` contract is
versioned.

### Implement custom authentication

Implement `pre_authenticate` to add custom OAuth, SSO, keychain, or signing credentials,
then return `AuthenticationResult::Handled`; return `AuthenticationResult::Fallback` to
retain Kaji's environment/profile token lookup. `before_request` and `after_response` in
`kaji_extension.rs` are available for logging, tracing, and audit events.

Existing `authenticate` implementations returning an optional bearer token remain
supported. The generated `auth login` command delegates to `login`.

```rust
impl ExtensionV1 for KajiAuthExtension {
    fn pre_authenticate(
        &self,
        auth: &mut AuthenticateContext<'_, '_, '_>,
    ) -> anyhow::Result<AuthenticationResult> {
        auth.bearer_token(&read_enterprise_token()?)?;
        Ok(AuthenticationResult::Handled)
    }
}
```

Returning `Fallback` is useful when an extension only adds context such as a tenant
header and should still use the configured token.

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
