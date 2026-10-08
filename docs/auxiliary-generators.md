# Auxiliary generators: validation, hooks, fixtures, and docs

> Start with [TypeScript helpers](guides/typescript-helpers.md),
> [testing generated SDKs](guides/testing.md), and
> [generated artifacts](guides/artifacts.md). This page is the detailed
> renderer and Rust API reference.

Kaji has Rust renderers for Zod, TanStack React/Vue Query, SWR, Faker, MSW,
Cypress, ReDoc, and MCP tool manifests.

These renderers are available from `kaji.json` as built-in config plugins and
also expose a direct Rust API. `kaji generate --language all` remains a
direct-mode shortcut for SDK packages only; it does not guess which optional
artifacts your project wants. Use the [JSON recipe](cli.md#json-recipes-and-built-in-plugins)
to select them, or use the Rust API below when embedding Kaji.

For a TypeScript package, use `zod`, `tanstack-react-query`,
`tanstack-vue-query`, `swr`, `faker`, `msw`, or `cypress`. Use an `artifacts`
package for `redoc` and `mcp`. `output`, `clients_import`, and `group_by_tag`
map to `ArtifactOptions`; ReDoc also accepts `openapi_spec` and `title`.
All configured artifact files are emitted under the package's `path`.

## Generate files directly

Every renderer exposes the same inherent method:

```rust
generate(&self, api: &kaji_core::Api, options: &ArtifactOptions)
    -> anyhow::Result<Vec<kaji_core::GeneratedFile>>
```

For example, write a Zod module beside a full TypeScript SDK:

```rust
use kaji::{prelude::*, ts};
use kaji::ts::artifacts::{ArtifactOptions, TypeScriptZod};

let mut tree = kaji::generate(
    &api,
    ProfileSet::new("sdk")
        .package(ts::package("typescript").with(ts::sdk().fetch())),
)?;
let options = ArtifactOptions {
    output_dir: Some("sdk/typescript".into()),
    ..Default::default()
};
for file in TypeScriptZod.generate(&api, &options)? {
    tree.insert(file)?;
}
tree.write_to("generated")?;
```

The file is `generated/sdk/typescript/zod.ts`. Direct insertion does not add
npm dependencies or barrel exports. Add the appropriate runtime dependency
and import/re-export the module in your consuming package, or use a wrapper
plugin to register dependencies through the workspace.

## Available renderers

All names below are exported from `kaji::ts::artifacts`.

| Renderer | Default output | Purpose / required consumer dependency |
| --- | --- | --- |
| `TypeScriptZod` | `typescript/zod.ts` | Zod 4 component, request-body, and response validation schemas; `zod`. |
| `TypeScriptReactQuery` | `typescript/react-query.ts` | GET query keys/hooks and non-GET mutation hooks; `@tanstack/react-query` and its framework peers. |
| `TypeScriptVueQuery` | `typescript/vue-query.ts` | GET queries and non-GET mutations; `@tanstack/vue-query` and its peers. |
| `TypeScriptSwr` | `typescript/swr.ts` | GET hooks only; `swr` and its peers. |
| `TypeScriptFaker` | `typescript/faker.ts` | Component factories; `@faker-js/faker` plus generated model exports. |
| `TypeScriptMsw` | `typescript/msw.ts` | MSW v2 route-handler scaffolding; `msw`. |
| `TypeScriptCypress` | `cypress/e2e/api.cy.ts` | Routing smoke-test scaffolding; a configured Cypress project. Config mode adds Cypress as a development dependency when Kaji owns the package manifest. |
| `ReDoc` | `redoc/redoc.html`, `redoc/redocly.yaml` | Documentation entry point loading ReDoc from its CDN. |
| `McpToolManifest` | `mcp/tools.json` | Tool metadata for your own MCP integration, not an executable server. |

These helpers have narrower scope than full framework-specific generator
products. They do not expose every TanStack/Zod/MSW feature or every upstream
generator option.

For a complete, runnable config that places these helpers beside a Fetch SDK,
keeps an Axios SDK separate, and includes the shared Docker mock, see the
[complete TypeScript example](../examples/typescript-stack/README.md).

## ArtifactOptions reference

Use struct update syntax with `..Default::default()` to override only needed
fields. Options are typed Rust values, not string-keyed plugin configuration.

| Field | Default | Used for |
| --- | --- | --- |
| `output_dir: Option<String>` | `None` | Use renderer-specific directory above. `Some(".")` places files at the receiving tree/package root. Other values are safe relative paths. |
| `package_name: Option<String>` | `None` | Reserved by shared package rendering; current public auxiliary renderers do not emit a manifest or consume this setting. |
| `naming: Naming` | `CamelCase` | Hook query-key and MCP tool identifiers; also `PascalCase` and `SnakeCase`. Does not rename generated SDK operation imports. |
| `openapi_spec: Option<String>` | `None` | ReDoc spec URL/path; falls back to `openapi.yaml`. Does not copy the spec. |
| `title: Option<String>` | `None` | ReDoc page title; falls back to the API name. |
| `clients_import: String` | `"./clients"` | Hook imports relative to the emitted hook file. |
| `group_by_tag: bool` | `true` | Hook operation subdirectories; must match the SDK plugin setting. |

## TanStack/SWR with Fetch or Axios

Hooks import the **actual operation functions** generated by `ts::sdk()`.
They do not generate or select a second HTTP transport. Put hooks in the same
TypeScript package as its Fetch or Axios SDK, or explicitly point their imports
at the desired operation modules.

```rust
use kaji::ts::artifacts::{ArtifactOptions, TypeScriptReactQuery};

let options = ArtifactOptions {
    output_dir: Some("sdk/typescript".into()),
    clients_import: "./clients".into(),
    group_by_tag: true,
    ..Default::default()
};
for file in TypeScriptReactQuery.generate(&api, &options)? {
    tree.insert(file)?;
}
```

Keep `group_by_tag` aligned with `ts::sdk().group_by_tag(...)`. If the hook
file moves into a subdirectory, adjust `clients_import`, for example to
`"../clients"`. Hook input is derived from
`Parameters<typeof generatedOperation>[0]`, including the configured client.

React/Vue consumers provide typed query/mutation option factories, key
factories, hook overrides and cancellation forwarding. SWR accepts native
configuration. Keys omit client/config/headers and include an explicit cache
scope plus operation inputs; choose distinct scopes for tenants/origins or
response-affecting headers. Set `throwOnError: true` to reject SDK failures.
Infinite/suspense helpers and automatic SSR policy remain follow-up work.

The native composition plugins split after 50 operations while preserving the
configured entrypoint. Set `max_operations_per_file` in recipes, or call
`.max_operations_per_file(...)` / `.single_file()` on the Rust query builder.
Direct `ArtifactOptions` renderers above still emit one file. See the
[helper guide](guides/typescript-helpers.md) for examples and cache semantics.

## A typed wrapper plugin

A wrapper can participate in package file ownership and dependency collection:

```rust
use anyhow::Result;
use kaji::{prelude::*, ts};
use kaji::ts::artifacts::{ArtifactOptions, TypeScriptZod};

struct Zod {
    meta: Meta,
}

impl Plugin<ts::TypeScript> for Zod {
    fn kind(&self) -> &'static str { "example-zod" }
    fn meta(&self) -> &Meta { &self.meta }

    fn generate(&self, cx: &mut PluginContext<'_, ts::TypeScript>) -> Result<()> {
        cx.workspace.dependency("zod", "^4.0.0")?;
        let options = ArtifactOptions {
            output_dir: Some(".".into()),
            ..Default::default()
        };
        for file in TypeScriptZod.generate(cx.api, &options)? {
            cx.files.emit(file)?;
        }
        Ok(())
    }
}

let package = ts::package("typescript")
    .with(ts::sdk())
    .with(Zod { meta: Meta::new() });
```

This declares a dependency and adds `zod.ts`; it does not automatically re-export
it from `index.ts`. TypeScript's preserved `custom/index.ts` can provide your
durable exports. The wrapper shown uses only the input API, so it has no contract
dependency. A consumer of another plugin's emitted symbols should declare
typed requirements instead; see [plugin authoring](typed-plugins.md).

## Important boundaries

- **Zod:** Kaji targets Zod 4. The module exports one `<Name>Schema` per
  component plus a stable `kajiSchemas` registry and `getKajiSchema(name)`.
  It also exports `<Operation>RequestBodySchemas` and
  `<Operation>ResponseSchemas`, indexed by declared media type and response
  status, with the combined `kajiOperationSchemas` registry keyed by operation
  id. Zod 4 schemas implement Standard Schema V1, so the registries can be
  consumed by either Zod or Standard Schema-aware validation wrappers.
  Request-body and response entries are emitted only when the source declares
  a schema; Kaji does not guess a media type or synthesize validators for
  undeclared parameter bundles. This is not a complete JSON Schema validator;
  unsupported constraints may be approximated. Compile and test the generated
  validation module for your schema shapes.
- **Faker:** factories are randomized unless you seed Faker yourself. The module
  imports `./models`: pair it with `ts::types()` at its default location or
  supply a matching model barrel. The split SDK does not automatically create
  that barrel. Recursive models may recurse indefinitely; union/intersection
  handling is simplified. Do not treat generated samples as validated fixtures.
- **MSW:** handlers return empty JSON objects as editable scaffolding; they do
  not replay complete OpenAPI examples or mock scenarios. For contract-derived
  responses and conditional cases, use [the HTTP mock package](mocking.md).
- **Cypress:** requests include every operation's method and template path,
  without filling path values, authentication, or bodies. Configure those
  before use. It may issue writes/deletes: run only against an isolated test
  service, never a production API.
- **ReDoc:** the HTML depends on a CDN script and an accessible spec URL. It is
  not an offline bundled documentation site.
- **MCP:** the manifest includes operation metadata and basic inputs. It does
  not start a server, enforce authorization, or provide a transport.

Large Zod, Faker, MSW and Cypress outputs split into adjacent chunk directories
once their source exceeds 128 KiB. Imports and the aggregate entrypoint remain
stable, including custom output directories and model provider bindings. Set
`max_file_bytes` on the plugin in `kaji.json`, or call
`.max_file_bytes(...)` on the Rust builder. Declarations stay intact, so a single
large schema or operation may exceed the budget. Recursive Zod schemas use
explicit model types and lazy references across chunks. Recursive Faker factories
limit optional branches, arrays and nullable recursion; a required cycle with no
finite fixture throws a clear error rather than exhausting the call stack.
