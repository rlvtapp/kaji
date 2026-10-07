# Verify an SDK before shipping it

A successful generation proves that Kaji assembled source. It does not prove
that your package compiles, that a request reaches the right endpoint, or that
custom middleware preserves response decoding. SDK authors should check those
properties separately and make their release workflow run the same checks.

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
