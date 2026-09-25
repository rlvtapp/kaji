# Configuration reference

`ProfileSet` is the first-party configuration API. It is a typed
Rust builder: there is no JavaScript runtime, config discovery, or hidden
plugin order. Calling a target method more than once is safe—it only emits one
package for that target.

## Complete profile example

```rust
use kaji::{
    MockServerOptions, PackageOptions, ProfileSet, SdkClientStyle,
    SdkSurface, TypeScriptOptions,
};

let profiles = ProfileSet::new("sdk")
    .rust()
    .typescript_fetch()
    .typescript_axios()
    .go()
    .python()
    .php()
    .java()
    .dotnet()
    .elixir()
    .mock_server()
    .typescript_options(TypeScriptOptions {
        client_name: Some("RelevateEmail".into()),
        client_style: SdkClientStyle::Namespaced,
        surface: SdkSurface::Client,
        group_by_tag: true,
    })
    .go_options(PackageOptions {
        package_name: Some("relevateemail".into()),
        client_style: SdkClientStyle::Namespaced,
    })
    .python_options(PackageOptions::flat())
    .mock_server_options(MockServerOptions {
        image: "httpmock/httpmock:0.8.0".into(),
        port: 5000,
    });
```

An empty output root or a profile with no targets is rejected. `generate` is
the operation that validates and renders the entire set.

### ProfileSet API map

| Method | Use it for |
| --- | --- |
| `ProfileSet::new(root)` | Start a release under one output root. |
| `.with(Target)` | Add any target dynamically; duplicates are ignored. |
| `.rust()`, `.typescript_fetch()`, etc. | Add a named first-party target. |
| `.typescript_options(...)` | Set options shared by both selected TypeScript targets. |
| `.<language>_options(...)` | Set package identity and client style for one native target. |
| `.mock_server_options(...)` | Set the generated mock image and port. |
| `.build()` | Inspect the underlying `SdkProfile` values for the core Rust/TypeScript targets. It does not render packages and does not include external native plugin targets. |

For normal use, pass the builder directly to `generate`, `generate_openapi`,
or `generate_openapi_file`.
`build()` is primarily useful to an integration that is composing the
lower-level core generator itself.

## Targets

| Builder method | Output directory | Runtime/transport | Default client shape |
| --- | --- | --- | --- |
| `.rust()` | `rust` | Reqwest | namespaced |
| `.typescript_fetch()` | `typescript-fetch` | Fetch | namespaced |
| `.typescript_axios()` | `typescript-axios` | Axios | namespaced |
| `.go()` | `go` | Go standard library | namespaced |
| `.python()` | `python` | Python standard library | namespaced |
| `.php()` | `php` | PSR-18 / PSR-7 | namespaced |
| `.java()` | `java` | JDK `HttpClient` / Jackson | namespaced |
| `.dotnet()` | `dotnet` | `HttpClient` / `System.Text.Json` | namespaced |
| `.elixir()` | `elixir` | Finch / Jason | namespaced |
| `.mock_server()` | `mock-server` | standalone `httpmock` Docker image | n/a |

The output root is the argument to `ProfileSet::new`. For example,
`ProfileSet::new("artifacts")` writes the Go package to `artifacts/go`.

You can also select a target dynamically with `.with(Target::Go)`. `Target`
has one variant for every row above: `Rust`, `TypeScriptFetch`,
`TypeScriptAxios`, `Go`, `Python`, `Php`, `Java`, `DotNet`, `Elixir`, and
`MockServer`.

## TypeScript options

`TypeScriptOptions` applies to both selected TypeScript transports.

| Field | Default | Meaning |
| --- | --- | --- |
| `client_name: Option<String>` | derived from API title | Exported SDK class name, e.g. `RelevateEmail`. |
| `client_style: SdkClientStyle` | `Namespaced` | `client.contacts.list()` or `client.listContacts()`. |
| `surface: SdkSurface` | `Client` | `Client` exports the configured class and raw operation functions; `Raw` exports only models and direct functions. |
| `group_by_tag: bool` | `true` | Organizes operation and model files by the first OpenAPI tag. Untagged operations get a stable path-derived group. Set `false` for one flat directory. |

Convenience constructors:

```rust
// Direct models + operation functions only.
let raw = TypeScriptOptions::raw();

// A class client with `client.createContact(...)` methods.
let flat = TypeScriptOptions::flat_client();
```

The generated package always keeps direct operation exports. `Raw` only
removes the instantiated class wrapper; it does not remove typed models.

## Native language package options

`PackageOptions` is shared by Go, Python, PHP, Java, .NET, and Elixir.

| Field | Default | Meaning |
| --- | --- | --- |
| `package_name: Option<String>` | derived from API title | Distribution/module/package identity. Each target normalizes it for its ecosystem. |
| `client_style: SdkClientStyle` | `Namespaced` | Adds resource facades such as `client.contacts.list(...)`; `Flat` uses conventional direct methods. |

Use the matching builder to apply it: `.go_options(...)`,
`.python_options(...)`, `.php_options(...)`, `.java_options(...)`,
`.dotnet_options(...)`, or `.elixir_options(...)`.

```rust
let profiles = ProfileSet::new("sdk")
    .go()
    .go_options(PackageOptions::flat())
    .python()
    .python_options(PackageOptions {
        package_name: Some("relevate-email".into()),
        ..PackageOptions::default()
    });
```

Rust currently uses its fixed Reqwest package profile and does not take
`PackageOptions` through `ProfileSet`.

## Mock-server options

`MockServerOptions` controls the generated Docker package.

| Field | Default | Meaning |
| --- | --- | --- |
| `image: String` | `httpmock/httpmock` | Docker base image. Pin a tag or digest for a reproducible release. |
| `port: u16` | `5000` | Container and default published port. The generated `.env.example` exposes it as `KAJI_MOCK_PORT`. |

```rust
.mock_server_options(MockServerOptions {
    image: "httpmock/httpmock:0.8.0".into(),
    port: 4010,
})
```

See [contract mocking](mocking.md) for route fixtures and scenarios.

## Low-level Rust profiles

`kaji_core::SdkProfile` is available for advanced Rust/TypeScript composition
through `generate_sdks`. Prefer `ProfileSet` unless you specifically need to
build the profile structs yourself.

| Field | Rust constraint | TypeScript constraint |
| --- | --- | --- |
| `language` | `SdkLanguage::Rust` | `SdkLanguage::TypeScript` |
| `output_dir` | required relative package directory | required relative package directory |
| `package_name` | optional | optional |
| `client_name` | optional | optional class name |
| `client_style` | flat or namespaced | flat or namespaced |
| `surface` | client | raw or client |
| `transports` | exactly `[SdkTransport::Reqwest]` | exactly one: `[Fetch]` or `[Axios]` for structured output |
| `style` | `SdkStyle::Native` | `SdkStyle::Structured` for the maintained layout |
| `group_by_tag` | `false` | defaults to `true` |

`SdkProfile::rust("sdk/rust")` and `SdkProfile::typescript("sdk/ts")` start
from the supported defaults. Mixing Fetch and Axios in one structured package
is rejected; select both first-party TypeScript targets instead when you need
both packages.

The corresponding core functions are:

```rust
use kaji_core::{Api, SdkProfile, generate_sdks};

let tree = generate_sdks(
    &api,
    &[
        SdkProfile::rust("sdk/rust"),
        SdkProfile::typescript("sdk/typescript-fetch"),
    ],
)?;
```

`kaji::custom(profile)` is a small identity helper for passing an advanced
`SdkProfile` through code that otherwise works with first-party profiles. It
does not register a new `ProfileSet` target by itself.

For direct OpenAPI JSON/YAML input at the core layer, use
`kaji_core::generate_openapi_document_sdks`. The first-party `kaji` crate
offers `generate_openapi` and `generate_openapi_file` for all maintained
language targets and should be preferred for multi-language releases.

## What Kaji does not configure yet

Kaji does not yet ship a stable config-file format, a CLI, a Node compatibility
layer, or a generator-time arbitrary custom HTTP transport injection. Those
are deliberately not implied by these builders. Consumers can still use the
native transport injection/hooks that each generated SDK exposes.
