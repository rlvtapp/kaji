# Verify an SDK before shipping it

Generation, compilation and API behavior are separate checks. Run all three
for the package you ship.

## Latest 0.5.0 integration evidence

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

The [bundled middleware example](../examples/bundled-middleware/README.md)
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
See [native providers](native-sdk-providers.md) and [shared fixtures](shared-sdk-fixtures.md).

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

Ignored tests are not passes. [Recorded evidence](verification/evidence.md)
and [compatibility results](guru-compatibility.md) describe the checks actually run.

## Postman and Terraform artifacts

### Validate a Postman collection

With Python and `jsonschema` installed:

```sh
cargo test -p poolster-plugin-postman validates_actual_official_draft04_schema -- --ignored
```

This uses the pinned official Collection 2.1 Draft04 schema.
[Collection execution →](../packages/internal/postman-execute/README.md)

### Exercise a Terraform provider

With Go and the pinned Framework dependencies:

```sh
cargo test -p poolster-plugin-terraform generated_provider_executes_native_framework_lifecycle -- --ignored
```

The probe covers plan/state consistency, import, drift, auth, 404 handling and
identity recovery. It exercises Framework objects and mock HTTP.
[Terraform CLI lifecycle and limits →](terraform-provider.md)

## Latest 0.5.0 integration evidence

The latest input-plugin worktree run recorded **634 passed, one failed and
116 ignored Rust tests**. The failed output snapshot also fails on a pristine
branch base with byte-identical generated output. The Go compiler suite passed.

That run includes pinned native-input corpora and actual Cap’n Proto compilation.
[Input corpus and source-to-output checks →](input-plugins.md#upstream-corpus-and-end-to-end-coverage)

For earlier runtime checks, use [recorded integration evidence](verification/evidence.md#latest-050-integration-evidence).

## Shared runtime contract and artifact execution

The shared [runtime contract](../packages/internal/runtime-contract/README.md) defines
17 scenarios per target. Its manifest records supported and unsupported cases.
The recorded baseline has 163 supported passes across ten SDK runtimes.

[Runtime and artifact results →](verification/evidence.md#shared-runtime-contract-and-artifact-execution)

## Expanded security, models and delivery sources

See the recorded checks for OAuth, webhooks, retry safety, model evolution,
Postman synchronization and Terraform release scaffolding.

[Security, models and delivery results →](verification/evidence.md#expanded-security-models-and-delivery-sources)

## Additional native coverage

Large-contract, streaming, multipart and native model probes have their own
requirements and recorded results.

[Additional native results →](verification/evidence.md#additional-native-coverage)

## Terraform lifecycle polling

Recorded Framework and local CLI probes cover bounded read-GET waiters,
plan/state consistency and deletion. General multi-step mutation workflows
remain unsupported.

[Polling results →](verification/evidence.md#terraform-lifecycle-polling)

## Live delivery

The [delivery test workflow](../packages/internal/sdk-delivery-test/README.md) is prepared.
Live GitHub synchronization and registry publication have not been established
by the mocked tests.

[Configure delivery](sdk-automation.md) · [Publish releases](sdk-publishing.md)
