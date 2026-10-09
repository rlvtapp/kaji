# SDK roadmap

Poolster keeps its Kubb-like plugin graph: providers publish typed capabilities,
consumers select compatible instances, and package finalization owns manifests.
Runtime and delivery improvements extend this architecture while preserving
independent plugins and native language APIs.

## Delivered in this implementation

- Opt-in package-local idempotency keys through OpenAPI extensions or recipes,
  native customer overrides, secure automatic UUIDs, and retry-stable keys.
  PATCH requires a key for replay; configured custom headers stay operation-local.
  Server retry delays include bounded `retry-after-ms` handling. See
  [idempotency and retry safety](../guides/idempotency.md).

- Ownership-aware output preflight, safe stale-file cleanup, preservation of
  custom files, and read-only generation `--check`.
- Independent TypeScript models, transport, operations and client providers,
  named handle binding, and native query/schema/mock consumers. The CLI exposes
  these providers through `id` and `uses`.
- Native Rust/Go provider contracts and substitutable transport interfaces;
  Python and Ruby model contracts with optional roundtrip consumers.
- Bounded schema sample generation and executable wire tests. TypeScript supports
  schema-directed lossless int64 strings/bigints through Fetch, Axios and SSE.
- Optional native async Python/httpx, sync/async client-credentials providers,
  cache/singleflight and bounded unauthorized retry, plus raw-body webhook HMAC
  verification. Generated Python docs use actual operations.
- Forward-compatible Java enum values, and unknown-property/null handling
  improvements in native model renderers. Runtime evidence varies by language.
- Native page-number helpers in all ten SDK targets, with declared controls,
  response selectors and lazy iteration. Target-specific limits and existing
  cursor/offset/URL forms are listed in the [pagination guide](../guides/pagination.md).
- A ten-language executable HTTP conformance harness and CI matrix; locally
  verified runtime scenarios and unsupported behavior are listed in its manifest.
- Optional Python/Go operation smoke tests, package API references, SDK doctor,
  nested API release notes, Terraform data sources and executable Newman tests.
- Optional custom-language package metadata, independent package versions,
  build/test execution, API diff sizing, reviewable CI/release scaffolding and
  owned-output SDK repository synchronization.

## Additional implemented capabilities

- Opt-in Standard Webhooks HMAC v1 verifiers in all ten SDK targets, canonical
  vector probes and explicit native toolchain evidence. Go adds cached OAuth
  client credentials with coordinated refresh and one replay-safe 401 recovery.
- TypeScript Fetch/Axios and Rust bounded generated operation tests join existing
  Python/Go consumers, with explicit unsupported diagnostics.
- Swift cursor pagination and recursive model support; identifier collision
  diagnostics in Swift/Java/C#; Java/C# multiline SSE framing.
- A checked-in complex OpenAPI regression corpus and checksum-pinned Microsoft
  Graph native regression, plus a manual read-only large-contract workflow.
- Explicit OpenAPI 3.2/future-version diagnostics before artifact writes.
- Reviewed-hash Postman remote collection helper and create-once Terraform
  GoReleaser/signing/registry scaffolding. No live service publication was run.
- A [full feature catalog](../about/features.md) with target differences and evidence levels.

## Follow-up sequence

Package-scoped add/replace/guarded-patch source customization is now available
through JSON recipes and the library API. Runtime middleware has been added to
TypeScript, Python, Go, Rust, Ruby, Swift, and Elixir; native HTTP-client injection remains
available in Java, C#, and PHP. See [SDK customization](../reference/regeneration/sdk-customization.md)
for exact language behavior and integration examples.

Release delivery now includes editable language-checking and registry-publishing
actions, exact-tag release matrices, Release Please configuration, and a private
GitHub App or self-hosted OIDC broker integration. See [publishing](../reference/automation/sdk-publishing.md),
[App setup](../reference/automation/github-app.md), and [source customization](../reference/regeneration/source-customization.md).
The gated disposable delivery workflow is prepared. Live installation and registry publication remain untested; no publication was requested.

The [Terraform provider plan](terraform-provider-plan.md) describes the migration
from the existing raw-JSON prototype to typed lifecycle-aware providers. The
[Postman plan](postman-generation-plan.md) describes a new collection output.
Postman export/execution, reviewed remote collection updates and typed Terraform CRUD/single-entity data sources are available. See [Postman](../reference/outputs/postman.md) and [Terraform](../reference/outputs/terraform-provider.md); remote environment/workspace provisioning, advanced lifecycle and live registry verification remain roadmap work.

1.
Broaden the verified model policies to more real public contracts.
Open enum
   options, decoded unknown fields and nullable presence now have native roundtrip
   probes across targets; constructor helpers expose explicit null on dynamic models.
2.

Extend the explicitly unsupported pagination forms and bindings in the
   capability table where needed, and verify legacy continuation edge cases.
Preserve native public APIs; different iteration interfaces are expected.
3.
Extend OAuth and native protocol coverage.

All ten targets have optional
   client-credentials helpers and native call scopes; PHP timeout policy is supplied
   by its PSR driver.
Timeout scope remains native and documented.
4.
Broaden independent native provider/consumer APIs and expose Rust/Go composition
   in the recipe registry.
Retain convenient `sdk()` plugins and make ambiguous
   bindings fail before file emission.
5.

Exercise release workflows end to end in a test repository: release tags,
   registry-specific auth, custom toolchains, multi-repository routing and
   published launcher availability.
Add provenance/source snapshots for remote
   spec diffs and richer status reporting.
6.
Extend source ejection with optional template adapters only when demanded by
   real custom plugins.

Verify additional custom media codecs beyond the implemented
   OpenAPI 3.2 whole-query, sequential JSON and ordered multipart transports.
Bounded public-HTTPS
   reference resolution is available with closure provenance.

Each stage requires generated consumer compilation and meaningful wire behavior
checks, not just snapshot approval. Opt-in compatibility changes stay opt-in
until their migration path is tested. SDK convenience plugins share maintained
renderers with provider plugins; TypeScript `sdk()` is currently one plugin,
not an engine-expanded bundle.

See [TypeScript](../../crates/plugins/typescript/README.md),
[native providers](../reference/outputs/native-sdk-providers.md), [fixtures](../verification/shared-sdk-fixtures.md),
[safe regeneration](../reference/regeneration/safe-regeneration.md), and [automation](../reference/automation/sdk-automation.md).


### Latest 0.5.0 additions

- Optional generated operation tests now cover all ten SDK language recipes with
  fake native drivers and explicit unsupported-case reports.
- Ruby and Swift gain opt-in replay-safe retries with bounded backoff and
  cancellation during waits; their defaults remain one attempt.
- Elixir gains offset and absolute same-origin URL pagination. Relative URL and
  request-body controls remain unsupported.
- Java/C# named scalar and union wrappers preserve the underlying JSON. C# gains
  opt-in open enums and presence wrappers that distinguish absent from explicit null.
- Reviewed Postman environment sync preserves remote secrets and manual variables.

The latest local verification passed 418 workspace tests, all fifteen native Swift
probes, 163 shared native wire cases across ten runtimes and 37 Node checks.
Java/C#/PHP/Elixir operation, model and pagination probes passed with disposable
native toolchains and are repeated in CI.

General Terraform type migrations,
actual OpenAPI 3.2 handling, remaining protocol
features and the prepared-only live delivery trial remain separate work.

## Latest tested additions

See the [Speakeasy Terraform comparison](../reference/outputs/terraform-speakeasy.md) for nested schemas,
composite IDs and bounded state upgrades. New runtime coverage includes Swift SSE,
Java/C# presence and multipart, Ruby OAuth, and additional URL pagination.
Bounded read-GET lifecycle polling is implemented; transformations, arbitrary job
endpoints and type-conversion migrations remain future work.

## Current parity hardening

Source ejection now exports the actual rebuildable generator and plugin workspace;
it does not introduce a runtime template override flag.
OpenAPI 3.2 ordinary
contracts, QUERY/custom methods, whole-query content, sequential item schemas and
ordered/nested multipart encodings are accepted.
Native transport plans retain
encoding and MIME headers; custom formats retain documented codec boundaries.

Local reference closure hashing and artifact revisions prevent stale compiler
caches.
Optional OAuth providers and native call scopes cover all ten targets.

Six pinned official contracts and a manual ten-language matrix expose failures
beyond the shared wire fixtures. Full Go SDKs compile for OpenAI, GitHub, Stripe,
Twilio and Linode; this is not an all-language pass claim. See
[large specifications](../verification/large-specs.md) for reproducible commands.

Remaining work includes custom-format codecs, authenticated/private remote reference
fetching, broader unknown-union/model preservation,
additional automatic DTO-to-MIME mappings and broader public-contract coverage. The live delivery trial remains
prepared-only at the user's request. These boundaries are tracked separately from
implemented features; Poolster keeps its plugin architecture.

Per-call controls now cover the TypeScript, Rust, Python, Go, Java and C# targets plus Ruby, using native scoped
clients or request/context options.
Swift adds bounded typed multipart; Go/Python/
Rust expose buffered part builders, and Java/C# extend typed multipart with JSON
and repeated arrays.
Rust `open_unions` can preserve unmatched future values.

These capabilities keep their documented scope and do not imply universal model
or multipart compatibility.
See [request controls](../guides/request-controls.md).

## Expanded native coverage

Swift/PHP/Elixir now have optional OAuth providers and independent call scopes.
Ruby/PHP/Elixir add explicit bounded multipart builders for declared upload
operations, including binary, JSON and repeated fields. Public HTTPS reference
closures are fetched with bounded requests and contribute to provenance hashes.
These additions preserve native driver interfaces and documented limits.

OpenAPI 3.2 custom methods preserve their case and default to unsafe retry
semantics; whole-query parameters, item schemas and positional multipart encoding
remain unsupported rather than being silently discarded.


The 3.2 compiler/runtime expansion adds typed recursive encoding metadata, whole-query
and named JSON content parameters, buffered record-oriented JSON and ordered nested
multipart in the native targets.
`$self`, tag hierarchy, reusable media definitions,
XML node metadata and device authorization endpoints are preserved.
Multipart
responses remain native buffered data for caller decoding.

See the
[OpenAPI 3.2 guide](../guides/openapi32.md) and
[model policy guide](../guides/forward-compatible-models.md) for the concrete APIs.

See the [generator completion backlog](generator-backlog.md) for language fixes, output size/splitting, framework APIs and artifact acceptance work. These items are planned, not current capabilities.
