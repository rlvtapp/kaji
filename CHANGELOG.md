# Changelog

## Unreleased

## [0.5.0](https://github.com/rlvtapp/kaji/compare/v0.4.0...v0.5.0) (2026-10-08)


### Features

* **artifacts:** add API references, Terraform data sources and Newman checks ([1b49647](https://github.com/rlvtapp/kaji/commit/1b4964741b9ba23b731ee975adb4627759c37051))
* **artifacts:** scaffold registry releases and reviewed Postman sync ([bbb91b3](https://github.com/rlvtapp/kaji/commit/bbb91b32de86c3bf846bd7f5f59bda3860a3c2d4))
* automate SDK checks releases and per-language repository delivery ([e7dc4a7](https://github.com/rlvtapp/kaji/commit/e7dc4a7edaf7488dcecff0bbf58caf7b3e1c0bf3))
* **cli:** eject rebuildable sources and expose runtime capabilities ([3838761](https://github.com/rlvtapp/kaji/commit/3838761aca76655b9e4b24b28d1488c07943b493))
* **cli:** expose native operation tests and verify replay-safe runtimes ([365469d](https://github.com/rlvtapp/kaji/commit/365469d6e4e50b6f5e9e639d5b796d72dd339e07))
* **core:** resolve package-local idempotency policies ([8d91d41](https://github.com/rlvtapp/kaji/commit/8d91d410fc601eaa1249dde6f05ca58fa5b321d1))
* **delivery:** add doctor, API release notes and gated workflow template ([54ea102](https://github.com/rlvtapp/kaji/commit/54ea102da6c78ac397c24dcf57c46db887a59e28))
* expand SDK customization and add Postman and typed Terraform generators ([8293ce7](https://github.com/rlvtapp/kaji/commit/8293ce7ad0e667a035b9c762bfd588b254a758b1))
* **generators:** improve output layouts and runtime contracts ([3c84389](https://github.com/rlvtapp/kaji/commit/3c84389c202eae91a54fd590d664e9159bd78063))
* **inputs:** add native contracts and parsers to the Rust engine ([e45d398](https://github.com/rlvtapp/kaji/commit/e45d398f408b703d31cd480b91d5fb1ce2b2bd56))
* **migration:** reuse vendor configs and normalize SDK annotations ([6037bfb](https://github.com/rlvtapp/kaji/commit/6037bfbd5330f98c2a347369d337e9ed0891ab4a))
* **node:** add NAPI embedding and explicit JS plugins ([60cd1ad](https://github.com/rlvtapp/kaji/commit/60cd1ad5bfe2358af1148257dda460236a7e5741))
* **openapi:** normalize 3.2 contracts and local reference closures ([0a29a8e](https://github.com/rlvtapp/kaji/commit/0a29a8e81329dcc4ae27f1c6c821ae0af9c20fa9))
* **openapi:** preserve 3.2 content and reference metadata ([fd2400d](https://github.com/rlvtapp/kaji/commit/fd2400dd28fc39e925a6e65c7ff373d12eff76a5))
* **pagination:** add native page helpers across remaining SDK targets ([80036f1](https://github.com/rlvtapp/kaji/commit/80036f1101b45d343824aae2f82fed48436daff2))
* **postman:** add reviewed environment sync and document feature boundaries ([833b885](https://github.com/rlvtapp/kaji/commit/833b8858034d32a3005327535615ce0d75dd4d24))
* **sdk:** add model compatibility and 3.2 native transports ([ea6975f](https://github.com/rlvtapp/kaji/commit/ea6975f2b537b7750cab6de2d9fb06092f5c4788))
* **sdk:** add native OAuth, scoped HTTP controls and buffered uploads ([3728f4f](https://github.com/rlvtapp/kaji/commit/3728f4f366008d2c3077866f97e7b92515612047))
* **sdk:** add response checks, page pagination and operation smoke tests ([17e0206](https://github.com/rlvtapp/kaji/commit/17e0206401c6997112a3f0c5ac50f80f2428b68c))
* **sdk:** add webhook verifiers OAuth providers and native operation tests ([4d7ddb2](https://github.com/rlvtapp/kaji/commit/4d7ddb2bf53b52b36241f4d0d6af1afa99f94ad9))
* **sdk:** bundle idempotency keys and honor bounded retry delays ([1f12047](https://github.com/rlvtapp/kaji/commit/1f12047b5262fae21a4aa027b01591268598ab76))
* **sdk:** complete operation test plugins and expand native runtime behavior ([ebeb8e1](https://github.com/rlvtapp/kaji/commit/ebeb8e166d08577c72b312906c5afc04b18b5653))
* **sdk:** expand native protocols and resolve remote and custom HTTP contracts ([8b1211c](https://github.com/rlvtapp/kaji/commit/8b1211cf636c87f0498ac58a1ed6debe43905e2f))
* **sdk:** extend native streaming pagination models multipart and OAuth ([c36859c](https://github.com/rlvtapp/kaji/commit/c36859cadc824eca19266eb03b7106ac8acb1d69))
* **terraform:** generate nested schemas composite identities and state renames ([fc23d5a](https://github.com/rlvtapp/kaji/commit/fc23d5a5793201c3276bb3dff4b419433ae7dd74))
* **terraform:** wait for asynchronous lifecycle completion with bounded polling ([4c2de75](https://github.com/rlvtapp/kaji/commit/4c2de753da18b04e6b04f3874d3eba44c87e3713))
* **typescript:** bound generated modules and expand query factories ([996af56](https://github.com/rlvtapp/kaji/commit/996af564105af3c34e8526ec44f92238499f2820))
* validate SDK response shapes and strengthen consumer middleware tests ([7e60802](https://github.com/rlvtapp/kaji/commit/7e60802a5f6cb796166be06f4621e0e903e5468b))


### Bug Fixes

* **ci:** build compiler before probes and handle workspace releases ([4264e67](https://github.com/rlvtapp/kaji/commit/4264e67377fe45f86bf8f8a3fcceb694429e3c96))
* **ci:** install Go for Swift compiler integration ([2c085d6](https://github.com/rlvtapp/kaji/commit/2c085d60797f51fc478284ea46bbd0380160ba0f))
* **ci:** keep release preparation manual ([6f35b6a](https://github.com/rlvtapp/kaji/commit/6f35b6acdd08c0aef716458d62146d96af6d74f0))
* **ci:** satisfy current Clippy resource plan lint ([e4586a8](https://github.com/rlvtapp/kaji/commit/e4586a8e560b775c806c25dc3961ffbe7da48ea3))
* **ci:** select Swift 6 and load Elixir probe dependencies ([340e91d](https://github.com/rlvtapp/kaji/commit/340e91dd3ce0b19c97b91dd791451b3c6e54f388))
* **ci:** update release preparation while keeping publication gated ([c585810](https://github.com/rlvtapp/kaji/commit/c5858108a7867260820e7bcc74078607202b3a92))
* **ci:** use a byte string in CLI fixture hashing ([847adb2](https://github.com/rlvtapp/kaji/commit/847adb2be8e96b7a62dd6a99dccb3621ba48efe6))
* compile large contracts with stable symbols and faithful references ([6875b9f](https://github.com/rlvtapp/kaji/commit/6875b9fbc3a242bfc097976896bb822c2871f39e))
* **csharp:** preserve repeated array query values in native requests ([2acb175](https://github.com/rlvtapp/kaji/commit/2acb175007f21a753cdab0f9d7c346aa9f6f0130))
* inherit 0.5.0 workspace versions across all crates ([77aec8f](https://github.com/rlvtapp/kaji/commit/77aec8f3d122fa5cdcc2017ab0cd9fb9ccc3c4cb))
* **openapi:** preserve parameter identities and deterministic overrides ([23f4886](https://github.com/rlvtapp/kaji/commit/23f48865d06c03ae8427c0cf08953ad8a8019af4))
* **pagination:** validate selectors and prevent invalid continuation counters ([2242f15](https://github.com/rlvtapp/kaji/commit/2242f157a55f6df953f0a1fee98ee7282cc8defa))
* **rust:** allow native dependency downloads on fresh CI runners ([9417729](https://github.com/rlvtapp/kaji/commit/9417729334fef99969c9ccb900777a0af343d64a))
* **sdk:** allocate native model names and preserve recursive shapes ([1d851f6](https://github.com/rlvtapp/kaji/commit/1d851f636c84fdc9217c0bcb9564340cde6bf3d9))
* **sdk:** correct native serialization cancellation and ESM packaging ([6d3f5e0](https://github.com/rlvtapp/kaji/commit/6d3f5e0c7cb608981e897dbee496bfb9ddd0c78e))
* **sdk:** preserve native symbols and wire values in dynamic languages ([8b45ed4](https://github.com/rlvtapp/kaji/commit/8b45ed424633f149185c32adc322fac04bb289ae))
* **sdk:** resolve native Java Csharp and Elixir generation failures ([5cdcd19](https://github.com/rlvtapp/kaji/commit/5cdcd19dee603fb879d65270091ff96a3ea70f55))
* **sdk:** retain allocated idempotency header arguments ([8177a32](https://github.com/rlvtapp/kaji/commit/8177a32dce865798dec254a10755d9ade8af3d62))
* **swift:** bound compiler complexity for large query operations ([84ca00a](https://github.com/rlvtapp/kaji/commit/84ca00a6ddf413ac0e6036600384148828138fb8))
* **swift:** bound query code generation and preserve native wire names ([5330255](https://github.com/rlvtapp/kaji/commit/5330255c74c6555ad7a06ca660c4329f4ccc5b16))


### Performance Improvements

* **generators:** split validation registries and large command trees ([72cfa8f](https://github.com/rlvtapp/kaji/commit/72cfa8ff5ab5d2f2de6cc4fb77acdb1d1d810eeb))

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

- Migration from Stainless, Fern and Speakeasy through `kaji migrate`, direct
  generation from supported vendor configurations, and automatic normalization
  of supported OpenAPI annotations. Originals are preserved; unsupported settings
  receive explicit manual-review diagnostics.
- Bundled runtime HTTP middleware that registers automatically in generated SDKs,
  plus package source overlays, explicit replacements and guarded patches.
- Named typed plugin contracts and independent model, transport, operation and
  client providers for TypeScript and native Rust/Go composition.
- Rebuildable `kaji eject` source bundles with SHA-256 manifests, preserving the
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
- Opt-in `x-kaji-idempotency` and per-package rules across ten SDK targets, with
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
