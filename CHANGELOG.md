# Changelog

## Unreleased

## 0.5.0 — 2026-10-08

### Added

#### Generation and customization

- Native symbol allocation preserves distinct wire keys across keywords, normalized
  names, case-insensitive paths, aliases and recursive model graphs.
- Shared TypeScript response descriptors reduce repeated schema expansion; Stripe's
  measured package shrank from 147.3 MB to 3.58 MB and compiles with the default heap.
- Bounded Ruby modules, Go response registries, Swift operation/resource extensions,
  Rust/TypeScript CLI commands and Zod/Faker/MSW/Cypress modules. Auxiliary consumers
  accept `max_file_bytes`; query consumers accept `max_operations_per_file`.
- React/Vue query and mutation option/key factories with typed framework overrides,
  explicit cache scopes, HTTP cancellation and callback context. Runtime tests cover
  cache reuse/invalidation, SDK failures, mutation callbacks and abort propagation.
- Idempotency key generation follows allocated header argument names, preserving
  explicit keys and stable retries when native names collide.
- Corpus size reporting separates metadata from source and records largest files
  and configurable warnings. Additional native regressions and CI checks cover
  split imports, regeneration cleanup, large models and framework consumers.

- Documented a prioritized generator completion backlog covering native corpus
  failures, output size/splitting, framework APIs and artifact acceptance checks.

- Migration from Stainless, Fern and Speakeasy through `poolster migrate`, direct
  generation from supported vendor configurations, and automatic normalization
  of supported OpenAPI annotations. Originals are preserved; unsupported settings
  receive explicit manual-review diagnostics.
- Bundled runtime HTTP middleware that registers automatically in generated SDKs,
  plus package source overlays, explicit replacements and guarded patches.
- Named typed plugin contracts and independent model, transport, operation and
  client providers for TypeScript and native Rust/Go composition.
- Rebuildable `poolster eject` source bundles with SHA-256 manifests, preserving the
  plugin architecture and supporting custom generator builds.
- Ownership-aware regeneration and check mode with stale-file cleanup, customer
  edit protection, create-once files and npm manifest merging.
- Optional generated operation tests in all ten SDK languages, with bounded
  structural fixtures, fake native HTTP drivers and unsupported-case diagnostics.
- Package-local API references and a standalone custom plugin composition example.

#### OpenAPI and SDK runtimes

- OpenAPI 3.2 whole-query and parameter content serialization, buffered
  JSON-sequence/NDJSON/JSONL bodies, ordered nested multipart plans, device
  authorization metadata, tag hierarchy and `$self` identity.
- QUERY and custom HTTP methods with unsafe-by-default retries, local and bounded
  public-HTTPS reference closure resolution, and source provenance invalidation.
  Custom plugins retain typed content and expanded OpenAPI metadata.
- Opt-in `x-poolster-idempotency` and per-package rules across ten SDK targets, with
  secure automatic UUIDs, caller overrides, retry-stable keys and API documentation.
  Custom plugins receive the resolved policy.
- Shared pagination plans and native page-number helpers across all ten targets,
  preserving language-specific iteration APIs. Additional cursor, offset and
  same-origin URL pagination capabilities are documented per target.
- Consumer runtime middleware and opt-in structural response checks in TypeScript
  Fetch/Axios, Go, Python sync/async and Ruby.
- Optional OAuth client-credentials providers across the ten SDK targets, with
  coordinated refresh and bounded safe unauthorized replay. Native request scopes
  expose per-call headers, deadlines and cancellation where supported.
- Opt-in Standard Webhooks HMAC v1 verification across ten targets, with timestamp,
  key-rotation and canonical-vector checks.
- Replay-safe Ruby/Swift retries, bounded `Retry-After` and `retry-after-ms` delays,
  HTTP-date handling and cancellable backoff. The default remains one attempt.
- Multipart upload builders across native targets, including buffered
  Ruby/PHP/Elixir bodies and richer Java/C# scalar, binary, JSON and repeated-array
  parts. Swift adds incremental cancellable SSE.
- Forward-compatible model options: open enums across supported targets, Rust
  unmatched union values retaining raw JSON, schema-allowed unknown fields,
  Java/C# scalar and union wrappers, Java/C# optional-presence wrappers, and
  Python/Ruby/PHP/Elixir explicit-null helpers. Strict decoding remains the default
  where applicable; see the feature catalog for target differences.

#### Postman and Terraform

- Postman Collection 2.1 generation with request/response examples, media and
  parameter mappings, authentication variants, stable IDs, secret redaction and
  create-once environment templates. Includes official-schema validation and
  executable collection tests through pinned Newman.
- Reviewed-hash Postman collection and environment synchronization, preserving
  remote secrets and manually added variables, with bounded responses and verified
  read-back.
- Typed Terraform Plugin Framework providers with explicit or inferred CRUD
  bindings, authentication, safe HTTP transport, import and lifecycle diagnostics.
- Terraform nested objects/lists/maps, composite identities with configured parent
  IDs, single-entity data sources and explicit versioned root-field state renames.
- Explicit Terraform lifecycle polling through validated read GET operations,
  bounded scalar criteria, attempts/deadlines, cancellation and recoverable state.
- Optional editable Terraform Registry release scaffolding with an explicit
  namespace and injected provider version.

#### SDK delivery and documentation

- SDK automation for scaffolding, synchronization, checks, publishing, releases,
  spec relay, remote status and reviewable destination setup installation.
- Independent SDK package versions, API diff release sizing and notes, preservation
  of destination-owned versions, and safe generated SDK pull requests.
- One repository per language through `--repository-pattern 'OWNER/api-{lang}'`,
  isolated generation jobs, destination-scoped authentication and editable checks,
  release and publishing actions for each repository.
- Release Please and immutable-tag publishing workflows, supported registry OIDC
  integrations, GitHub App manifests and a self-hosted OIDC token broker.
- Editable GitHub Actions for SDK checks, publishing and spec synchronization;
  spec relay uses review PRs and protects manual changes and source provenance.
- Read-only SDK doctor/inspection and a gated disposable delivery workflow
  prepared without publication.
- SDK author guides and examples for generation, customization, forward-compatible
  models, OpenAPI 3.2, publishing, GitHub delivery, Postman and Terraform.
- Full feature catalog and sourced generator comparison, linked from the main
  README and documentation index.

### Changed

- Python SDKs support async/httpx clients. TypeScript supports wide integers through
  string/bigint representations; model decoding better preserves nullable values
  and schema-allowed unknown fields.
- Compiler artifact revisions invalidate caches when the normalized representation
  changes. Source-node extraction retains fields omitted by parser models.
- Unsupported future OpenAPI versions and malformed encodings fail explicitly;
  version checks run before modifying compiler artifacts. Model identifier
  collisions and unsupported multipart requests receive explicit diagnostics.
- PATCH replay now requires an idempotency key.
- Root release metadata stays synchronized across Rust, npm and Python. Workspace
  version updates are regression-tested; automatic publication requires explicit
  enablement.

### Fixed

- TypeScript ESM imports resolve in installed Node packages. Runtime fixes preserve
  repeated query arrays, encoded paths, nested model decoding and cancellation.
- Go request/model naming handles the pinned full OpenAI and GitHub contracts;
  Rust escapes reserved names and pagers avoid saturating-counter loops.
- Resolve repeated source operation IDs deterministically and lift nested local
  schema targets into emitted components. Preserve valid YAML block scalar tabs
  and Unicode line separators, and treat vendor extension references as data.
- Go allocates model symbols around runtime, service, enum and normalization
  collisions. Custom JSON methods preserve wire keys that cannot use struct tags.
- Swift recursive models compile as immutable classes. Java/C# inline response
  imports, Java presence constructors, C# renamed presence properties, Ruby
  `value` fields and unknown-property collisions are corrected.
- Java/C# SSE parsers join multiline data and ignore event metadata. Elixir string
  enum typespecs and generated Finch request assertions are corrected.
- CI builds the compiler before CLI integration tests, selects Swift 6 with its Go
  dependency, and loads Elixir probe dependencies. Rust native checks support fresh
  dependency downloads and explicit offline mode.

### Breaking changes

- Rust generator API: `HttpMethod` now includes `Custom(String)` and is no longer
  `Copy`. Clone stored values when needed, or borrow them for `as_str()`.
- Manually constructed `OAuthFlow` values must include
  `device_authorization_url: None` when no device endpoint is declared.

### Testing and verification limits

- Add a shared executable HTTP contract with ten native harnesses and a 17-scenario
  wire corpus, plus OAuth/cancellation, nested-model, installed TypeScript package,
  regeneration-conflict and delivery failure/cleanup checks.
- Add checksum-pinned Microsoft Graph and six full official contracts, plus a
  205-contract APIs.guru corpus, including five Azure services, with per-phase
  logs, timeout/failure reports, cleanup tests, verified companion reference
  files and manual native-language workflows. See
  [large-spec testing](docs/large-specs.md). All 205 pass local Go generation and
  native compilation. A subsequent 2,050-case cross-language run records
  generation/native failures in the other nine targets, with per-contract
  diagnostics and a prioritized fix list in the
  [compatibility baseline](docs/guru-compatibility.md). Workflow availability does not imply
  every contract passes in every language.
- Postman collection/environment sync is included; whole-workspace synchronization
  and publication are outside this release's scope.
- Terraform supports a bounded typed schema subset. Independent collection data
  sources, arbitrary asynchronous job mappings and general state type migrations
  remain unsupported. Registry release files are scaffolding; live publication
  is not verified. The legacy raw-JSON Terraform API remains available.
- Terraform Framework tests and local-mock Terraform CLI lifecycle tests run
  alongside SDK and compiler checks. Some native probes require external toolchains.
- Delivery scaffolding does not create repositories, install Apps or configure
  registry trust automatically. Live delivery remains a prepared, gated workflow.
- Migration reuses supported configuration and annotations; proprietary templates,
  unresolved overlays and unsupported vendor behavior still require review.

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
- Generated TypeScript headers use plain “Generated by Poolster” attribution.
  No domain ownership or availability is implied.


## 0.3.0 — 2026-09-30

### Added

- Added first-class Ruby and Swift SDK generators. Ruby output is a Ruby 3.1+
  gem using `Net::HTTP`, `URI`, and `JSON`; Swift output is a Swift 5.9+ Swift
  Package Manager library using `URLSession` and `Codable`.
- Added generated TypeScript and native Rust API CLI targets. They derive nested
  commands, request flags, help text, authentication, OAuth flows, API-key
  profiles, and environment-variable support from OpenAPI.
- Added a no-Docker native mock server: `poolster mock serve`. It serves
  schema-shaped dynamic responses, evaluates `x-poolster-mock` scenarios, and
  exposes health and request-log endpoints for people and agents.
- Added `poolster check` contract diagnostics, JSON output, severity controls, and
  reviewable baselines for incrementally improving existing specifications.
- Added path slicing with repeatable `--include-path` and `--exclude-path`
  selectors for direct generation and `poolster.json` recipes.
- Added reproducible generation locks that record secret-free inputs, generator
  settings, selected operations, and artifact hashes.
- Added `poolster show` for inspecting the exact operation slice selected from a
  local contract, including JSON output for automation and agents.
- Added `poolster update` to replay direct-generation locks when their local source
  or compiler-artifact input changes, with a `--force` override.
- Added `poolster auth login`, `logout`, and `status` for named, environment-backed
  credential profiles in authenticated remote OpenAPI inputs. Profiles retain
  only the environment-variable name, never a token value.
- Added `poolster discover` and safe `poolster download` support for the APIs.guru
  directory, with relevance-ranked and JSON output for automation.
- Added `poolster mcp generator`, exposing local generation controls to MCP clients.
- Added runnable examples for API CLIs, native mocks, reproducible generation,
  agentic generation, Ruby, and Swift.

### Changed

- Made `poolster-plugin-csharp` and `csharp` the canonical C#/.NET generator
  surface. The `dotnet` crate and selector remain compatibility aliases.
- Extended CLI configuration, JSON Schema, MCP discovery, examples, and
  documentation for C#, Ruby, and Swift targets.
- Added authenticated remote OpenAPI inputs with custom headers, Basic, Bearer,
  and environment/profile-backed secret values.
- Improved generated Go retry behavior and TypeScript generated-client
  ergonomics.
- Added CI coverage for the canonical C# target and generated Rust SDK builds
  on fresh runners.
- CI now builds Poolster once, generates a single cross-language fixture, and
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

- Established the standalone Poolster Rust workspace.
- Added first-party SDK targets and language-neutral contract mocks.
