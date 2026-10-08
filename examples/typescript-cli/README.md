# TypeScript and Rust API CLI example

This example generates a Node.js CLI from an OAuth-protected Notes API. It
shows operation flags, profiles, OAuth device login, and the PKCE browser-login
fallback. The `example.test` URLs are deliberately placeholders: replace them
with a registered OAuth client and API origin before using it against a real
service.

```sh
cd examples/typescript-cli
npx poolster generate
cd generated/notes
npm install
npm run build
node dist/index.js --help
node dist/index.js messages send --help
```

The generated CLI contains `notes auth login`, `notes auth status`,
`notes auth profiles`, `notes auth use`, and `notes auth logout`. Device login
is the configured default; use `--flow browser` to exercise Authorization Code
with PKCE after registering the loopback callback in your OAuth provider.

```sh
# Interactive OAuth, using the configured default profile.
notes auth login
notes auth use work
notes auth profiles

# API-token workflow for automation. This does not write the token to disk.
NOTES_TOKEN="$CI_TOKEN" notes get --note-id note_123 --json

# Simple object bodies become flags. Array fields repeat; content fields may
# read from a file.
notes messages send \
  --from hello@example.test \
  --to first@example.test --to second@example.test \
  --subject "Welcome" \
  --html-file ./welcome.html \
  --tags onboarding --tags transactional

# Use a different API origin or credential profile.
notes get --note-id note_123 --base-url https://api.acme.test/v1 --profile work

# Namespaces are inferred from literal OpenAPI path segments.
notes admin users list
```

`poolster.json` owns the public OAuth client ID and endpoint overrides. The
OpenAPI `authorizationCode` flow supplies the authorization and token URLs;
the device authorization URL is configured separately because standard
OpenAPI does not model it.

## Rust CLI

```sh
cd generated/notes-rust
cargo run -- messages send --from hello@example.test --to recipient@example.test --subject "Welcome" --html-file ./welcome.html
```

The Rust CLI preserves `src/kaji_extension.rs` on regeneration. Implement its
`login` hook for provider-specific OAuth or SSO; `notes auth login` calls it.

## Terminal and reference behavior in 0.4.0

In a terminal, commands prompt for missing required inputs and mask credential
entry. Pipes and --json remain non-interactive; supply required flags in CI.
Inspect generated references/<command-group>.md for OpenAPI-derived command
parameters and body fields. These references are not generated agent skills.
