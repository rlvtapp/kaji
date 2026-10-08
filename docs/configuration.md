# Configuration reference

## Choose a configuration surface

| Task | Surface |
| --- | --- |
| Select bundled HTTP SDKs and artifacts | [`kaji.json`](config-file.md) |
| Inspect a native source format | [Contract CLI and input providers](input-plugins.md#inspect-the-built-in-inputs) |
| Register an input or output plugin | Rust application linked to its provider crate |
| Compose typed provider/consumer contracts | [Typed Rust plugins](typed-plugins.md) |

## CLI JSON configuration

For normal CLI use, `kaji.json` is the source of truth. Create it with
`npx kajicli init`, then run `npx kajicli generate`. The complete config format and the
built-in SDK, TypeScript artifact, documentation, and mock plugin names are in
the dedicated [`kaji.json` reference](config-file.md).

The config is deliberately explicit: every package has a language, a directory,
and a list of selected plugins. Kaji does not run Node/JavaScript plugin code
from this file. Custom input and output plugins are linked into a Rust application.

The rest of this page is the equivalent typed Rust API for embedding Kaji,
building custom plugins, or using options that are not yet represented in the
CLI config.

Options belong to the plugin that uses them. Package identity belongs to the
language package. `Common` provides optional defaults across a release.

Import `kaji::prelude::*` to bring the package extension traits into scope.

## Complete release

```rust
use kaji::{csharp, elixir, go, java, mock, php, prelude::*, python, ruby, rust, swift, ts};

let release = ProfileSet::new("sdk")
    .common(Common::default()
        .client_style(SdkClientStyle::Namespaced)
        .package_version("1.0.0"))
    .package(ts::package("typescript/fetch")
        .name("@acme/sdk")
        .with(ts::sdk().fetch().client_name("Acme")))
    .package(ts::package("typescript/axios")
        .name("@acme/sdk-axios")
        .with(ts::sdk().axios().raw()))
    .package(rust::package("rust").name("acme-sdk").with(rust::sdk()))
    .package(go::package("go").name("acme").with(go::sdk().jobs(4)))
    .package(python::package("python").with(python::sdk().flat()))
    .package(php::package("php").with(php::sdk()))
    .package(java::package("java").with(java::sdk()))
    .package(csharp::package("csharp").with(csharp::sdk()))
    .package(elixir::package("elixir").with(elixir::sdk()))
    .package(ruby::package("ruby").with(ruby::sdk()))
    .package(swift::package("swift").with(swift::sdk()))
    .package(mock::package("mock-server")
        .with(mock::server().image("httpmock/httpmock:0.8.0").port(4010)));

let tree = kaji::generate(&api, release)?;
tree.write_to("generated")?;
```

This writes below `generated/sdk`. Package directories are explicit: they do not
have to match language names. Select Fetch and Axios in separate packages;
one TypeScript SDK plugin uses one transport.

## Release, package, and output

| API | Meaning |
| --- | --- |
| `ProfileSet::new(root)` | Relative directory under the final output directory. |
| `.common(Common)` | Release-wide optional defaults. |
| `.package(Package<L>)` | Add an independently configured language package. |
| `language::package(directory)` | Start a package with a safe relative directory. |
| `.name(name)` | SDK package identity; normalized by the language. Requires its `PackageExt` trait, included in the prelude. |
| `.common(Common)` on a package | Override release defaults for this package. |
| `.with(plugin)` | Add a `Plugin<L>`; options are set on the plugin instance. |
| `.settings(L::Settings)` / `.settings_mut()` | Language-owned configuration, useful for community integrations. |
| `generate(&api, release)` | Render packages into a `GeneratedTree`. |
| `generate_with_security_catalog(&api, release, Some(&catalog))` | Include named OpenAPI security definitions with an in-memory API. |
| `generate_with_input(&input, release)` | Consume an `InputContract` publishing `AdaptedApi` and render existing HTTP SDKs. |
| `generate_openapi(path, name, version, release)` | Read current compiler artifacts, including required schema and security catalogs, and generate. |
| `tree.check(directory)` | Compare expected output with its destination without writing. |
| `tree.write_to(directory)` | Validate/materialize owned output while preserving custom and unrelated files. |

A release must contain at least one package. Unsafe paths, overlapping file
owners, and invalid plugin contracts fail generation. Adding a package or plugin
twice is not a deduplication mechanism.

Generated output is tracked by `.kaji/ownership.json` with stable owners and
content fingerprints.
`tree.write_to` refuses to overwrite locally edited owned
files, removes only unchanged obsolete owned files, and preserves unrelated
files.
TypeScript `custom/index.ts` is create-once.
`tree.check(directory)` returns
added/modified/removed paths without writing; use it before materialization when
reviewing drift.

Preflight validates the proposed output, but filesystem I/O
failures can still interrupt writing: materialization is not an atomic directory
transaction.
See [safe regeneration](safe-regeneration.md) for legacy adoption,
manifest merge rules, and conflict resolution.

### SDK-author customization and delivery

Use [SDK customization](sdk-customization.md) for `Package::middleware` and
`Package::customize`: source is supplied by the author, scoped to one package,
and applied before output is materialized. Bundled middleware is wired into the
supported language's default runtime; added overlays alone are not automatic
registration. Unsupported middleware languages/paths or incompatible runtime
providers fail generation. Compile and test supplied source with native tools;
Kaji does not execute it during generation.

For release-enabled packages, [SDK automation](sdk-automation.md) explains
optional package metadata, native build/test commands, independent version
handling, generated SDK PRs, and Release Please. [Publishing](sdk-publishing.md)
covers registry trust and tagged artifacts. Metadata does not publish a package
by itself. Keep this reference for typed settings, and follow those guides for
the complete author workflow.

### Shared defaults

`Common::default()` leaves each setting unset:

| Builder / field | Effect |
| --- | --- |
| `.client_name(name)` / `client_name: Option<String>` | TypeScript SDK class name. Other first-party SDKs do not consume this value. |
| `.client_style(style)` / `client_style: Option<SdkClientStyle>` | `Flat` or `Namespaced`. SDK plugins default to namespaced. |
| `.layout(SourceLayout::PerOperation)` / `layout: Option<SourceLayout>` | Shared layout for supporting generators; explicit plugin layouts take precedence. |
| `.package_version(version)` / `package_version: Option<String>` | Overrides the API version seen by this package's generators. |

Precedence is explicit plugin choice, then package defaults, then release
defaults, then plugin defaults. Package version is resolved by the engine;
client settings are consumed by each plugin.

## Input provider options

`InputRegistry` selects a format and optional provider ID. `InputProvider<C>` adds
a native capability to a package graph. Features on the `kaji-inputs` bundle select
which providers are linked; each independent provider crate owns its parser
dependencies. See [input configuration](input-plugins.md#select-or-replace-a-provider)
for Cargo and Rust examples.

## SDK plugin options

Every SDK below supports `.flat()` and `.namespaced()`. The last explicit
style selection wins. All default to namespaced.

| Module | Transport/runtime | Additional SDK methods |
| --- | --- | --- |
| `ts` | Fetch (default) or Axios | See TypeScript reference below. |
| `rust` | Reqwest | `.operation_prefix(string)` prefixes direct operation names and resource delegates. |
| `go` | Standard-library HTTP | `.jobs(usize)`: 0 automatic (up to 8 workers), 1 serial, explicit values capped at 64. Files are always split. |
| `python` | Standard library | None beyond client style. |
| `php` | PSR-18 / PSR-7 | None beyond client style. |
| `java` | JDK HttpClient / Jackson | None beyond client style. |
| `csharp` | .NET 8 `HttpClient` / `System.Text.Json` | None beyond client style. `dotnet` remains a compatibility alias. |
| `elixir` | Finch / Jason | None beyond client style. |
| `ruby` | Ruby 3.1 standard library (`Net::HTTP`) | None beyond client style. |
| `swift` | Swift 5.9 `URLSession` | None beyond client style. |

Package naming is configured on `language::package(...).name(...)`, not on
the SDK plugin. Runtime credentials and retry knobs belong to the generated
SDK consumer, not these generation builders.

## TypeScript SDK

| Method | Default | Effect |
| --- | --- | --- |
| `.fetch()` / `.axios()` | Fetch | Select exactly one transport. The last selection wins. |
| `.client_name(name)` | Shared default or API-derived name | Exported class name. |
| `.flat()` / `.namespaced()` | Namespaced | Full-client method grouping. |
| `.raw()` | Full client | Omit the instantiated class; retain operation functions, models, and runtime. |
| `.group_by_tag(bool)` | `true` | Group operation/client files; schema files remain individual models. |
| `.model_options(ModelOptions)` | See below | Model syntax and enum formatting. |
| `.throw_on_error(bool)` | `true` | Default direct-operation error behavior; consumers may override per request. |

`raw()` controls the public surface, not the transport. A full client also
exports direct operations. See [generated SDKs](generated-sdks.md).

### ModelOptions

```rust
let sdk = ts::sdk().model_options(ts::ModelOptions {
    syntax: ts::Syntax::Interface,
    enum_type: ts::EnumType::AsConst,
    array_type: ts::ArrayType::Generic,
    ..Default::default()
});
```

| Field | Default | Choices / effect |
| --- | --- | --- |
| `array_type` | `ArrayType::Array` | `T[]`; `Generic` emits `Array<T>`. |
| `optional_type` | `OptionalType::QuestionToken` | `name?: T`; `QuestionTokenAndUndefined`: `name?: T \| undefined`; `Undefined`: `name: T \| undefined`. |
| `syntax` | `Syntax::Type` | Type aliases; `Interface` selects interfaces where appropriate. |
| `enum_type` | `EnumType::Literal` | Literal union, `AsConst`, `Enum`, or `ConstEnum`. |
| `enum_key_casing` | `EnumKeyCasing::None` | Also `CamelCase`, `PascalCase`, `SnakeCase`, `ScreamingSnakeCase`. |
| `enum_const_casing` | `EnumConstCasing::CamelCase` | Also `PascalCase`. |
| `enum_type_suffix` | `"Key"` | Suffix used for enum key types where emitted. |
| `integer_as_string` | `false` | Render integer schema types as strings; does not convert HTTP data at runtime. |
| `remove_optional_properties` | `false` | Omit optional properties from generated model shapes. |

These options apply to `ts::sdk()`, not the standalone `ts::types()` renderer.

### Types-only package

```rust
let models = ts::types().output("generated/models");
let models_handle = models.handle();
let package = ts::package("types").name("@acme/types").with(models);
```

`output` is a module path without `.ts`; its default is `models`.
The plugin publishes a `TsTypes` contract for community consumers. It generates
types and package metadata, not an HTTP client. Do not combine it with
`ts::sdk()` in one package: their files overlap. See
[plugin authoring](typed-plugins.md) for handles and shared workspaces.

### Auxiliary renderers

Zod, TanStack React/Vue Query, SWR, Faker, MSW, Cypress, ReDoc, and MCP manifests
can be selected by JSON config and also have an explicit `ArtifactOptions` Rust
API. They currently return files directly rather than being first-party
`.with(...)` package plugins. The complete [auxiliary generator
reference](auxiliary-generators.md) covers every option and integration
requirement.

## Mock server

`mock::package(directory).with(mock::server())` generates a language-neutral
Docker package. It has no SDK package `.name()` setting.

| Method | Default | Effect |
| --- | --- | --- |
| `.image(string)` | `httpmock/httpmock` | Docker base image; pin a tag or digest for reproducible releases. |
| `.port(u16)` | `5000` | Container and default published port; choose a usable nonzero port. |

See [contract mocking](mocking.md) for fixture declarations and limitations.

## CLI versus Rust configuration

The JSON CLI selects all current built-in SDK packages, auxiliary artifacts,
and the mock package. Model options, custom plugins, and typed inter-plugin
contracts remain Rust API work for now. See the [CLI reference](cli.md) for
the supported JSON fields and direct-mode flags.

## Idempotency keys

Idempotency is opt-in. An operation may declare `x-kaji-idempotency: true`, or
an object such as `{ "header": "Idempotency-Key", "auto_generate": true }`.
The default header is `Idempotency-Key`; automatic key generation defaults to
`false`. Setting `enabled: false` explicitly disables the policy.

Each SDK package can override operation extensions independently:

```json
{
  "language": "typescript",
  "path": "typescript",
  "plugins": [{ "name": "sdk" }],
  "idempotency": {
    "defaults": { "enabled": false },
    "operations": {
      "createOrder": { "enabled": true, "header": "X-Request-Key", "auto_generate": true }
    }
  }
}
```

Package defaults override operation extensions; named operation rules override
both.
Rules are complete replacements, rather than partial merges.
Unknown or
ambiguous operation IDs fail generation.
The core adds an optional string header
parameter to enabled operations and preserves an existing header's spelling
when it matches without regard to case.

Duplicate, required, referenced,
constrained, or non-string idempotency header schemas fail generation.
Policies
cannot use transport/authentication headers such as `Authorization`, `Host`,
`Content-Type`, or `Cookie`, and cannot collide with another parameter’s native
identifier after punctuation and case normalization.

Rust authors use `.idempotency(kaji_core::idempotency::IdempotencyConfig { ... })`
on their typed package.
Generation prepares a package-local API copy, so one
SDK's policy does not change another SDK's inputs.
Native runtime renderers
consume `x-kaji-idempotency-resolved` metadata; automatic generation must keep a
single key for all retry attempts and preserve a caller-supplied key.

A header
policy does not add retry loops to targets without them or promise server-side
deduplication.
See [the author and customer guide](guides/idempotency.md).

## Forward-compatible models

Java and C# SDK recipes support `"preserve_presence": true` on the `sdk` plugin.
This opt-in changes optional-property types to generated `Presence<T>` wrappers,
so missing properties round-trip separately from explicit null.
Required property
types stay unchanged; the generated `PRESENCE.md` describes native construction
and inspection.

Leave the option unset to preserve the existing public model API.
The option is rejected on other languages.
`"open_enums": true` also works for
TypeScript, Python, Rust, Java, C# and Swift.
It keeps known enum APIs while
allowing future wire values; TypeScript enum styles can change to const value
objects.

Rust also supports `"open_unions": true` for raw unmatched union values.
See [forward-compatible models](guides/forward-compatible-models.md) for native
representations and migration considerations.
