# Recorded verification evidence

[How to verify](../verification.md) · [Compatibility matrix](../guru-compatibility.md)

These are reports from earlier verification runs. Counts and host-specific
statements describe those runs; they are not the current workspace result.
Current input-plugin results are in [the input guide](../input-plugins.md#verification).

Expand the area you are investigating.

## Latest 0.5.0 integration evidence

<details>
<summary>Read the recorded checks and limits</summary>

The integrated workspace passes 469 tests, formatting checks and Clippy with
warnings denied.
Freshly generated SDKs pass 163 shared HTTP scenarios across
all ten runtimes.
Additional native probes cover rebuilding an ejected renderer,
local-reference provenance, OpenAPI 3.2 QUERY requests, scoped HTTP controls,
OAuth refresh and buffered multipart uploads.

All 17 previously documented Swift native probes pass.
New Swift/PHP/Elixir
OAuth and scoped-client probes pass, as do Ruby/PHP/Elixir multipart probes and
custom-method wire probes in Go, Fetch/Axios, Rust and Ruby.

The model and 3.2 expansion adds native nested unknown-enum/property and
missing/null roundtrips, explicit-null constructor helpers, whole-query and named
JSON-content parameters, sequential JSON requests/responses and ordered nested
MIME plans.
Generated operations verify required null parameters and required MIME
part headers.
TypeScript probes also cover lossless integers in JSON sequences.

Compiler tests retain reusable media references, device authorization, tag hierarchy,
XML metadata and `$self`; source-format revisions invalidate compiler caches.
Buffered multipart responses retain native bytes for application decoding.


The complete pinned OpenAI contract generates and compiles or imports in
TypeScript, Rust, Python, Go, Java, C# and Ruby. All six pinned public contracts
compile in Go. The manual public-contract matrix records results for ten language
targets; this does not establish that every contract works in every target.

The broader [205-contract compatibility baseline](../guru-compatibility.md) attempts
all ten languages and records generation/native failures beyond these curated
fixtures. Go passes all 205; the other targets have explicit failures. Use that
matrix when assessing large-contract compatibility and stability.

These checks use local fixtures and mock HTTP servers. Live GitHub synchronization
and registry publication remain prepared workflows, without a live delivery trial.

</details>

## Shared runtime contract and artifact execution

<details>
<summary>Read the recorded checks and limits</summary>

The [runtime contract](../../packages/internal/runtime-contract/README.md) exports the same
API into ten SDK targets and drives their public operations against a loopback
server.
Its manifest declares 17 scenarios and every supported/unsupported
mapping.
TypeScript, Python, Go, Rust, Swift and Ruby passed locally.

Java, PHP
and Elixir also passed with disposable toolchains, with fifteen supported scenarios
each; strict structural missing/wrong-shape checks remain unsupported in those three.
C# passed all seventeen scenarios, including strict structural decoding and repeated
array query keys.
In total, 163 supported scenarios passed across ten targets.

CI independently repeats these checks.

Additional native checks execute Python sync/async and Ruby response validation,
Python/TypeScript/Go/Rust/Ruby/Swift page pagination, and Python/Go generated operation smoke
tests. Optional API reference and Terraform data source recipes are covered by
CLI output assertions and actual JSON Schema validation. Newman executes an
exported collection against a local mock; a real Terraform CLI exercises
create/read/update/import/destroy and a supported single-entity data source.
These tests have the limits stated in each artifact's guide.

The [delivery test workflow](../../packages/internal/sdk-delivery-test/README.md) is prepared
only. Local drift, immutable-tag mock publishing and workflow validation passed;
no workflow dispatch, GitHub App installation, registry trust configuration or
real package publication has been performed.

The pagination fixture declares `listContacts` with optional integer page/limit
controls and a typed results array.
Generation asserts a helper in all ten
SDKs, and the runtime CI matrix builds the resulting packages.

Dedicated Swift,
PHP and Elixir native pagination probes are selected in their toolchain jobs;
Swift, PHP, Elixir and .NET native pagination probes passed locally using
disposable toolchains where needed.
The [pagination guide](../guides/pagination.md)
records current forms and unsupported bindings explicitly.

The shared export also includes a POST operation with a custom automatic
idempotency header, so the native matrix compiles this policy in every target.

Dedicated executed probes cover TypeScript Fetch/Axios, Go, Python sync/async,
Rust, Ruby, and Swift UUID generation and caller preservation; targets with
retries additionally verify stable keys across attempts and operation-scoped
custom headers.
Retry-capable targets reject empty/whitespace-only keys as replay
protection.
PHP/Elixir behavior and Java/C# retry parsing passed their native probes.

The [idempotency guide](../guides/idempotency.md)
explains server requirements and recipe precedence.

The expanded corpus executes mutation retry safety, caller/automatic key lifetime,
page iteration, strict decoding and exact serialization bytes.
Separate probes
exercise cancellation and OAuth refresh recovery.
An installed TypeScript package
check builds, packs, installs, imports root/subpath exports and checks customer
NodeNext types.

Regeneration tests protect all packages from partial writes on
late conflicts; delivery tests inject failure at every phase and verify cleanup.
These checks use local fixtures and mocked delivery, not registry publication.

</details>

## Expanded security, models and delivery sources

<details>
<summary>Read the recorded checks and limits</summary>

Optional Standard Webhooks verifiers now exist for all ten SDK targets. Native
canonical-vector probes passed in TypeScript, Rust, Go, Python, Ruby and Swift on
Apple, plus Java/C#/PHP/Elixir using disposable native toolchains. Swift's Linux
Crypto backend remains unverified. Go's OAuth probe runs with the race detector
and checks coordinated refresh, cancellation, expiry and bounded replay safety.
See [OAuth/webhooks](../guides/oauth-webhooks.md).

Generated operation tests now exist in all ten SDK languages (and the dotnet
recipe alias).
Ruby and Swift emitted operation/retry probes passed locally alongside the prior
TypeScript/Rust/Go/Python probes.
Java/C#/PHP/Elixir emitted probes also passed
with disposable native toolchains.
The suites use production SDK compilation and
fake native drivers.

Unsupported operations are explicitly
listed, not counted as covered.
The complex OpenAPI corpus passes through the
actual Go compiler and executes Swift models; Java/C# SSE probes passed natively.

The checksum-pinned Graph regression compiles the full generated Go package,
executes typed request and extension-data roundtrips, compares SDK outputs with
one/four workers and generates a multi-language recipe. Run metadata intentionally
records different output paths/worker settings. TypeScript generation in that
large-contract check is not native TypeScript compilation. See [large specs](../large-specs.md).

Postman sync tests mock API requests, reviewed remote hashes and read-back checks.
Terraform release scaffolding is editable and inactive until deliberately copied
into workflows. Native Framework lifecycle checks passed after the namespace and
version changes; GoReleaser signing, external release uploads and registry ingestion
remain unexecuted. The [feature catalog](../features.md) records these boundaries.


The shared corpus now enables Ruby/Swift retry options and passed their replay-safe
GET, generated-key POST/PATCH and caller-key override cases. Defaults remain one
attempt. Swift cancellation interrupts backoff; Ruby's cancellation callback is
checked between attempts and during backoff, while in-flight synchronous transport
interruption remains the driver's responsibility.

Java/C# named scalar and union wrappers now serialize as their underlying JSON
values. C# `sdk().open_enums(true)` (JSON `open_enums`) preserves future enum strings;
the default enum API is unchanged. Dedicated native model probes are selected in
CI and passed locally. Default optional-nullable fields still cannot reliably
distinguish omission from explicit null; these changes do not claim that gap is
closed. Union wrappers preserve raw JSON and do not validate union matching.

Postman environment synchronization adds mocked checks for reviewed publication,
secret preservation, manual-variable preservation, duplicate rejection, read-back
failure, response size limits and redacted transport errors. No live Postman API
call has been made.

</details>

## Additional native coverage

<details>
<summary>Read the recorded checks and limits</summary>

Terraform Framework probes exercise nested lists/maps, unknown-child hydration,
known-plan consistency, composite parent/child identity, import diagnostics and
actual versioned state upgrades. The real local CLI lifecycle also passed with
Terraform 1.13.4. Swift native probes exercise incremental SSE before EOF, socket
cancellation, offset/URL pagination and unknown enum roundtrips. Rust's URL probe
also checks typed errors preserve raw response bytes. Ruby OAuth probes cover
concurrent refresh, cancellation, expiry and bounded 401 recovery.

Java/C# presence and multipart probes passed with disposable JDK17/Maven and .NET8
toolchains. PHP and Elixir probes passed in disposable Linux containers. Two checksum-pinned official Open-Meteo
contracts compile locally in Rust, TypeScript, Go, Python, Ruby and Swift. CI adds
Java, C#, PHP and Elixir; both contracts also passed locally in those four targets.
These checks never call production API endpoints.

</details>

## Terraform lifecycle polling

<details>
<summary>Read the recorded checks and limits</summary>

Native Framework tests passed for bounded read-GET create/update/delete waiters:
pending-to-ready state, confirmed deletion, composite path escaping, conjunction
criteria and failure precedence, attempts/deadlines/cancellation, recoverable
accepted-create identity and retained state after failures. Tests also cover exact
large numeric comparisons, missing versus null, duplicate/trailing JSON, response
limits and credential/body redaction.

A real Terraform 1.13.4 mock lifecycle passed validation, apply, no-change plan,
update, no-change plan and destroy. It verified exactly one POST, PATCH and DELETE.
These tests use deterministic local mocks; arbitrary job endpoints and multi-step
mutation workflows are not implemented. CI selects both probes explicitly.

</details>
