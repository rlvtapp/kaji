# TypeScript API CLI

| Task | Section |
| --- | --- |
| Script commands without prompts | [Terminal modes](#terminal-experience) |
| Override command groups | [Generated references](#generated-references) |
| Set API keys and profiles | [Profiles](#api-keys-and-profiles) |
| Add product-specific behavior | [Extensions](#extensions) |
| Configure OAuth or tokens | [Authentication](#authentication) |

The `typescript-cli` package turns OpenAPI operations into a publishable Node.js
command-line client. It generates one command per operation ID, named in kebab case,
plus `auth login`, `auth set-token`, `auth status`, and `auth logout`.

Use config mode so the API origin and any product-specific OAuth information are
explicit and reviewable:

```json
{
  "openapi": { "input": "./openapi.yaml", "name": "Acme API", "version": "1.0.0" },
  "output": { "path": "./generated" },
  "packages": [{
    "language": "typescript-cli",
    "path": "cli",
    "name": "@acme/cli",
    "plugins": [{
      "name": "cli",
      "command_name": "acme",
      "base_url": "https://api.acme.test/v1",
      "oauth": {
        "security_scheme": "OAuth",
        "client_id": "acme-cli",
        "preferred_flow": "device",
        "device_authorization_url": "https://auth.acme.test/oauth/device/code",
        "scopes": ["projects:read", "projects:write"],
        "redirect_uri": "http://127.0.0.1:8765/callback"
      }
    }]
  }]
}
```

`authorization_url` and `token_url` are read from the selected OpenAPI OAuth2 security
scheme when available. Set either field in the `oauth` block to override that source. A
device authorization URL is not part of ordinary OpenAPI OAuth metadata, so specify
`device_authorization_url` for device login.

After generation, install and build the generated package:

```sh
cd generated/cli
npm install
npm run build
npm link
acme auth login
acme list-projects --json
```

Every operation accepts `--profile`, `--base-url`, and `--json`. Parameters become named
flags. Top-level scalar JSON body properties also become flags; arrays repeat the flag,
and `html`, `text`, `body`, `content`, `markdown`, and `template` accept a matching
`--*-file` option. For example:

```sh
acme messages send --from hello@acme.test --to one@example.test --to two@example.test \
  --subject Welcome --html-file ./welcome.html
```

## Terminal experience

Generated CLIs automatically choose their mode. In an interactive terminal, omitted
required parameters and simple body fields are collected with concise prompts,
credentials entered through `auth set-token` or `auth set-key` are masked, and completed
operations receive a styled status line.

For example, running `acme messages send` prompts only for the required values that were
not already provided as flags.

When stdin or stdout is piped—or when `--json` is passed—the CLI never prompts. Success
values are compact JSON on stdout and failures are JSON on stderr. This makes the same
commands safe to use from agents, shell scripts, and CI:

```sh
acme messages send --from hello@acme.test --to person@example.test \
  --subject Welcome --text 'Hello' --json | jq .
```

Set `NO_COLOR=1` to suppress terminal colour. Complex request bodies remain explicit:
terminal mode prompts for `--data` JSON rather than guessing nested object structure.

## Generated references

Each generated package includes factual OpenAPI-derived command references at
`references/<command-group>.md`. They list operations, parameters, and simple
request-body fields. Poolster deliberately does not generate `SKILL.md`: real agent
workflow, safety, and product guidance belongs to the API provider.

### Override command names

Nested, polymorphic, or otherwise complex request bodies retain `--data` and
`--data-file` as the explicit JSON escape hatch. Poolster automatically groups known
verb-style operation IDs by literal path segments: `sendMessage` on `/messages` becomes
`messages send`, while `getUser` on `/admin/users/{id}` becomes `admin users get`.

Override either part when necessary with an operation extension:

```yaml
x-poolster-cli:
  group: messages
  command: deliver
```

`command: "messages deliver"` remains available as a complete explicit path. The default
base URL can also be replaced with the generated `<COMMAND>_BASE_URL` environment
variable. In non-interactive automation, `<COMMAND>_TOKEN` supplies an ephemeral token
without writing credentials to disk; it takes precedence over the selected profile.

## API keys and profiles

OpenAPI `apiKey` schemes are applied automatically in their declared header, query, or
cookie location. Store a key under its OpenAPI scheme name, or inject one in CI without
writing it to disk:

```sh
acme auth set-key "$ACME_KEY" --profile production
acme auth use production
ACME_APIKEY_TOKEN="$ACME_KEY" acme projects list
```

`auth set-key` infers the declared API-key scheme when there is exactly one; pass
`--scheme ApiKey` only when the API declares multiple key schemes. `auth profiles` lists
saved profiles; `auth use <name>` selects the default for future commands, and
`--profile <name>` always overrides it.

## Extensions

Every generated CLI includes `src/poolster.extension.ts`, a create-once file Poolster will never
overwrite. Its `extension` object can provide `authenticate`, `login`, `beforeRequest`,
and `afterResponse` hooks.

`authenticate` runs before Poolster looks up credentials and receives mutable headers/query
values; return `"handled"` after supplying custom SSO, keychain, client-assertion, or
request-signing credentials. It also receives `defaultAuthenticate()` to delegate to
Poolster's OAuth and token flow. `login` similarly receives a `defaultLogin` function.

The request and response hooks are intended for logging, tracing, custom headers, and
audit events.

## Authentication

`auth set-token` stores an API key, bearer token, or basic-credential value for one
OpenAPI security scheme. Use `--scheme` when an API has more than one:

```sh
acme auth set-token "$ACME_TOKEN" --scheme ApiToken --profile ci
```

`auth login` supports OAuth device authorization and Authorization Code with PKCE
(`--flow browser`). OAuth access and refresh tokens are persisted per profile and
refreshed before expiry. The generated runtime validates the PKCE callback state and
only accepts loopback callback URLs.

### Credential storage

Credentials are written to a user-private configuration file (`0700` directory, `0600`
file on Unix). For environments requiring hardware-backed or enterprise keychain
storage, provide a project runtime wrapper before distribution; Poolster does not silently
add a native credential dependency to every generated CLI.
