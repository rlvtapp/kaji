# Author and compose native generator plugins

Use native Rust plugins to add an input format, output consumer, provider or
language. Plugins are linked into your generator application. The shipped CLI
exposes its compiled registry; JSON cannot dynamically load crates or binaries.
The Rust API is pre-1.0, with no stable binary ABI.

```text
InputPlugin -> typed source contract -> package graph -> Plugin<L> -> files
```

| Starting point | Guide |
| --- | --- |
| Parse or replace an input | [Input plugins](input-plugins.md) |
| Compose maintained plugins | [Composition introduction](library/plugins.md) |
| Customize source for one SDK | [SDK customization](sdk-customization.md) |
| Deliver a generated package | [SDK automation](sdk-automation.md) |

[All hook signatures and lifecycle defaults →](plugin-hooks.md)

## Decide the extension scope

| Need | Authoring surface | What you maintain |
| --- | --- | --- |
| Add a source format or parser | `InputPlugin` plus native `Contract` values | Parsing, validation, source diagnostics and compatible consumers. |
| Generate standard SDKs in several languages | Package recipe plus each language's `sdk()` | Spec, names/options and native tests. |
| Add/replace/patch code for one package | `customizations` / `Package::customize` | Source files and explicit export/wiring patches. |
| Ship runtime policy already enabled for SDK consumers | `middleware` / `Package::middleware` | Source implementing the supported language factory ABI. |
| Add a reusable renderer/consumer | `Plugin<ExistingLanguage>` | Typed requirements, output contracts, files and validation. |
| Replace a model/transport/client provider | Language-owned provider contract | The contract and native runtime ABI consumed by downstream plugins. |
| Support another language | `Language` plus its plugins/workspace | Renderers, shared state, finalization and optional bundle hook. |
| Build or publish through another registry | Release metadata Post plugin | Commands, toolchain, registry auth and retry verification. |

Generator overrides are source overlays applied to generated files. They are not
a programmable transform of the neutral API model.

## What core owns, and what the language owns

Core owns input registry selection, typed graph resolution, phase ordering, contract publication, safe file
paths and ownership, package option inheritance, source overlays and optional
release metadata. It does not own language syntax, model renderers, transport
ABIs, framework code or registry authentication.

A `Language` declares its `Settings` and mutable `Workspace`. The language's
finalizer turns shared workspace state into files such as manifests or export
barrels. A TypeScript workspace tracks `Symbol`s, dependencies and exports;
other maintained languages may use a unit workspace or language-specific state.
Community languages can choose their own workspace without changing core.

C# is the canonical `poolster-plugin-csharp` crate and target. `dotnet` remains a
compatibility facade over that implementation, preserving existing Rust profiles
and the legacy CLI alias.

## Build a package recipe first

```rust
use poolster::prelude::*;
use poolster_plugin_rust as rust;
use poolster_plugin_typescript as ts;

let release = ProfileSet::new("sdk")
    .common(Common::default().client_style(SdkClientStyle::Namespaced))
    .package(ts::package("typescript/fetch")
        .name("@acme/api")
        .with(ts::sdk().fetch().client_name("Acme"))
        .with(ts::composition::zod().output("validation")))
    .package(ts::package("typescript/axios")
        .with(ts::sdk().axios().raw()))
    .package(rust::package("rust").with(rust::sdk()));

// poolster::generate(&api, release)?.write_to(output_directory)?;
```

The prelude imports package extension traits. Package settings own identity and
shared defaults; each plugin instance owns its rendering options. `Common`
provides optional client name/style/version defaults. Package defaults override
release defaults, and explicit plugin settings override inherited values. A
language consumes only options it implements; a shared default does not create
an unsupported capability.

Separate Fetch/Axios packages make distinct distributable variants. An advanced
recipe can put several independent providers in one package, but their files and
contracts must remain unambiguous. Do not combine complete `sdk()` and another
model renderer owning the same model files.

## Contracts describe actual outputs

The same `Contract` trait connects native inputs and generated outputs.
`InputProvider<C>` places a registered input capability into this graph; see
[the input bridge](input-plugins.md#feed-an-input-into-the-output-graph).

A contract is a Rust value implementing `Contract`. It might contain generated
symbols, module paths, model settings, or another typed capability. Prefer facts
about the actual provider output over naming conventions that consumers guess.

| Declaration | Runtime operation | Meaning |
| --- | --- | --- |
| `Provision::of::<C>()` | `cx.publish(value)` | This plugin produces contract `C`. |
| `Requirement::on::<C>(None)` | `cx.inputs.get::<C>()` | Bind the unique available provider automatically. |
| `Requirement::on::<C>(Some(handle))` | `cx.inputs.get::<C>()` | Bind this particular provider instance. |
| Requirement with `.optional()` | `cx.inputs.optional::<C>()` | Absence is permitted; ambiguous/dangling bindings still fail. |

The resolver orders providers before consumers and rejects missing or ambiguous
bindings, cycles, duplicate identities and invalid handles. Contracts and handles
are package-local.

A plugin may only read declared inputs and publish declared outputs. Every
provision must be published. Store one fresh `Meta` on each instance and return a
reference to it; recreating metadata breaks instance identity. Labels affect
diagnostics only.

## Compose TypeScript providers independently

The maintained providers reuse the complete SDK's renderers:

```rust
use poolster::prelude::*;
use poolster_plugin_typescript as ts;
use ts::composition;

let models = composition::models().output("domain/types");
let model_handle = models.models_handle();
let transport = composition::transport().axios().output("infra/http");
let transport_handle = transport.transport_handle();
let operations = composition::operations()
    .output("api/calls")
    .using_models(model_handle)
    .using_transport(transport_handle);
let operation_handle = operations.operations_handle();

let package = ts::package("web")
    .with(models)
    .with(transport)
    .with(operations)
    .with(composition::client().output("api/client")
        .using_models(model_handle)
        .using_transport(transport_handle)
        .using_operations(operation_handle))
    .with(composition::react_query().output("ui/queries")
        .using_operations(operation_handle));
```

| TypeScript contract | Provider publishes | Consumer must respect |
| --- | --- | --- |
| `composition::Models` | Schema symbols, operation model modules and selected `ModelOptions` | Actual paths/names, aliases and integer/optional/enum settings. |
| `composition::Transport` | Runtime module path and lossless JSON capability | Required native TypeScript runtime exports and codecs. |
| `composition::Operations` | Operation-id-to-function symbols | Actual function paths/signatures rather than hardcoded client imports. |
| `composition::Client` | Public client symbol | Selected facade module/name. |
| `TsTypes` | Types-only schema symbols | This older types-only contract is distinct from the SDK `Models` contract. |

The convenience `ts::sdk()` publishes model, transport and operation contracts,
so native auxiliaries can consume it. The provider graph is available when you
want independent ownership and substitution. `composition::{react_query,
vue_query,swr}` consume operation contracts; `zod`/`faker` consume model contracts;
MSW/Cypress consume models and operations. MSW/Cypress retain their existing smoke
route behavior rather than promising a full behavioral service simulator.

Use the selected `Symbol::import_from` to compute imports after relocation.
Register dependencies and exports during Generate before finalization:
`workspace.dependency`, `dev_dependency`, `peer_dependency`, `export` or
`export_namespace`. Conflicting dependency ranges fail; there is no semver
intersection solver. Namespaced exports keep relocated provider symbols from
colliding at the root. See the [TypeScript reference](../crates/plugins/typescript/README.md)
for exact layouts, auxiliaries and strict generated-consumer tests.

### Select providers in the shipped CLI

This is a package fragment for a normal recipe:

```json
{
  "language": "typescript",
  "path": "web",
  "plugins": [
    { "name": "models", "id": "domain", "output": "domain/types" },
    { "name": "transport", "id": "http", "transport": "fetch", "output": "infra/http" },
    { "name": "operations", "id": "calls", "output": "api/calls",
      "uses": { "models": "domain", "transport": "http" } },
    { "name": "client", "output": "api/client",
      "uses": { "models": "domain", "transport": "http", "operations": "calls" } },
    { "name": "tanstack-react-query", "output": "ui/queries",
      "uses": { "operations": "calls" } }
  ]
}
```

This fragment was verified with the checked-in [CLI basic Notes contract](../examples/cli-basic/openapi.yaml).
Use it in a full recipe with that contract as the OpenAPI input, or supply your
own noncolliding schema and operation names.

`id` identifies a configured provider; `uses` selects compatible roles in that
package. Named binding support is currently TypeScript-specific. Rust/Go expose
native provider APIs through Rust embedding, not equivalent CLI bindings.
Arbitrary community plugins require a generator executable linking them in;
JSON cannot load a Rust crate or a replacement transport implementation.

### Substitute a custom transport deliberately

A native TypeScript plugin declares `Provision::of::<composition::Transport>()`,
emits its runtime module, and publishes `Transport::new("custom/request")`.
That module must implement the runtime exports/signatures consumed by operations,
including request/result, options and response helpers. Relocating a module is
not sufficient to establish ABI compatibility. Models using exact string/bigint
integers also require coherent lossless serialization/parsing; publish
`.lossless_json(true)` only when the custom transport actually supplies it.
Unsupported lossless transports fail rather than silently rounding integers.

See [native Rust/Go providers](native-sdk-providers.md) for their transport
contracts and practical boundaries. Transport substitution should be tested
through a generated public method and its real consumer compiler.

## Start with a compiled minimal plugin

The standalone [custom plugin example](../examples/custom-plugin/README.md)
depends only on `poolster-core`, declares a documentation language, publishes a typed
`Names` contract from two providers and binds a consumer to the replacement’s
handle. Its test verifies the actual emitted value, not just registration:

```sh
cargo test --manifest-path examples/custom-plugin/Cargo.toml
```

Use this example before introducing a model or transport ABI.

## Add a target-neutral API reference

Any native package can include the generic consumer:

```rust
let package = poolster_plugin_typescript::package("typescript")
    .with(poolster_plugin_typescript::sdk())
    .with(poolster_core::api_reference().output("docs/API_REFERENCE.md"));
```

`api_reference::<L>()` defaults to `API_REFERENCE.md` and publishes the typed
`ApiReferenceDocument` contract. Its `.handle()` allows another consumer to bind
that document explicitly. It reads the normalized `Api` directly and has no
model/transport provider dependency. CLI packages can opt in with
`"api_reference": true`; the default is false.

The reference lists operation IDs, HTTP methods/paths, parameters, request media,
response statuses/media, schema references and component field shapes.
These are
contract identifiers, not a promise of generated client method names.
It omits
examples, defaults, enum literal values, source descriptions and external reference
URIs, and escapes active Markdown/HTML.

It is documentation, not an executable
operation test or full schema validator.
Output remains owned and participates in
safe regeneration/checks; the normal writer rejects path escapes and collisions.

## Implement a reusable consumer

This small plugin emits an inventory from the model contract. It is an example
of generator authoring; the generated SDK itself does not depend on Rust:

```rust
use anyhow::Result;
use poolster::prelude::{GeneratedFile, Meta, Plugin, PluginContext, Requirement};
use poolster_plugin_typescript::{TypeScript, composition::Models};

struct ModelInventory { meta: Meta }
impl Plugin<TypeScript> for ModelInventory {
    fn kind(&self) -> &'static str { "model-inventory" }
    fn meta(&self) -> &Meta { &self.meta }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on::<Models>(None)]
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
        let names = cx.inputs.get::<Models>()?.schemas.keys()
            .cloned().collect::<Vec<_>>().join("\n");
        cx.files.emit(GeneratedFile::new("model-inventory.txt", names)?)
    }
}
```

Construct it with `ModelInventory { meta: Meta::new() }` and add `.with(...)` to
a package containing the SDK or model provider. The `poolster` prelude re-exports the core plugin types; a generator can also depend
on `poolster_core` directly.
Check imports against the pinned crate version.

## Generation phases and file ownership

```text
Validate graph
  -> Generate plugins in dependency order
  -> language finalize (manifests, barrels, shared metadata)
  -> Post plugins
  -> language bundled middleware registration
  -> language finalize_files
  -> ordered source overlays
  -> owned-output planning and write/check
```

Use `Enforce::Post` or `PluginPhase::Post` for derived artifacts requiring the
completed normal package. Generate consumers register shared dependency/export
state before finalization; a Post plugin should emit completed files deliberately,
not assume the finalizer will run a second time. Release metadata is an optional
Post plugin and does not teach core to invoke registry SDKs.

Emit package-relative `GeneratedFile`s through `cx.files.emit`. Duplicate owners,
escaping paths and lexical path aliases fail. `emit_custom` creates a starter
file that future regeneration preserves; it is not appropriate for author-owned
source that must update from a recipe. The ownership planner preserves unowned
files and rejects manual edits to owned output. Read
[safe regeneration](safe-regeneration.md) before implementing replacements.

## Add a language's bundled middleware hook

`Package::middleware(BundledMiddleware { path, contents, symbol, async_symbol })`
bundles SDK-author source that is registered automatically in generated runtimes.
The CLI reads UTF-8 `source` files relative to the recipe; native embedding passes
contents directly. Consumers of the SDK need no constructor middleware setup.

Implement `Language::bundle_middleware(tree, middleware)` for the language's
native ABI. The hook runs after Post plugins and before source overlays, on a
package-relative staged tree. Validate supported paths/symbols, reject collisions
before inserting source, register the factory in the actual client, and preserve
normal injected transports. If the language does not implement the hook,
bundled middleware fails explicitly rather than being emitted but unused.

TypeScript uses exported middleware functions; Python supports sync/async
symbols; Go and Rust use their maintained transport contracts. Java/C#/PHP use
static native transport factories. These are distinct ABIs, not interchangeable
source files. Refer to [SDK customization](sdk-customization.md) for per-language
contracts and source examples. A source overlay that merely adds a file does not
register middleware automatically.

## Troubleshoot provider model naming

The flat TypeScript model provider currently rejects a collision between a
component schema name and an operation model name. For example, a component
`CreatePet` and operation `createPet` can both emit `CreatePet.ts`, causing a
`multiple generators emitted` error. Changing the provider output directory
relocates both files and does not resolve their collision.

Use noncolliding component/operation names in the contract, or use the SDK
convenience recipe with its tagged layout where it separates these model paths.
The checked-in TypeScript stack's complete `sdk` recipe works with its Pet
contract; switching that contract directly to flat providers is not yet an
interchangeable layout choice. Compile/check the concrete result before moving
an existing package onto independent providers.

## Validate the extension before releasing it

Exercise missing/ambiguous bindings, explicit provider selection, cycles and
colliding output. Compile an actual generated SDK/consumer with the native
compiler. Test relocated imports, selected model settings, request/response/error
behavior, streaming ownership, retry timing and regeneration after removing the
plugin. A renderer string assertion cannot substitute for a runtime test of a
new transport ABI.

Core graph tests live in `crates/core/tests/typed_packages.rs`; language
provider tests live beside their renderers. TypeScript's opt-in compiler/runtime
tests require a local TypeScript installation and actual framework dependencies.
Native language compilation tests require their toolchains. State which tests
you executed; generation success alone does not prove a package compiles or a
publisher can authenticate.
