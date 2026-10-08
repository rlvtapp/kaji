# `poolster.json` reference

[Recipe introduction](cli/config.md) · [Copyable recipes](cli/recipes.md) · [CLI reference](cli.md)

> New to configuration? Start with the shorter [`poolster.json` recipe guide](cli/config.md)
> and [copyable CLI recipes](cli/recipes.md). This page is the complete field
> reference.

`poolster.json` is Poolster's recommended, reproducible generation recipe. It records
the OpenAPI input, output root, SDK packages, client choices, helper artifacts,
and mock/documentation output in source control.

## Editor autocomplete

Poolster's [JSON Schema](https://raw.githubusercontent.com/rlvtapp/poolster/main/schemas/v1/poolster.schema.json)
gives VS Code, JetBrains IDEs, and other JSON Schema-aware editors completion,
descriptions, enum choices, and inline validation. `poolster init` adds it for you.
For an existing file, add this as the first property:

```json
{
  "$schema": "https://raw.githubusercontent.com/rlvtapp/poolster/main/schemas/v1/poolster.schema.json"
}
```

Run `npx poolster init` to create a safe starter file, then run `npx poolster generate`. Poolster
never overwrites an existing config file. Use `npx poolster generate --config path.json`
when the recipe is not named `poolster.json` or is not in the current directory.

## Config mode versus direct mode

Poolster has two intentionally separate ways to generate:

| Mode | Command | Use it when |
| --- | --- | --- |
| Config-first | `npx poolster generate` or `npx poolster generate --config poolster.json` | The project has multiple packages, helper artifacts, a mock server, or a generation recipe worth reviewing and committing. |
| Direct | `npx poolster generate openapi.yaml --output generated --language go` | You need one quick package, an experiment, or a compact CI command. |

They cannot be combined. For example, `npx poolster generate --config poolster.json
--language go` fails rather than silently overriding part of the recipe. This
keeps generated releases deterministic and easy to review.

Direct mode has one TypeScript package and selects its transport with
`--typescript-transport fetch|axios`. Config mode can declare as many
TypeScript packages as needed, each with its own output path, package name, and
SDK transport.

## Complete example

<details>
<summary>Expand a full multi-package recipe</summary>

```json
{
  "openapi": {
    "input": "./openapi.yaml",
    "name": "Relevate Email",
    "version": "1.0.0"
  },
  "output": { "path": "./generated" },
  "defaults": { "client_style": "namespaced" },
  "packages": [
    {
      "language": "typescript",
      "path": "typescript/fetch",
      "name": "@relevate/email",
      "plugins": [
        {
          "name": "sdk",
          "transport": "fetch",
          "surface": "client",
          "client_name": "RelevateEmail",
          "group_by_tag": true,
          "throw_on_error": true
        },
        { "name": "zod", "output": "validation" },
        {
          "name": "tanstack-react-query",
          "output": "react-query",
          "group_by_tag": true
        },
        { "name": "msw", "output": "mocks" }
      ]
    },
    {
      "language": "typescript",
      "path": "typescript/axios",
      "name": "@relevate/email-axios",
      "plugins": [{ "name": "sdk", "transport": "axios" }]
    },
    {
      "language": "go",
      "path": "go",
      "name": "email",
      "plugins": [{ "name": "sdk", "jobs": 4 }]
    },
    {
      "language": "artifacts",
      "path": "docs",
      "plugins": [
        { "name": "redoc", "openapi_spec": "../openapi.yaml", "title": "Email API" },
        { "name": "mcp" }
      ]
    },
    {
      "language": "mock",
      "path": "mock-server",
      "plugins": [{ "name": "server", "image": "httpmock/httpmock:0.8.0", "port": 4010 }]
    }
  ]
}
```

All paths in the config are resolved from the directory containing the config
file. `input` and `artifacts` may be absolute paths, but package paths and
artifact `output` paths must remain safe relative output paths.


</details>

## Top-level fields

| Field | Required | Meaning |
| --- | --- | --- |
| `openapi` | Yes | OpenAPI source/compiler settings. |
| `output.path` | Yes | Root directory that receives every configured package. |
| `defaults.client_style` | No | Default `namespaced` or `flat` client surface for SDK packages. Defaults to `namespaced`. |
| `packages` | Yes | One or more independently generated packages. |

### `openapi`

Set exactly one source field:

| Field | Required | Meaning |
| --- | --- | --- |
| `input` | One of `input`/`artifacts` | A local Swagger 2.0 or OpenAPI 3.0/3.1 YAML/JSON file, or an `https://`/`http://` URL. Remote documents are downloaded into Poolster's private compiler workspace, then handled exactly like local input. |
| `artifacts` | One of `input`/`artifacts` | Existing Poolster compiler artifact directory for a faster repeat generation. |
| `name` | No | API display name; defaults to `API`. |
| `version` | No | Generated package version; defaults to `0.1.0`. |
| `compiler` | No | Local path to a replacement `poolster-openapi` executable. Only applies with `input`. |
| `paths.include` | No | OpenAPI path glob patterns to include. Empty includes every path. `*` crosses `/`; includes are ORed. |
| `paths.exclude` | No | OpenAPI path glob patterns to omit after inclusion. Exclusions always win. |

Path selectors apply once to the normalized contract, before every SDK, API
CLI, mock, and helper artifact is rendered. Each pattern must begin with `/`.
Poolster stops when they would produce zero operations—this catches a renamed
endpoint or a typo before it becomes an empty release.

```json
{
  "openapi": {
    "input": "./openapi.yaml",
    "paths": { "include": ["/messages*"], "exclude": ["/messages/internal*"] }
  }
}
```

## Generation metadata and review

Every generation writes `.poolster/generation.lock.json` below the output root.
Commit it alongside generated code. It has no credentials: it records Poolster's
version, source locator, hashes of the input/config/compiler artifacts, the
path selection, target package labels, and selected operation inventory.

That makes a generated change explainable in review and makes CI drift checks
reproducible. It is generated metadata, not a hand-edited configuration file:
run `poolster generate` to refresh it. A remote input's URL is recorded, but its
authorization headers/tokens are never written to the lock.

### Remote URL object, headers, and authentication

For a public document, `input` can be the URL string directly:

```json
{ "openapi": { "input": "https://api.example.com/openapi.yaml" } }
```

For a private document, make `input` an object. It accepts `url`, optional
`headers`, and one optional `auth` object. Header values and auth values can be
literal strings, `{ "env": "VARIABLE_NAME" }`, or
`{ "profile": "PROFILE_NAME" }`, which resolves only while
Poolster runs. Prefer environment values and never commit API tokens/passwords.

```json
{
  "openapi": {
    "input": {
      "url": "https://partner.example.com/openapi.json",
      "headers": {
        "X-Organization": "relevate",
        "X-API-Version": { "env": "PARTNER_API_VERSION" }
      },
      "auth": {
        "type": "basic",
        "username": { "env": "PARTNER_OPENAPI_USER" },
        "password": { "env": "PARTNER_OPENAPI_PASSWORD" }
      }
    }
  }
}
```

Bearer authentication is equally direct:

```json
{
  "openapi": {
    "input": {
      "url": "https://partner.example.com/openapi.json",
      "auth": { "type": "bearer", "token": { "env": "PARTNER_OPENAPI_TOKEN" } }
    }
  }
}
```

For a reusable provider mapping, keep the token in the environment and register
only its variable name:

```sh
export GITHUB_TOKEN=…
poolster auth login github --token-env GITHUB_TOKEN
```

Then use `{ "profile": "github" }` wherever a secret value is accepted:

```json
{ "auth": { "type": "bearer", "token": { "profile": "github" } } }
```

`poolster auth status` never exposes token values; `poolster auth logout github`
removes the mapping. Set `POOLSTER_CONFIG_HOME` to relocate this local profile
store, for example in a sandboxed agent workspace.

Use `headers` for API-key schemes or nonstandard authentication, for example
`"X-API-Key": { "env": "PARTNER_OPENAPI_KEY" }`.
Header names and values are
validated by the HTTP client.
Poolster applies custom headers first, then Basic or
Bearer auth, so the `auth` object deliberately wins if both try to set
`Authorization`.

Remote downloads follow normal HTTPS redirects, time out after
120 seconds, and reject documents larger than 128 MiB.
The download is stored
only in the temporary compilation workspace.

## Packages and plugins

Every package contains:

| Field | Required | Meaning |
| --- | --- | --- |
| `language` | Yes | One of `typescript`, `typescript-cli`, `rust`, `rust-cli`, `go`, `python`, `php`, `symfony`, `java`, `csharp`, `elixir`, `ruby`, `swift`, `mock`, or `artifacts`. `dotnet` remains a legacy alias for `csharp`. |
| `path` | Yes | Package directory below `output.path`. |
| `name` | No | Ecosystem package identity for SDK languages. |
| `client_style` | No | Package-level `namespaced` or `flat` override. |
| `version` | No | Independent SDK package version. |
| `release` | No | Optional build/test/publisher metadata; see [SDK automation](sdk-automation.md). |
| `plugins` | Yes | Plugins emitted into this package. Names are validated; unknown names are errors. |

### SDK languages

`rust`, `go`, `python`, `php`, `java`, `csharp`, `elixir`, `ruby`, and `swift` require exactly
one `{ "name": "sdk" }` plugin. Go accepts `jobs`, a bounded generation worker
count. Python accepts `async_client: true` on `sdk` and optional `webhooks` and
`roundtrips` consumer plugins. Java accepts opt-in `open_enums: true`. Native
Rust/Go library APIs also expose [independent providers](native-sdk-providers.md);
their CLI recipes still use the complete SDK.

`symfony` requires one `sdk` plugin. It produces a Symfony bundle that wraps the
portable PHP SDK; set `sdk_package` when its Composer name differs from Poolster's
default `poolster/<api>-sdk`.

### TypeScript

TypeScript supports the convenient `sdk` plugin or independent `models`,
`transport`, `operations`, and `client` providers.
Give instances unique `id`
values and select dependencies through `uses.models`, `uses.transport`, or
`uses.operations`.
An unbound role requires exactly one compatible provider.
Providers use `output` for a module or directory; helper consumers use it for
a directory.

Complete `sdk` and explicit providers cannot be mixed.
Native
providers support customized imports without `clients_import`.
See the
[TypeScript plugin reference](../crates/plugins/typescript/README.md).

When present, `sdk` appears exactly once and accepts:

| SDK field | Values / default | Meaning |
| --- | --- | --- |
| `transport` | `fetch` (default), `axios` | HTTP client implementation for that package. |
| `surface` | `client` (default), `raw` | Full client class/resources or direct operation functions only. |
| `client_name` | API-derived | Exported full-client class name. |
| `group_by_tag` | `true` | Organize operation and client modules by OpenAPI tag. |
| `throw_on_error` | `true` | Default typed-operation error behavior. |
| `integer_as_string` | `false` | Represent all integer schemas as strings. |
| `int64` | `number`, `string`, `bigint` | Representation for int64 schemas, with schema-directed codecs for lossless modes. |

Additional TypeScript plugins are `zod`, `tanstack-react-query`,
`tanstack-vue-query`, `swr`, `faker`, `msw`, and `cypress`.

Their shared fields are:

| Field | Default | Meaning |
| --- | --- | --- |
| `output` | Package root | Subdirectory beneath the package for this artifact. |
| `clients_import` | `./clients` | Legacy standalone helper imports; native consumers instead use `uses.operations`. |
| `max_file_bytes` | `131072` | Positive byte budget for Zod, Faker, MSW and Cypress modules; preserves entrypoints and splits at atomic declaration boundaries. A single declaration may exceed the budget. |
| `max_operations_per_file` | `50` | Positive operation budget for native React Query, Vue Query and SWR helper modules; preserves the aggregate entrypoint. |
| `id` / `uses` | automatic binding | Select native model/operation providers by instance name. |
| `group_by_tag` | `true` | Must match the SDK layout when hooks import its operation modules. |

Native providers finalize manifests with Zod/Faker/MSW runtime dependencies,
query framework peer dependencies, and Cypress as a development dependency.
Application setup and Cypress configuration remain application-owned. See [auxiliary generators](auxiliary-generators.md)
for behavioral limitations and framework-specific setup.

### Documentation and mock packages

| Language | Plugin | Fields | Output |
| --- | --- | --- | --- |
| `artifacts` | `redoc` | `output`, `openapi_spec`, `title` | `redoc.html`, `redocly.yaml` |
| `artifacts` | `mcp` | `output` | `tools.json` metadata manifest |
| `mock` | `server` | `image`, `port` | Docker `httpmock` contract mock package |

`mcp` creates metadata for an MCP integration; it does not start an MCP server.
The generated mock is useful across every generated SDK, but it is not a
complete behavioral simulation. Read [contract mocking](mocking.md) before
using it for integration tests.

## Package source customizations

SDK packages can also declare `middleware` entries with `source`, `path`, and
`symbol`; Python async output additionally needs `async_symbol`.
Poolster copies each
native source module and enables it in the generated HTTP runtime by default.
SDK customers need no registration.
Supported source layouts and factory ABIs
are listed in [SDK customization](sdk-customization.md).

Unsupported targets or
transport ABIs fail explicitly.
See the [complete runnable example](../examples/bundled-middleware/README.md)
for generation, an executable policy test, and release metadata.

Every package accepts a `customizations` array.
Each entry sets `mode` to `add`,
`replace`, or `patch`, a destination `path` relative to that package, and a
UTF-8 `source` relative to the config file.
A patch also sets `find`, which must
match exactly once.

These source overlays run after package finalization and
remain subject to safe regeneration and drift checks.
See [SDK customization](sdk-customization.md)
for examples, ownership rules, and runtime middleware.

## Console output and automation

Interactive generation has a compact summary of selected packages/plugins,
file count, duration, and output directory. Colors are automatic when stderr is
a terminal; `NO_COLOR` disables them. Pass `--color always` for a manually
styled transcript or `--color never` for plain logs. Piped and CI output stays
plain in the default `auto` mode.

Argument/configuration errors exit with status `2`; compiler, generation, and
filesystem errors exit with status `1`. Generated files are overwritten, while
explicit custom starter files and unrelated output files remain. Poolster does not
prune stale generated files, so use a fresh output directory after removing or
renaming packages, schemas, or operations.

## Package-local API reference

Set `api_reference: true` on any package to emit `API_REFERENCE.md` alongside
its generated source. The reference lists normalized operations, parameter
locations and types, response media/status mappings, and component schemas.
It is an optional language-neutral plugin; it does not replace generated
language symbols or include authentication secrets and specification examples.
The library equivalent is `poolster::api_reference::<Language>()`, with an optional
`.output("relative/path.md")`.

Python packages accept the optional `operation-tests` consumer. Terraform's
`provider` plugin accepts `data_sources: true` to expose supported single-entity
GET bindings as typed data sources. Both remain opt-in; consult the testing and
Terraform guides for capability limits.

### Shared source layout

Set `defaults.layout` once to split every supporting plugin into focused files.
A package's `layout` overrides this default; an individual plugin's `layout`
overrides both. An explicit `single-file` opts that plugin out of splitting.
Layout objects replace the inherited object in full.

```json
{
  "defaults": { "layout": { "mode": "per-operation" } },
  "packages": [{
    "language": "typescript",
    "path": "typescript",
    "plugins": [
      { "name": "sdk" },
      { "name": "tanstack-react-query" },
      { "name": "zod", "layout": { "mode": "single-file" } }
    ]
  }]
}
```

This excerpt belongs inside a config with `openapi` and `output`. Configurable
layouts in JSON are consumed by TypeScript query helpers, Zod, Faker,
MSW and Cypress. Other generators retain their native automatic file splitting.
A shared default does not imply that every generator supports every layout mode.
