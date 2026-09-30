# TypeScript API CLI

The `typescript-cli` package turns OpenAPI operations into a publishable Node.js
command-line client. It generates one command per operation ID, named in kebab
case, plus `auth login`, `auth set-token`, `auth status`, and `auth logout`.

Use config mode so the API origin and any product-specific OAuth information
are explicit and reviewable:

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

`authorization_url` and `token_url` are read from the selected OpenAPI OAuth2
security scheme when available. Set either field in the `oauth` block to
override that source. A device authorization URL is not part of ordinary
OpenAPI OAuth metadata, so specify `device_authorization_url` for device login.

After generation, install and build the generated package:

```sh
cd generated/cli
npm install
npm run build
npm link
acme auth login
acme list-projects --json
```

Every operation accepts `--profile`, `--base-url`, and `--json`. Parameters
become named flags. Top-level scalar JSON body properties also become flags;
arrays repeat the flag, and `html`, `text`, `body`, `content`, `markdown`, and
`template` accept a matching `--*-file` option. For example:

```sh
acme messages send --from hello@acme.test --to one@example.test --to two@example.test \
  --subject Welcome --html-file ./welcome.html
```

Nested, polymorphic, or otherwise complex request bodies retain `--data` and
`--data-file` as the explicit JSON escape hatch. Kaji automatically groups
known verb-style operation IDs by literal path segments: `sendMessage` on
`/messages` becomes `messages send`, while `getUser` on `/admin/users/{id}`
becomes `admin users get`. Override either part when necessary with an
operation extension:

```yaml
x-kaji-cli:
  group: messages
  command: deliver
```

`command: "messages deliver"` remains available as a complete explicit path.
The default base URL can also be replaced with the generated
`<COMMAND>_BASE_URL` environment variable. In non-interactive automation,
`<COMMAND>_TOKEN` supplies an ephemeral token without writing credentials to
disk; it takes precedence over the selected profile.

## API keys and profiles

OpenAPI `apiKey` schemes are applied automatically in their declared header,
query, or cookie location. Store a key under its OpenAPI scheme name, or inject
one in CI without writing it to disk:

```sh
acme auth set-key "$ACME_KEY" --profile production
acme auth use production
ACME_APIKEY_TOKEN="$ACME_KEY" acme projects list
```

`auth set-key` infers the declared API-key scheme when there is exactly one;
pass `--scheme ApiKey` only when the API declares multiple key schemes. `auth
profiles` lists saved profiles; `auth use <name>` selects the default for future
commands, and `--profile <name>` always overrides it.

## Extensions

Every generated CLI includes `src/kaji.extension.ts`, a create-once file Kaji
will never overwrite. Its `extension` object can provide `authenticate`,
`login`, `beforeRequest`, and `afterResponse` hooks. `authenticate` runs
before Kaji looks up credentials and receives mutable headers/query values;
return `"handled"` after supplying custom SSO, keychain, client-assertion, or
request-signing credentials. It also receives `defaultAuthenticate()` to
delegate to Kaji's OAuth and token flow. `login` similarly receives a
`defaultLogin` function. The request and response hooks are intended for
logging, tracing, custom headers, and audit events.

## Authentication

`auth set-token` stores an API key, bearer token, or basic-credential value for
one OpenAPI security scheme. Use `--scheme` when an API has more than one:

```sh
acme auth set-token "$ACME_TOKEN" --scheme ApiToken --profile ci
```

`auth login` supports OAuth device authorization and Authorization Code with
PKCE (`--flow browser`). OAuth access and refresh tokens are persisted per
profile and refreshed before expiry. The generated runtime validates the PKCE
callback state and only accepts loopback callback URLs.

Credentials are written to a user-private configuration file (`0700` directory,
`0600` file on Unix). For environments requiring hardware-backed or enterprise
keychain storage, provide a project runtime wrapper before distribution; Kaji
does not silently add a native credential dependency to every generated CLI.
