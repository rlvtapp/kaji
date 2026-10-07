# Verify an SDK before shipping it

A successful generation proves that Kaji assembled source. It does not prove
that your package compiles, that a request reaches the right endpoint, or that
custom middleware preserves response decoding. SDK authors should check those
properties separately and make their release workflow run the same checks.

## Latest 0.5.0 integration evidence

The integrated workspace passes 434 tests, formatting checks and Clippy with
warnings denied. Freshly generated SDKs pass 163 shared HTTP scenarios across
all ten runtimes. Additional native probes cover rebuilding an ejected renderer,
local-reference provenance, OpenAPI 3.2 QUERY requests, scoped HTTP controls,
OAuth refresh and buffered multipart uploads. All 17 Swift native probes pass.

The complete pinned OpenAI contract generates and compiles or imports in
TypeScript, Rust, Python, Go, Java, C# and Ruby. All six pinned public contracts
compile in Go. The manual public-contract matrix records results for ten language
targets; this does not establish that every contract works in every target.

These checks use local fixtures and mock HTTP servers. Live GitHub synchronization
and registry publication remain prepared workflows, without a live delivery trial.

## Verify your own package

1. Generate from the committed recipe and contract.
2. Run its native build with the dependencies declared by that package.
3. Call a generated operation through a deterministic fixture driver. Assert
   URL, headers, body, response decoding, and any authored policy behavior.
4. Run `kaji generate --config kaji.json --check` to detect source drift.
5. Declare the build/test commands in `release` metadata so SDK CI and release
   checks use them.

The [bundled middleware example](../examples/bundled-middleware/README.md)
performs all five steps for a TypeScript SDK. The test deliberately passes no
middleware option: the generated constructor must install the authored policy.
The [SDK automation guide](sdk-automation.md) explains the release boundary;
[SDK publishing](sdk-publishing.md) explains registry-specific artifact checks.

A fixture driver is useful for exact request assertions without a running API.
A loopback/mock test additionally exercises the native network stack. Sandbox
permissions, local ports, and native dependencies can make that test opt-in;
keep its prerequisites explicit rather than reporting an ignored test as a pass.

## What Kaji's own tests establish

| Evidence | What it checks | Limit |
| --- | --- | --- |
| Output snapshots | Paths, byte lengths, and fingerprints of emitted files | Do not execute the SDK |
| Package composition tests | Binding, ordering, ownership, package scope, and failures | Do not prove native HTTP behavior |
| Native compilation probes | A generated package and typed consumer compile | Need the native toolchain/dependencies |
| Fixture transport execution | Generated calls serialize/decode correctly and policies run | Does not prove live API compatibility |
| Loopback contract tests | Generated clients communicate with a contract mock | Cover the selected operations/targets |
| Action/broker tests | Policy enforcement, tag/artifact selection, registry retry logic | Mocked APIs do not prove a live release setup |

### Output snapshots

`crates/kaji/tests/golden_output.rs` generates Rust, TypeScript Fetch/Axios, Go,
Python, PHP, Java, C#/DotNet, Elixir, and a mock-server package from a shared API.
The checked-in fixture protects every emitted file, including runtimes,
README files, and manifests. Review output changes before updating it.

```sh
cargo test -p kaji --test golden_output
```

### Runtime and middleware checks

Plugin tests include executable generated consumer/transport probes. Runtime
middleware probes cover request/response changes, error recovery, ordered
composition, and synthetic responses. Separate bundled-policy probes call the
SDK without consumer middleware setup. See [shared fixtures](shared-sdk-fixtures.md)
and [native providers](native-sdk-providers.md) for language-level contracts.

The current author-bundle work was executed for TypeScript Fetch/Axios, Python
sync/async, Go, Ruby, Rust, and Swift. Java/C#/PHP and Elixir have registration
checks and opt-in executable probes; those new native probes remain unexecuted
on this host because their toolchains were unavailable. Passing a generator's
source tests is not a substitute for running its native probe in CI.

Tests annotated `#[ignore = "..."]` describe their own tool/dependency
requirements. Run a selected probe by name once those requirements are present;
do not assume one generic ignored-test command has the right environment for
all languages. For example, the TypeScript middleware probes require
`KAJI_TSC_JS` and `KAJI_AXIOS_NODE_MODULES` to point at installed dependencies.

### Live contract mock

`crates/kaji/tests/sdk_to_mock_contract.rs` generates Go and Python SDKs and
calls a loopback contract mock. It checks the declared route, bearer headers,
and decoded model, and requires the standalone mock fixture to be emitted.

```sh
cargo test -p kaji --test sdk_to_mock_contract -- --ignored
```

This test opens a local port. Its successful execution provides evidence for
that contract and those targets, not universal support across languages.

## Maintainer checks

```sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
node --test packages/sdk-check/test/*.mjs packages/sdk-publish/test/*.mjs packages/github-app-broker/*.test.mjs
```

The Node broker integration test opens a local port; the publisher's archive
reader probe needs Python 3.11+ (set `KAJI_TEST_PYTHON` when necessary). Registry
and GitHub API tests use mocked services. Production delivery still requires an
end-to-end repository trial with a registered App, protected release environment,
remaining runtime consistency and delivery validation work.

## Postman and Terraform artifacts

The Postman plugin has serialization/redaction/authentication tests and an opt-in
check against the full pinned official Collection 2.1 Draft04 schema. Run it with
Python and jsonschema installed:

```sh
cargo test -p kaji-plugin-postman validates_actual_official_draft04_schema -- --ignored
```

The typed Terraform plugin tests semantic exclusions and contract composition,
then exercises emitted Go through real Framework plan/state objects and mock HTTP:

```sh
cargo test -p kaji-plugin-terraform generated_provider_executes_native_framework_lifecycle -- --ignored
```

That test requires Go and the pinned Framework module dependencies. It verifies
create/update plan consistency, import, drift, authentication errors, HTTP 404
refresh/delete, and recovery of identity after create normalization errors. It
does not launch the Terraform CLI or validate remote registry publication.
The repository CI runs both checks and the combined API-artifacts example.

## Shared runtime contract and artifact execution

The [runtime contract](../packages/runtime-contract/README.md) exports the same
API into ten SDK targets and drives their public operations against a loopback
server. Its manifest declares 17 scenarios and every supported/unsupported
mapping. TypeScript, Python, Go, Rust, Swift and Ruby passed locally. Java, PHP
and Elixir also passed with disposable toolchains, with fifteen supported scenarios
each; strict structural missing/wrong-shape checks remain unsupported in those three.
C# passed all seventeen scenarios, including strict structural decoding and repeated
array query keys. In total, 163 supported scenarios passed across ten targets.
CI independently repeats these checks.

Additional native checks execute Python sync/async and Ruby response validation,
Python/TypeScript/Go/Rust/Ruby/Swift page pagination, and Python/Go generated operation smoke
tests. Optional API reference and Terraform data source recipes are covered by
CLI output assertions and actual JSON Schema validation. Newman executes an
exported collection against a local mock; a real Terraform CLI exercises
create/read/update/import/destroy and a supported single-entity data source.
These tests have the limits stated in each artifact's guide.

The [delivery test workflow](../packages/sdk-delivery-test/README.md) is prepared
only. Local drift, immutable-tag mock publishing and workflow validation passed;
no workflow dispatch, GitHub App installation, registry trust configuration or
real package publication has been performed.

The pagination fixture declares `listContacts` with optional integer page/limit
controls and a typed results array. Generation asserts a helper in all ten
SDKs, and the runtime CI matrix builds the resulting packages. Dedicated Swift,
PHP and Elixir native pagination probes are selected in their toolchain jobs;
Swift, PHP, Elixir and .NET native pagination probes passed locally using
disposable toolchains where needed. The [pagination guide](guides/pagination.md)
records current forms and unsupported bindings explicitly.

The shared export also includes a POST operation with a custom automatic
idempotency header, so the native matrix compiles this policy in every target.
Dedicated executed probes cover TypeScript Fetch/Axios, Go, Python sync/async,
Rust, Ruby, and Swift UUID generation and caller preservation; targets with
retries additionally verify stable keys across attempts and operation-scoped
custom headers. Retry-capable targets reject empty/whitespace-only keys as replay
protection. PHP/Elixir behavior and Java/C# retry parsing passed their native probes. The [idempotency guide](guides/idempotency.md)
explains server requirements and recipe precedence.

The expanded corpus executes mutation retry safety, caller/automatic key lifetime,
page iteration, strict decoding and exact serialization bytes. Separate probes
exercise cancellation and OAuth refresh recovery. An installed TypeScript package
check builds, packs, installs, imports root/subpath exports and checks customer
NodeNext types. Regeneration tests protect all packages from partial writes on
late conflicts; delivery tests inject failure at every phase and verify cleanup.
These checks use local fixtures and mocked delivery, not registry publication.

## Expanded security, models and delivery sources

Optional Standard Webhooks verifiers now exist for all ten SDK targets. Native
canonical-vector probes passed in TypeScript, Rust, Go, Python, Ruby and Swift on
Apple, plus Java/C#/PHP/Elixir using disposable native toolchains. Swift's Linux
Crypto backend remains unverified. Go's OAuth probe runs with the race detector
and checks coordinated refresh, cancellation, expiry and bounded replay safety.
See [OAuth/webhooks](guides/oauth-webhooks.md).

Generated operation tests now exist in all ten SDK languages (and the dotnet
recipe alias). Ruby and Swift emitted operation/retry probes passed locally alongside the prior
TypeScript/Rust/Go/Python probes. Java/C#/PHP/Elixir emitted probes also passed
with disposable native toolchains. The suites use production SDK compilation and
fake native drivers. Unsupported operations are explicitly
listed, not counted as covered. The complex OpenAPI corpus passes through the
actual Go compiler and executes Swift models; Java/C# SSE probes passed natively.

The checksum-pinned Graph regression compiles the full generated Go package,
executes typed request and extension-data roundtrips, compares SDK outputs with
one/four workers and generates a multi-language recipe. Run metadata intentionally
records different output paths/worker settings. TypeScript generation in that
large-contract check is not native TypeScript compilation. See [large specs](large-specs.md).

Postman sync tests mock API requests, reviewed remote hashes and read-back checks.
Terraform release scaffolding is editable and inactive until deliberately copied
into workflows. Native Framework lifecycle checks passed after the namespace and
version changes; GoReleaser signing, external release uploads and registry ingestion
remain unexecuted. The [feature catalog](features.md) records these boundaries.


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

## Additional native coverage

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

## Terraform lifecycle polling

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
