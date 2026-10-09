# Verify an SDK before shipping it

Generation, compilation and API behavior are separate checks. Run all three
for the package you ship.

## Current alpha.2 verification (9 October 2026)

After client styles, contract-scoped configuration and GraphQL ecosystem completion,
the full workspace suite passes **816 tests, 0 failures, 143 ignored**. Formatting
and workspace Clippy with `-D warnings` pass. Earlier checks recorded 801/0/141
after Rust scalar mappings and 798/0/141 after initial
GraphQL completion and 788/0/138 after output
migration and 752/0/136 before that migration. Ignored external probes are not counted as passed by that command.
Initial sandbox-denied local-server tests passed when rerun with local-port access;
an earlier overlapping-build artifact issue was resolved by a clean sequential run.
No unresolved workspace failures remain in that completed run.

Separate explicitly enabled integration runs established:

| Pipeline | Executed evidence | Boundary |
| --- | --- | --- |
| GraphQL → TypeScript | Clean installed package, compilation and local GraphQL server; selections, errors/partial, scalar mappings, cancellation and injected subscriptions | No bundled network subscription transport or scalar codecs |
| GraphQL → Rust | Clean `.crate` consumer, compilation and local server; selections, presence/nullability, partial/errors and transport failures | Custom scalars are JSON values; subscriptions unsupported |
| Protobuf → Go gRPC | Two official-toolchain integration probes; local TCP unary/all streaming modes, errors/metadata/deadlines/cancellation/race checks and pinned upstream proto2 imports | Not editions or all old conformance extensions |
| AsyncAPI → TypeScript/Kafka | Generated compilation and actual local Redpanda broker send/receive; keys/headers, invalid payloads, substitution and regeneration | Documented AsyncAPI 3.0/3.1 plaintext Kafka subset |
| Arazzo → TypeScript | Generated compilation and local HTTP execution; checkout and pinned upstream source resolution | Sequential supported subset, no retries/actions/auth automation |
| Cap’n Proto input | Explicit official-compiler source/block probe using capnp 1.5.0 | Input compilation only; no Rust generated package yet |

The full workspace pass includes focused hook, provenance, completeness,
compatibility, codec and symbol-planning tests. Symbol planning remains opt-in;
this is not evidence that all existing generators use it or that Node typed hooks exist.

The **pre-output-migration** frozen **205 pinned APIs.guru specs × 10 HTTP SDK
languages** sweep is complete: **2,050 effective passes** for generation, native
compilation or PHP/Ruby syntax checks, and deterministic regeneration after
audited infrastructure retries. Original failures and retry evidence remain
preserved. PHP emitted deprecation warnings in 140 contracts; syntax checks do
not establish endpoint runtime behavior. These binaries predate the output
contract migration, so the final migrated build still needs its own corpus run.
The earlier [2,050-pass ledger](guru-compatibility.md) is a separate historical
result. The reference-directory harness now verifies and reuses pinned trees;
all 25 offline harness tests pass. [Harness evidence](../verification-results/reference-harness-2026-10-09.json)
records the helper revision separately from the frozen generator binaries.

Reproduce external protocol checks after installing their pinned prerequisites:

```sh
POOLSTER_GRPC_TOOLS=/path/to/pinned/tools \
cargo test -p poolster-plugin-go --test grpc_native -- --include-ignored
POOLSTER_TSC_JS=/path/to/typescript/lib/tsc.js \
cargo test -p poolster-plugin-typescript --test workflow_native -- --include-ignored
```

GraphQL's command is below. Kafka needs the real local broker and Node dependencies
in the [reproduction guide](../../crates/plugins/typescript/tests/fixtures/KAFKA.md).
The [support matrix](../plugin-support-matrix.md) distinguishes implemented output
from parsing, and [remaining work](../reference/inputs/native-pipelines.md#remaining-work) records gaps.
Alpha.2 release artifacts and publication have not been verified or released.

The source-size audit has three pre-existing failures: `crates/core/src/files.rs`,
`crates/cli/src/sdk_install.rs` and `crates/cli/src/sdk_automation.rs`. All three
are byte-identical to `HEAD`; their line counts already exceed the recorded
budgets there. Migration changes satisfy the ratchet without raising those budgets.
See [output migration coverage](output-contract-migration.md).

## GraphQL client completion checks

### Client styles and ecosystem follow-up

The [ecosystem test record](../verification-results/graphql-ecosystem-2026-10-09.json)
separates the current checks from the earlier client/scalar records below.
The combined TypeScript GraphQL suites pass **20/20**, with ignored integration
tests explicitly enabled and **zero skipped** in that run. The default workspace
suite's 143 ignored tests are reported separately.
The fresh local native addon passes the full npm SDK suite: **71 passed, 0 failed,
0 skipped**. Offline installed SDK/bundle tarballs generate the flat client and
all seven companions and regenerate without changes. The local addon override
does not prove unpublished binaries for other platforms.
Raw, flat and grouped client generation remains compatible with existing operation
exports. The flat JavaScript example now uses a configured client and compiles,
executes query/mutation/partial responses against its local server, and passes
deterministic regeneration checks.

React Query, Vue Query and SWR checks compile and execute raw, flat and grouped
packages. They exercise mounted React/SWR hooks, Vue effect scopes and reactive
variables/cache scopes, local GraphQL HTTP requests, cache identity, cancellation,
mutation retry policy and preserved GraphQL partial/error results.

Zod, Faker and MSW checks cover selected shapes, nullability versus presence,
recursive inputs, primitive scalar wire mappings, seeded fixtures, named query/
mutation handlers and real MSW interception with local-server forwarding.
Community-client substitution includes explicit package assembly and conflicting
manifest ownership rejection. The first broad run exposed a missing manifest
provider in that new fixture; it was corrected and the final full suite passes.

Cypress **15.21.1**, using headless Electron **138**, passes **3 browser tests**:
generated query/mutation requests, matching-operation interception with unrelated
operations forwarded, and preserved partial data/errors. Browser checks are
separate from typings and callback tests. The first browser harness attempt used
an uncompiled exported fixture; another interpreted an absent result field as
failure despite three passing tests. The corrected compiled runner exits cleanly.

A combined SDK tarball containing all seven companions installs into a clean
consumer, compiles strict selected-result checks, and executes its public exports
through MSW and query clients. Its dependency installation uses the npm cache;
it does not verify published Poolster binaries. React type declarations are
explicit pinned dependencies for generated React/SWR development builds.

The Rust four-style package checks pass **3/3**. The local server fixture now
consumes request bodies before simulated delay and handles cancelled connections;
this prevents expected cancellation from crashing the fixture server.

Reproduce the browser check after exporting the helper fixture through
`POOLSTER_GRAPHQL_HELPER_FIXTURE_OUTPUT` in the `graphql_helpers` integration test:

```sh
CYPRESS_CACHE_FOLDER=/path/to/pinned/cypress-cache \
POOLSTER_CYPRESS_MODULES=/path/to/pinned/node_modules \
POOLSTER_GRAPHQL_HELPER_NODE_MODULES=/path/to/pinned/helper/node_modules \
POOLSTER_TSC_JS=/path/to/typescript/lib/tsc.js \
POOLSTER_GRAPHQL_JS=/path/to/graphql/index.js \
node crates/plugins/typescript/tests/fixtures/graphql_helpers/browser-runner.cjs \
  /path/to/exported/fixture
```

The [integration guide](../reference/outputs/graphql-integrations.md) records supported options and
limitations. Dynamic selections, subscriptions in companions, infinite pagination
and helper partitioning are not established by these passes.

### Earlier operation-client increment

[Final-source test record](../verification-results/graphql-clients-2026-10-09.json)
records counts, dependency versions, artifact/log hashes and audit boundaries.

The focused TypeScript GraphQL suites pass **10 tests with ignored tests explicitly
enabled**: six existing native-client checks, two packaged-client checks and two
scalar mapping checks. Dependencies are pinned to TypeScript **5.9.3** and GraphQL
**16.14.2**. The scalar fixture initially expected an incorrect `complete` result
kind; it was corrected to the existing `success` API and its two tests pass.

Package checks use `npm pack`, an offline install into a clean consumer, strict
compilation through package exports and execution against a local GraphQL server.
They cover queries/mutations, selected fields, variables/defaults, nullability,
partial data/errors, HTTP/malformed-response/network/cancellation failures,
customization and deterministic regeneration. Scalar tests exercise independent
input/output wire types against real server-side scalar coercion, without adding
client-side codecs. The updated CLI example generates, compiles, runs and passes
`--check` with a mapped `DateTime` field.

Rust GraphQL explicitly enabled tests pass **2/2**. They pack a `.crate` archive,
unpack it into a clean consumer, compile and run against the pinned local GraphQL
server. Omitted/null/value input states, defaults, conditional results, required
nullable fields, partial/errors and network/HTTP/protocol/decoding failures are
covered, including second-variant field retention without `__typename` and
negative consumer compilation checks. The npm suite passes **67 tests, zero skips**
with the current addon; focused native CLI tests cover TypeScript, Rust and mixed
packages. Clean npm tarball installation also passes using a locally built native addon
override; this does not verify unpublished binaries for every platform. The final workspace counts above include this increment. Runtime tests enabled
separately are not counted as passes in the default suite.

### Rust scalar mapping follow-up

[Scalar mapping test record](../verification-results/rust-graphql-scalars-2026-10-09.json)
records final-source counts and artifact/log hashes.

Rust custom scalars now support independent input/output types through
`.scalar()` / `.scalars()`, recipe `plugin.scalars`, and Node `input.rustScalars`.
TypeScript keeps its separate Node `input.scalars` map, including mixed generation.
The existing two Rust packaged-client tests explicitly pass with a real custom
scalar accepting strings and returning integers, preserving nested inputs, lists,
nullability, conditional fields and omission. Unmapped JSON values remain intact;
incompatible output data produces a decoding error, and incorrectly typed input
fails consumer compilation. Five focused source tests pass, including supported
type normalization and malformed/unknown/builtin mapping rejection.

The follow-up workspace passes **801/0/141**. CLI focused tests pass **13/13**;
Node focused tests pass **7/7**, including mixed scalar maps and local-server use;
the final full npm SDK suite passes **67/67**, with zero skips.
Installed npm tarballs also generate both mapped packages with a local native
addon override. Supported Rust mappings are self-contained Serde-compatible wire
types; no extra dependencies or runtime scalar codecs are installed.

## Historical HTTP integration evidence (8 October 2026)

The integrated workspace passes 483 tests, formatting checks and Clippy with
warnings denied. The earlier shared HTTP check passed 163 scenarios across all
ten runtimes. Additional native probes cover rebuilding an ejected renderer,
local-reference provenance, OpenAPI 3.2 QUERY requests, scoped HTTP controls,
OAuth refresh and buffered multipart uploads. The expanded Swift suite passes
all 20 native probes, including a 600-query-parameter compilation/wire regression.
New Swift/PHP/Elixir OAuth and scoped-client probes pass, as do Ruby/PHP/Elixir multipart probes and
custom-method wire probes in Go, Fetch/Axios, Rust and Ruby.

The model and 3.2 expansion adds native nested unknown-enum/property and
missing/null roundtrips, explicit-null constructor helpers, whole-query and named
JSON-content parameters, sequential JSON requests/responses and ordered nested
MIME plans. Generated operations verify required null parameters and required MIME
part headers. TypeScript probes also cover lossless integers in JSON sequences.
Compiler tests retain reusable media references, device authorization, tag hierarchy,
XML metadata and `$self`; source-format revisions invalidate compiler caches.
Buffered multipart responses retain native bytes for application decoding.


The complete pinned OpenAI contract generates and compiles or imports in
TypeScript, Rust, Python, Go, Java, C# and Ruby. All six pinned public contracts
compile in Go. The manual public-contract matrix records results for ten language
targets; this does not establish that every contract works in every target.

The broader [205-contract compatibility results](guru-compatibility.md) record
2,050 passing generation/native cases across ten languages after the fixes. The
ledger preserves build provenance and retries; Swift includes an iterative prefix
with frozen failure confirmations. These checks establish compilation/import coverage,
with runtime behavior verified separately by native probes. New idempotency
regressions verify that allocated header arguments preserve caller keys and
automatic keys through collisions and retries.

These checks use local fixtures and mock HTTP servers. Live GitHub synchronization
and registry publication remain prepared workflows, without a live delivery trial.

## Verify your own package

1. Generate from the committed contract and recipe.
2. Build with the package's declared dependencies and native toolchain.
3. Exercise a generated operation: check URL, headers, body, decoding and policy.
4. Run `poolster generate --config poolster.json --check` to detect drift.
5. Put the build/test commands in `release` metadata for CI.

The [bundled middleware example](../../examples/bundled-middleware/README.md)
walks through this sequence. It verifies the author's policy runs without
customer middleware registration.

Use a fake driver for exact request assertions. Add a local mock when you need
to exercise the native HTTP stack.

## What Poolster's own tests establish

| Check | Evidence | Boundary |
| --- | --- | --- |
| Snapshot | Stable generated paths and bytes | Does not execute an SDK |
| Plugin composition | Binding, order, scope and file ownership | Does not exercise HTTP |
| Native build | Generated package and consumer compile | Needs that language's tools |
| Fixture driver | Serialization, decoding and policy behavior | Does not call a live API |
| Local mock | Real HTTP for selected contracts | Covers only selected cases |
| Delivery/broker tests | Tag selection, policy and retries | Mocked services do not prove publication |

### Output snapshots

`crates/facade/tests/golden_output.rs` checks generated runtimes, models,
READMEs and manifests across the maintained target set. Review changes before
updating the snapshot.

```sh
cargo test -p poolster --test golden_output
```

### Runtime and middleware checks

Run native probes for the transport and bundled policy you ship.
See [native providers](../reference/outputs/native-sdk-providers.md) and [shared fixtures](shared-sdk-fixtures.md).

Ignored tests state their prerequisites. Select probes by name after installing
those tools. TypeScript middleware probes require `POOLSTER_TSC_JS` and
`POOLSTER_AXIOS_NODE_MODULES` pointing to installed dependencies.

### Live contract mock

This Go/Python check uses a loopback server to verify routes, bearer headers
and decoded models:

```sh
cargo test -p poolster --test sdk_to_mock_contract -- --ignored
```

It requires a local port and the generated standalone mock fixture.

## Maintainer checks

```sh
cargo fmt --all --check
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
(cd openapi && go test ./...)
node --test packages/internal/sdk-check/test/*.mjs packages/internal/sdk-publish/test/*.mjs packages/internal/github-app-broker/*.test.mjs
```

| Requirement | Used by |
| --- | --- |
| Local port | Broker integration and loopback tests |
| Python 3.11+ or `POOLSTER_TEST_PYTHON` | Publisher archive-reader probe |
| Native toolchain/dependencies | Generated SDK compilation and execution |
| Configured repository, App and registry trust | A real delivery trial |

Ignored tests are not passes. [Recorded evidence](evidence.md)
and [compatibility results](guru-compatibility.md) describe the checks actually run.

## Postman and Terraform artifacts

### Validate a Postman collection

With Python and `jsonschema` installed:

```sh
cargo test -p poolster-plugin-postman validates_actual_official_draft04_schema -- --ignored
```

This uses the pinned official Collection 2.1 Draft04 schema.
[Collection execution →](../../packages/internal/postman-execute/README.md)

### Exercise a Terraform provider

With Go and the pinned Framework dependencies:

```sh
cargo test -p poolster-plugin-terraform generated_provider_executes_native_framework_lifecycle -- --ignored
```

The probe covers plan/state consistency, import, drift, auth, 404 handling and
identity recovery. It exercises Framework objects and mock HTTP.
[Terraform CLI lifecycle and limits →](../reference/outputs/terraform-provider.md)

## Historical input-plugin integration evidence

The latest input-plugin worktree run recorded **634 passed, one failed and
116 ignored Rust tests**. The failed output snapshot also fails on a pristine
branch base with byte-identical generated output. The Go compiler suite passed.

That run includes pinned native-input corpora and actual Cap’n Proto compilation.
[Input corpus and source-to-output checks →](../reference/inputs/input-plugins.md#upstream-corpus-and-end-to-end-coverage)

For earlier runtime checks, use [recorded integration evidence](evidence.md#latest-050-integration-evidence).

## Shared runtime contract and artifact execution

The shared [runtime contract](../../packages/internal/runtime-contract/README.md) defines
17 scenarios per target. Its manifest records supported and unsupported cases.
The recorded baseline has 163 supported passes across ten SDK runtimes.

[Runtime and artifact results →](evidence.md#shared-runtime-contract-and-artifact-execution)

## Expanded security, models and delivery sources

See the recorded checks for OAuth, webhooks, retry safety, model evolution,
Postman synchronization and Terraform release scaffolding.

[Security, models and delivery results →](evidence.md#expanded-security-models-and-delivery-sources)

## Additional native coverage

Large-contract, streaming, multipart and native model probes have their own
requirements and recorded results.

[Additional native results →](evidence.md#additional-native-coverage)

## Terraform lifecycle polling

Recorded Framework and local CLI probes cover bounded read-GET waiters,
plan/state consistency and deletion. General multi-step mutation workflows
remain unsupported.

[Polling results →](evidence.md#terraform-lifecycle-polling)

## Live delivery

The [delivery test workflow](../../packages/internal/sdk-delivery-test/README.md) is prepared.
Live GitHub synchronization and registry publication have not been established
by the mocked tests.

[Configure delivery](../reference/automation/sdk-automation.md) · [Publish releases](../reference/automation/sdk-publishing.md)


## Native GraphQL delivery verification (8 October 2026)

The historical alpha.1 GraphQL implementation run from main passed
`cargo test --workspace --locked --no-fail-fast`: **688 passed, 0 failed,
130 ignored**. Formatting and workspace Clippy with warnings denied pass.
Ignored tests are not counted as passes.

The selected GraphQL native suite passes **6 tests with ignored tests included**
using TypeScript 5.9.3 and GraphQL 16.14.2. It compiles emitted packages and
consumers, executes queries/mutations against a real local GraphQL HTTP server,
checks abstract selections/fragments/conditional presence, anonymous operations,
partial results, root errors, HTTP/malformed response failures and cancellation.
It also checks provider substitution, regeneration, separately injected async
subscriptions, pinned GitHub schema compilation, symbol collisions and a Post
consumer using actual generated symbols. This does not establish a bundled
network subscription implementation.

```sh
POOLSTER_TSC_JS=/path/to/typescript/lib/tsc.js \
POOLSTER_GRAPHQL_JS=/path/to/graphql/index.js \
cargo test -p poolster-plugin-typescript --test graphql_native -- --include-ignored
```

CI installs these pinned dependencies and runs this selection explicitly.
The runnable `examples/graphql-native` recipe additionally generates, compiles
and executes its local server/consumer demonstration.

The existing recipe JSON-schema ignored check passes with Python 3.12 and
jsonschema 4.23.0. An initial default-Python attempt failed because the dependency
was absent; using the matching installed interpreter/dependency resolved it.
The native example validates against the actual recipe schema; conflicting or
missing input selectors are rejected.

The Node SDK/unplugin suite passes **67 tests**, and the full Go OpenAPI compiler
suite passes. The remaining ordinary Node CI selections pass **85 tests** with no
skips, and the Python launcher/collection-checker suites pass **5 tests**. Node
tests initially lacked the local addon/compiler and Newman prerequisites; after
supplying them, all passed. The archive-reader probe initially skipped with the
default Python and passed with Python 3.12. Existing OpenAPI output snapshots pass unchanged
on both main and the implementation worktree. Earlier snapshot failure records
above describe earlier runs, not this delivery.
The optional pristine-main full comparison was stopped after the snapshot and
provider checks passed; a complete main-baseline run is not claimed. The complete
688-test pass refers to the isolated implementation worktree.

Core graph regressions establish heterogeneous providers feeding a transformation
plugin, same-type replacements through explicit handles, dependency ordering,
native skipped-package reports, and preserving skipped owned files/local edits
without adopting modified hashes. CLI tests check actual JSON skip reports,
relative operation paths, check/regeneration and preserved OpenAPI recipes.

At that alpha.1 delivery boundary, no usable RPC/event/workflow/Cap’n Proto/Cap’n
Web output or Forge compatibility was established. The alpha.2 status below
supersedes that boundary for gRPC, Kafka and sequential workflow generation.

## GraphQL Go/Python outputs and Rust call ergonomics

The [test record](../verification-results/graphql-go-python-rust-styles-2026-10-09.json)
records the latest checks. The npm SDK suite passes 75/75 with no skipped tests.
The CLI suite and all changed output crate suites pass; ignored tests are recorded
separately, including explicitly executed pinned GraphQL.js 16.14.2 runtime probes.
Go and Python package compilation/runtime checks cover native GraphQL generation;
Rust packaged tests cover raw, flat, idiomatic and custom grouped clients.

Go/Python limitations remain documented in their usage guides. This verification
does not rerun the full workspace or the earlier 205-spec compatibility corpus.

## GraphQL remaining SDK language outputs

The [test record](../verification-results/graphql-other-languages-2026-10-09.json)
tracks PHP, Java, C#/DotNet, Ruby, Swift and Elixir, completing fixed-operation
GraphQL clients across all ten SDK languages. Each generated output is compiled
and executed against GraphQL.js **16.14.2**, covering raw, flat, default groups and
custom groups. Language toolchains and ignored-test counts are recorded separately. Explicit GraphQL
checks pass PHP **5**, Java **6**, C# **3**, Ruby **4**, Swift **3**, Elixir **4**;
these counts include externally enabled compilation/runtime tests and repeat some
ordinary checks rather than forming an additional distinct-test total.

The shared CLI suite passes **127 tests, 0 failures, 9 ignored**. The npm SDK suite
passes **76 tests, 0 failures, 0 skips**, including the existing HTTP configuration
paths. Changed crates, CLI and Node pass all-target Clippy with warnings denied.
Formatting and local documentation links pass. The full workspace and 205-spec
HTTP corpus were not rerun for this batch.

These checks cover fixed query/mutation documents, selection models, input
presence/null handling, partial GraphQL results, malformed responses, provider
substitution and regeneration. They do not establish uniform abstract-type or scalar
codec support: consult each [language guide](../reference/README.md) and the
[support matrix](../plugin-support-matrix.md#graphql-language-boundaries).
Subscriptions remain a distinct transport capability; the new outputs reject them.
All changes remain unreleased.
