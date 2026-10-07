# Kaji feature catalog

Use this page to choose what to generate and what to verify before shipping it.
Kaji is a generator and extensible source toolchain: SDK packages, runtime policies,
API artifacts and delivery files belong to you. Optional plugins are selected in
`kaji.json` or through the typed Rust library API.

**Implemented** means available in source. **Verified** means the stated tests
executed. **Limited** identifies a supported subset. **Prepared** means workflow
code exists without a real external run. None of these implies every contract is
supported. Version 0.5.0 is on the branch; use a source build until published.

## Start with your goal

| Goal | Select | Read next |
| --- | --- | --- |
| Ship SDK packages | One package per language, normally `sdk` | [Quickstart](cli/quickstart.md), [recipes](cli/config.md) |
| Ship your HTTP policy by default | Package `middleware` and authored source | [SDK customization](sdk-customization.md) |
| Let customers change request behavior | Native middleware or HTTP-driver injection | [Runtime middleware](guides/runtime-middleware.md) |
| Compose or replace generation | Typed provider/consumer handles | [Plugin development](typed-plugins.md) |
| Review contract changes and release SDKs | Release metadata and editable Actions | [Automation](sdk-automation.md), [publishing](sdk-publishing.md) |
| Build frontend integrations | TypeScript query/schema/mock consumers | [TypeScript helpers](guides/typescript-helpers.md) |
| Export a collection or provider | `postman` or typed `terraform` package | [API artifacts example](../examples/api-artifacts/README.md) |
| Expose tools to an AI client | MCP artifact or local Kaji MCP server | [MCP server](mcp-server.md) |

## Contracts and generation

| Capability | Status and scope | Guide |
| --- | --- | --- |
| Swagger 2.0; OpenAPI 3.0/3.1 | Local JSON/YAML and downloaded contracts; normalization through the bundled Go compiler | [Compiler](openapi-compiler.md) |
| OpenAPI 3.2 | Ordinary contracts, QUERY, nullable bodies and webhook-only specs; unsupported 3.2 additions fail before writes | [Compiler](openapi-compiler.md) |
| External references, recursive schemas | Local file references and recursive schemas; remote references require explicit bundling; native representation varies by target | [Shared fixtures](shared-sdk-fixtures.md) |
| Operation parameters and body/response media | Path/query/header inputs, declared media, examples and security alternatives carried into the neutral model | [Architecture](architecture.md) |
| Package-specific settings | Multiple packages/languages/providers; naming, client style, manifests, versions and plugins | [Configuration](configuration.md) |
| Typed plugin graph | Named capabilities, explicit handles, ordering, ambiguity/cycle checks before emission | [Typed plugins](typed-plugins.md) |
| Independent SDK providers | TypeScript models/transport/operations/client; Rust/Go native contracts. Other targets have narrower consumer contracts | [Native providers](native-sdk-providers.md) |
| Source customization | Add, explicit replace, guarded patch; package scope; customer create-once files | [Customization](sdk-customization.md) |
| Safe regeneration | Preflight all packages, ownership manifest, unchanged stale-file removal, edited-file protection | [Regeneration](safe-regeneration.md) |
| Read-only drift and inspection | `generate --check`, SDK doctor and inspectable compiler/package artifacts | [CLI](cli.md), [verification](verification.md) |
| Large public contract regression | Pinned Graph smoke plus six full official contracts, checksum verification and a manual ten-language native matrix | [Large specs](large-specs.md) |
| Arbitrary custom languages | Rust library `Language`/`Plugin` implementations and optional delivery metadata. CLI needs explicit registry integration | [Library plugins](library/plugins.md) |
| Ejectable generator sources | `kaji eject` exports rebuildable renderers, runtime sources and plugin interfaces, with a SHA-256 manifest | [Own the sources](source-customization.md) |

## SDK targets

All ten targets generate ordinary native packages with typed operations/models and
language-specific HTTP APIs. This table records meaningful differences rather
than imposing one runtime interface on every language.

| Target | Package/runtime | Pagination forms | Automatic retries | Shared wire evidence |
| --- | --- | --- | --- | --- |
| TypeScript | npm; Fetch or Axios; full or raw functions | Cursor, page, offset, next URL | Yes | Fetch 17/17; separate Axios probes |
| Python | Python package; urllib sync, opt-in httpx async | Cursor, page, offset, next URL | Yes | Sync 17/17; separate async probes |
| Go | Go module; native HTTP driver | Cursor, page, offset, next URL | Yes | 17/17 |
| Rust | Cargo; native transport contract | Cursor, page, offset, next URL | Yes | 17/17 |
| Java | Maven; HttpClient | Cursor, page, offset, next URL | Yes | Native harness 15/17 |
| C#/.NET | .NET; HttpClient/DelegatingHandler | Cursor, page, offset, next URL | Yes | Native harness 17/17 |
| PHP | Composer; PSR-18 | Cursor, page, offset, next URL | Yes | Native harness 15/17 |
| Elixir | Mix; Finch | Cursor, page, offset, absolute same-origin URL | Yes | Native harness 15/17 |
| Ruby | Ruby package; native HTTP | Page | Opt-in | 16/17; retries opt-in |
| Swift | Swift package; Foundation | Cursor, page, offset, next URL | Opt-in | 17/17; retries opt-in |

The corpus contains 17 scenarios per target. Unsupported scenarios are recorded,
not counted as passes. See the [executable manifest](../packages/runtime-contract/scenarios.json)
and [requirements](../packages/runtime-contract/README.md). Pagination bindings
and selector restrictions are in the [pagination guide](guides/pagination.md).

## Runtime behavior

| Feature | What you can do | Boundary |
| --- | --- | --- |
| Authentication | Configure declared bearer, Basic or API-key credentials through the native client | Scheme/binding details vary; use generated package docs |
| OAuth client credentials | Python sync/async and opt-in Go/Ruby/TypeScript/Rust/Java/C# providers, coordinated refresh and bounded unauthorized recovery | Replay scope and cancellation follow each native driver; Swift/PHP/Elixir require supplied credentials |
| Consumer middleware | Rewrite requests/responses, short-circuit or recover through native supported hooks | Java/C#/PHP use native HTTP decorators; hook signatures differ |
| Bundled author middleware | Ship policy modules and register them by default during generation | Customers need no middleware registration for bundled policies |
| Retry and backoff | Replay safe operations with bounded attempts and server delay handling | Ruby/Swift default to one attempt; retry settings enable replay-safe retries |
| Idempotency keys | `x-kaji-idempotency` or per-package rules; secure UUIDs, caller overrides, operation-scoped header | Requires server semantics; blank keys do not protect replay; PATCH requires a key |
| Pagination | Lazy helpers reuse the actual operation, transport/auth and middleware | Helpers yield pages; forms/body controls differ by target |
| Per-call headers and timeouts | TypeScript/Ruby request options; Go context options; Python/Rust/Java/C# scoped clients | [Native timeout scope differs](guides/request-controls.md); Swift/PHP/Elixir use driver/client settings |
| Cancellation | Native context/signal/task cancellation; tests cover supported transports and pagers | Custom drivers retain native cancellation responsibilities |
| Structural response checks | Opt-in TypeScript, Go, Python and Ruby checks; native model decoders also reject some invalid shapes | Java/PHP/Elixir shared cases remain permissive |
| Errors and raw results | Declared native errors and response envelopes where supported | Inspect each target's surface; raw response/stream ownership differs |
| Streaming and file media | Selected SSE/binary forms; multipart upload APIs in TypeScript/Go/Python/Rust/Java/C#/Swift | Buffered upload limits and shapes vary; Swift requires closed named roots. Ruby/PHP/Elixir need upload adapters. Shared corpus is not exhaustive |
| Forward-compatible models | Opt-in Java/C#/Swift open enums and Java/C# optional presence wrappers; transparent named scalar/union JSON; Rust opt-in unmatched union fallback; selected unknown-property and nullable handling; TypeScript int64 string/bigint options | Unknown enum/union roundtrip and omitted-vs-null behavior are not universal |
| Webhook verification | Opt-in verifiers in all ten SDKs: raw-body HMAC verification, timestamp checks and secret rotation | Native probes passed across ten languages; Swift Linux crypto unverified; HMAC v1, no durable replay store |

Start from [generated SDKs](generated-sdks.md), then follow
[middleware](guides/runtime-middleware.md), [idempotency](guides/idempotency.md),
[pagination](guides/pagination.md) and [verification](verification.md).

## Frontend, API tools and documentation

| Output | Select | Scope |
| --- | --- | --- |
| TanStack React Query | `tanstack-react-query` | Query keys/GET hooks and mutation hooks using generated operations |
| TanStack Vue Query | `tanstack-vue-query` | Native Vue query/mutation helpers |
| SWR | `swr` | GET hooks |
| Zod | `zod` | Component/request/response schemas |
| Faker | `faker` | Bounded model factories; not business-valid data |
| MSW | `msw` | Editable MSW v2 handlers |
| Cypress | `cypress` | Smoke-test scaffolding needing project fixtures/assertions |
| Contract mock | `mock` server | Deterministic HTTP/Docker mock; conditional `x-kaji-mock` scenarios |
| API CLIs | `typescript-cli` or `rust-cli` | API-specific command tools; TypeScript CLI has optional OAuth configuration |
| Symfony integration | `symfony` | Wraps a generated PHP SDK |
| ReDoc | Artifacts `redoc` | Documentation entry point for an OpenAPI contract |
| MCP | Artifacts `mcp` | Tool manifest for an integration; local Kaji MCP server is separate |
| API reference | Package `api_reference: true` | Regenerated package-local operation reference |

Read [auxiliary generators](auxiliary-generators.md), [mocking](mocking.md),
[API CLIs](typescript-cli.md) and [artifact guides](guides/artifacts.md).

## Postman and Terraform

| Capability | Status | Guide |
| --- | --- | --- |
| Postman Collection 2.1 | Implemented: operation folders, parameters/media/auth, examples, stable IDs and secret redaction | [Postman](postman.md) |
| Postman environment | Create-once template; customer credentials are not overwritten | [Postman](postman.md) |
| Collection checks | Official pinned schema validation; bounded local Newman execution | [Execution](../packages/postman-execute/README.md) |
| Postman remote sync | Collection and environment existing UIDs; secrets/manual environment variables preserved; read-only check and reviewed-hash publication; mocked tests, live service unverified | [Sync helper](../packages/postman-sync/README.md) |
| Typed Terraform provider | Framework CRUD bindings, typed nested plan/state, single/composite import, authentication, drift and diagnostics | [Terraform](terraform-provider.md) |
| Terraform data sources | Supported single-entity reads | [Terraform](terraform-provider.md) |
| Terraform native verification | Framework object tests and real local CLI lifecycle against a mock | [Verification](verification.md) |
| Advanced Terraform lifecycle | Nested objects/lists/maps, composite IDs and explicit root-rename upgrades; bounded read-GET lifecycle polling; general type migrations remain unsupported | [Provider boundaries](terraform-provider.md) |
| Terraform registry release | Opt-in editable GoReleaser/signing/workflow scaffold with explicit namespace; not activated or live-verified | [Terraform](terraform-provider.md) |

## GitHub delivery and publication

| Capability | What is implemented | What you configure |
| --- | --- | --- |
| Repository layouts | API repository, combined SDK repository, or one repository per language | Existing repositories and routing |
| SDK sync | Generated update PRs, owned-source checks and preserved destination versions | Credentials, branches and review policy |
| Spec relay/fetch | Source provenance, reviewable relay PRs and remote source workflows | Source/destination authorization and URLs |
| SDK checks | Editable per-language Actions; native build/test commands | Toolchains and your API-specific tests |
| Release sizing | API diff notes and Release Please version/changelog workflow | Package metadata, release branches |
| Publishing | Immutable-tag checks and supported registry publishers; custom command-vector publisher | Registry registration and protected environments |
| GitHub App | Manifest/setup plus self-hosted OIDC broker and scoped short-lived tokens | App installation, hosting and policies |
| Remote status | Inspect repository setup, workflows and delivery state | Authorized read access |
| Live delivery test | Gated disposable workflow is prepared | No installation or real publication was run |

See [repository automation](sdk-automation.md), [GitHub Actions](github-actions.md),
[publishing](sdk-publishing.md), [App](github-app.md) and
[broker](github-app-broker.md). Generated workflows are editable sources; workflow
presence is not evidence that registry trust or live delivery works.

## Tests and confidence

The current verification baseline includes 418 passing workspace tests, 163 native
wire scenarios across all ten runtimes, 37 runner/delivery/sync tests and an
installed TypeScript package consumer check. Ignored native probes need explicit
toolchain execution; they are not passes. Counts describe the recorded baseline,
not a permanent CI badge.

Snapshots protect output shape; native compilation checks types; fake transports
check request/response behavior; loopback tests check native HTTP; real registry
tests establish delivery. Keep those evidence levels separate. Optional operation tests in all ten SDK languages generate bounded fake-driver checks and explicit skip diagnostics.
Complex contracts, SSE/uploads and real API semantics still need focused tests.

[Verification](verification.md) has commands and prerequisites. The
intentional; behavior and evidence should be comparable where the capabilities
overlap.
