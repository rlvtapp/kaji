# Changelog

## Unreleased

## 0.5.0 — 2026-10-07

- Add optional Swift/PHP/Elixir OAuth providers and native call scopes, and
  buffered Ruby/PHP/Elixir multipart builders with native regression probes.
- Resolve bounded public-HTTPS reference closures with provenance invalidation,
  and retain OpenAPI 3.2 custom HTTP methods with unsafe-by-default retries.
- Add a sourced SDK-generator comparison and Kaji ownership/plugin positioning.


### Added

- Rebuildable `kaji eject` source bundles with SHA-256 manifests, preserving the
  plugin architecture; an integration test edits a renderer, rebuilds and generates.
- Bounded OpenAPI 3.2 support including QUERY, local reference closure hashing and
  recursive schemas, with explicit diagnostics for unsupported new constructs.
- Optional TypeScript, Rust, Java and C# OAuth client-credentials providers with
  coordinated refresh, cancellation and bounded safe unauthorized replay.
- Bounded multipart upload APIs in Go/Python/Rust/Swift, and richer Java/C# JSON,
  repeated-array and mixed-media multipart bodies, with native MIME/retry/cancellation probes.
- Native per-call headers and timeout controls in TypeScript, Ruby, Go, Python,
  Rust, Java and C#, preserving client defaults and native cancellation.
- Opt-in Rust unmatched union values retain raw JSON; strict decoding remains the default.
- Six checksum-pinned full official contracts and a manual ten-language native
  verification workflow; Go request/model naming now handles full OpenAI and GitHub.


- Explicit Terraform lifecycle polling through validated read GET operations,
  bounded scalar criteria, attempts/deadlines, cancellation and recoverable state.

- Native-verified Java/C#/PHP/Elixir probes and pinned public contract checks.
  Fixed Java/C# inline response imports, Java presence constructors, Elixir string
  enum typespecs and generated Finch request assertions. Rust native tests now
  allow dependency downloads on fresh runners, with explicit offline mode.

- Typed Terraform nested objects/lists/maps, composite identities including configured
  parent IDs, and explicit versioned root-field state renames, with native Framework tests.
- Swift incremental cancellable SSE and opt-in open enums; Rust/C#/Swift same-origin
  URL pagination and Swift offset pagination.
- Java/C# optional presence wrappers and scalar/binary multipart bodies; opt-in Ruby
  OAuth client credentials with coordinated refresh and bounded unauthorized replay.
- Checksum-pinned public OpenAPI compilation across ten CI toolchains, corruption
  tests and portable native test caches. Speakeasy Terraform comparison documents
  polling, transformations and general migrations still outside the supported subset.

- Full feature catalog with SDK target differences, configuration entry points and
  verification limits, linked from the main README and documentation index.
- Opt-in Standard Webhooks HMAC v1 verifiers across ten SDK targets, with shared
  canonical vectors, timestamp/rotation checks and native probes. Go gains cached
  OAuth client credentials with coordinated refresh and bounded safe 401 replay.
- Optional generated operation tests in all ten SDK language recipes, with bounded
  structural fixtures, fake native HTTP drivers and explicit unsupported diagnostics.
- Opt-in Ruby/Swift replay-safe retries, bounded server delay handling and backoff
  cancellation; the default remains one attempt.
- Elixir offset and absolute same-origin URL pagination; continuations reject origin
  changes before authentication. C# gains opt-in forward-compatible open enums.
- Reviewed Postman environment synchronization preserves remote secrets and manually
  added variables, with bounded responses, hash review and verified read-back.
- Java/C# named scalar and union wrappers preserve their underlying JSON value
  during round-trip serialization. Opt-in presence wrappers distinguish optional null from omission.
- Swift cursor pagination, checked-in complex-model OpenAPI regressions and a
  checksum-pinned Microsoft Graph native test with manual read-only CI.
- Reviewed-hash Postman collection synchronization and optional editable Terraform
  registry-release scaffolding, explicit namespace and injected provider version.

- Expanded 17-scenario native wire corpus, installed TypeScript ESM package checks,
  OAuth/cancellation and nested-model probes, atomic regeneration conflict tests,
  and delivery phase failure/cleanup tests. Native CI runs the additional probes.

- Opt-in `x-kaji-idempotency` and per-package recipe rules across ten SDK targets,
  secure automatic UUIDs, native caller overrides, retry-stable keys, and generated
  API reference documentation. Custom plugins receive the resolved policy.
- Bounded `retry-after-ms` handling and HTTP-date retry delay improvements in native
  runtimes, with replay safety tests. PATCH now requires an idempotency key.

- A shared executable HTTP runtime contract with ten native harnesses and explicit
  unsupported-scenario reporting, wired into the language CI matrix.
- Optional Python and Go generated operation smoke tests, a package-local API
  reference, and a standalone custom plugin composition example.
- Python sync/async and Ruby opt-in structural response validation, plus shared
  page pagination in Python/TypeScript/Go with RFC 6901 selectors.
- Terraform single-entity data sources and real local Terraform CLI lifecycle
  tests; executable Postman collection tests through pinned Newman.
- Read-only SDK doctor/inspection, structured API diff notes carried into Release
  Please, and a gated disposable delivery workflow prepared without publication.
- Ownership-aware regeneration and check mode with stale generated-file cleanup,
  customer edit protection, preserved create-once files, and npm manifest merging.
- SDK author customization through bundled runtime HTTP middleware across native
  targets, package source overlays, explicit replacement, and guarded patches.
  Bundled middleware registers by default without SDK consumer configuration.
- Named typed plugin contracts and provider composition for TypeScript and native
  Rust/Go, with independent model, transport, operation, and client providers.
- Package delivery metadata, independent SDK versions, API diff release sizing,
  destination-owned version preservation, and safe generated SDK pull requests.
- SDK automation commands for scaffolding, synchronization, checks, publishing,
  releases, remote status, spec relay, and reviewable destination setup installation.
- One repository per language using `--repository-pattern 'OWNER/api-{lang}'`,
  isolated generation jobs, destination-scoped authentication, and editable action
  sources for each repository's checks and release workflow.
- Release Please and immutable-tag publishing workflows, supported registry OIDC
  integrations, GitHub App manifests, and a self-hosted OIDC token broker.
- Editable GitHub Actions for SDK checks, publishing, and spec synchronization.
  Spec relay uses review PRs and protects manual changes and source provenance.
- Postman Collection 2.1 generation with request/response examples, parameter and
  media mappings, authentication variants, stable IDs, secret redaction, and
  create-once environment templates. Includes official-schema validation action.
- Typed Terraform Plugin Framework provider generation with explicit or inferred
  CRUD bindings, scalar state/schema mapping, import, authentication, safe HTTP
  transport, lifecycle handling, diagnostics, and generated native transport tests.
- Shared pagination plans and SDK fixture infrastructure; native page-number
  helpers across all ten targets, preserving each language's iteration API.
  Rust pagers now preserve optional starts and avoid saturating-counter loops.
- SDK author guides and examples covering generation, customization, publishing,
  GitHub automation, per-language repositories, Postman, and Terraform.

### Changed

- Unsupported OpenAPI 3.2/future versions fail before modifying compiler artifacts.
  Swift recursive models compile as immutable classes; Swift/Java/C# report model
  identifier collisions and reject unsupported multipart requests. Java/C# SSE
  parsers join multiline data and ignore event metadata.

- Generated TypeScript ESM imports resolve in installed Node packages. SDK runtime
  fixes preserve nullable/omitted fields, repeated query arrays, encoded paths,
  nested model decoding and cancellation behavior. Rust escapes reserved names.

- TypeScript Fetch/Axios and Go clients expose opt-in structural response checks.
  Consumer middleware tests cover rewrites, short circuits, error propagation,
  retries, and cancellation; TypeScript checks include middleware-produced results.
- TypeScript preserves wide integers with string/bigint support. Model handling
  improves unknown fields and nullable values; Java supports open enums.
- Python SDKs gain async/httpx support, OAuth client credentials, and webhook HMAC.
- OpenAPI artifacts preserve additional server, tag, parameter, response-example,
  and encoding metadata for artifact generators.
- CI validates Postman collections and generated Terraform providers alongside
  SDK and compiler checks. Root release metadata is synchronized across Rust,
  npm distribution packages, and Python.

### Scope and verification

- Postman workspace synchronization and publication are not included.
- Typed Terraform supports a bounded typed schema subset; independent collection data
  sources, arbitrary asynchronous job mappings, general type migrations and registry
  publishing remain future work. The legacy raw-JSON Terraform API remains.
- Terraform lifecycle tests run against the native Go Plugin Framework; Terraform
  CLI lifecycle runs against a local mock. Some native SDK probes require external toolchains.
- Workflow scaffolding and setup PR support do not create GitHub repositories,
  install Apps, or configure registry trust automatically.

## 0.4.0 — 2026-10-05

### Added

- Merge existing npm manifests during generation, preserving custom fields,
  scripts, dependency ranges, and dependency categories while appending missing
  requirements. Recipe package identity and generated exports remain authoritative.
- Symfony integration packages wrapping the generated PHP SDK.
- Interactive terminal prompts for required parameters, simple request bodies,
  and base URLs in generated TypeScript and Rust API CLIs.
- Masked credential prompts, styled terminal status output, and non-interactive
  behavior for pipes and `--json`.
- OpenAPI-derived command reference files under `references/<command-group>.md`
  in generated CLI packages.

### Changed

- Generated TypeScript packages use the published TypeScript 5.9 compiler line.
- Generated TypeScript headers use plain “Generated by Kaji” attribution.
  No domain ownership or availability is implied.


## 0.3.0 — 2026-09-30

### Added

- Added first-class Ruby and Swift SDK generators. Ruby output is a Ruby 3.1+
  gem using `Net::HTTP`, `URI`, and `JSON`; Swift output is a Swift 5.9+ Swift
  Package Manager library using `URLSession` and `Codable`.
- Added generated TypeScript and native Rust API CLI targets. They derive nested
  commands, request flags, help text, authentication, OAuth flows, API-key
  profiles, and environment-variable support from OpenAPI.
- Added a no-Docker native mock server: `kaji mock serve`. It serves
  schema-shaped dynamic responses, evaluates `x-kaji-mock` scenarios, and
  exposes health and request-log endpoints for people and agents.
- Added `kaji check` contract diagnostics, JSON output, severity controls, and
  reviewable baselines for incrementally improving existing specifications.
- Added path slicing with repeatable `--include-path` and `--exclude-path`
  selectors for direct generation and `kaji.json` recipes.
- Added reproducible generation locks that record secret-free inputs, generator
  settings, selected operations, and artifact hashes.
- Added `kaji show` for inspecting the exact operation slice selected from a
  local contract, including JSON output for automation and agents.
- Added `kaji update` to replay direct-generation locks when their local source
  or compiler-artifact input changes, with a `--force` override.
- Added `kaji auth login`, `logout`, and `status` for named, environment-backed
  credential profiles in authenticated remote OpenAPI inputs. Profiles retain
  only the environment-variable name, never a token value.
- Added `kaji discover` and safe `kaji download` support for the APIs.guru
  directory, with relevance-ranked and JSON output for automation.
- Added `kaji mcp generator`, exposing local generation controls to MCP clients.
- Added runnable examples for API CLIs, native mocks, reproducible generation,
  agentic generation, Ruby, and Swift.

### Changed

- Made `kaji-plugin-csharp` and `csharp` the canonical C#/.NET generator
  surface. The `dotnet` crate and selector remain compatibility aliases.
- Extended CLI configuration, JSON Schema, MCP discovery, examples, and
  documentation for C#, Ruby, and Swift targets.
- Added authenticated remote OpenAPI inputs with custom headers, Basic, Bearer,
  and environment/profile-backed secret values.
- Improved generated Go retry behavior and TypeScript generated-client
  ergonomics.
- Added CI coverage for the canonical C# target and generated Rust SDK builds
  on fresh runners.
- CI now builds Kaji once, generates a single cross-language fixture, and
  validates the generated Rust, TypeScript, Go, Python, PHP, Java, C#, Elixir,
  Ruby, and Swift packages in a native-toolchain matrix.

### Fixed

- Fixed generated C# operations and disposal code so emitted projects compile
  with .NET 8.
- Fixed Ruby model deserialization for schemas that retain additional
  properties.
- Fixed generated Java facades to inherit their internal operation partitions
  through fully-qualified package names.
- Fixed generated Elixir SDKs to use distinct chunk modules and compile without
  warnings.

- Established the standalone Kaji Rust workspace.
- Added first-party SDK targets and language-neutral contract mocks.
