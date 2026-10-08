# Test generated SDKs

Choose a test layer for the behavior you need. Generated samples supplement
service integration tests; skipped operations do not count as coverage.

| Tool | Use it for | Limit |
| --- | --- | --- |
| Faker | Schema-shaped data | Samples need scenario-specific validation |
| MSW | In-process frontend tests | Does not establish shared HTTP behavior |
| Docker mock | SDK integration over real HTTP | Does not model stateful product workflows |
| Cypress scaffold | Starting API smoke tests | Needs paths, credentials, bodies and assertions |
| Operation tests | Public method serialization and decoding | Uses in-memory drivers, not a live API |
| Native runtime corpus | Cross-language behavior | Consult its explicit scenario exclusions |

## Docker contract mock

Add a mock package to your recipe:

```json
{ "language": "mock", "path": "mock-server", "plugins": [{ "name": "server", "port": 4010 }] }
```

Run it and configure SDK clients with its base URL:

```sh
cd generated/mock-server
docker compose up --build
```

The Docker mock returns deterministic contract-derived happy paths. Put durable
conditional responses in `x-kaji-mock`; keep stateful workflows in a dedicated
service. See [contract mocking](../mocking.md).

MSW handlers and Cypress tests are editable scaffolding. Review mutation tests
before execution and use a sandbox. The [TypeScript stack example](../../examples/typescript-stack/README.md)
combines Faker, MSW, Cypress and the Docker mock.

## Generate operation smoke tests

All ten SDK recipes accept `operation-tests` beside `sdk`:

```json
{
  "language": "python",
  "path": "python",
  "package_name": "my-api-sdk",
  "api_reference": true,
  "plugins": [{ "name": "sdk" }, { "name": "operation-tests" }]
}
```

Tests call public operations through in-memory native HTTP drivers. They check
method, path, parameters, JSON bodies and decoded success. Samples are bounded
and omit specification examples/defaults to avoid copying secrets.

Review the emitted report and `.kaji/operation-test-diagnostics.json` before
claiming coverage. Unsupported operations, bounds and empty supported sets need
explicit attention. Authentication, streaming, complex constraints and real
server behavior need dedicated fixtures.

### Run the emitted tests

| Target | Command |
| --- | --- |
| TypeScript | Build with the package tsconfig, then `node dist/tests/operation-tests.js` |
| Python | Install locally, then `python -m unittest discover -s tests -v` |
| Go | `go test ./...` |
| Rust | `cargo test` |
| Ruby | `ruby test/operation_tests.rb` |
| Swift | Native compiler command in `OPERATION_TESTS.md` |
| PHP | `composer install && php tests/operations.php` |
| Elixir | `mix deps.get && mix run test/operations_test.exs` |
| Java | Maven test-classpath command in `OPERATION_TESTS.md` |
| C# | `dotnet run --project tests/OperationTests/OperationTests.csproj` |

The C# probe project is excluded from SDK compile inputs. TypeScript and Rust
also emit `OPERATION_TESTS.md`.

## Opt-in generated operation checks

TypeScript JSON consumers can bind `uses.models`, `uses.operations` and
`uses.transport` to named compatible providers. Native custom providers can
select typed handles through the Rust API, including `kaji::go::operation_tests()`.

Ruby, Swift, PHP, Elixir, Java and C# JSON recipes infer the bundled SDK. Where
custom provider selection is exposed, use the Rust API rather than ignored JSON
`uses` bindings.

Ruby and Swift have local native probes. PHP, Elixir, Java and C# execution is
selected in CI and has not run locally. These scope statements describe the
repository's recorded verification, not universal consumer compatibility.

## Verify runtime behavior across languages

The [runtime contract](../../packages/runtime-contract/README.md) generates ten
SDKs and checks 17 scenarios through a loopback service. Scenarios cover auth,
middleware, errors, retries, decoding, mutation replay, idempotency key lifetime,
page iteration and exact path/query/body serialization.

Consult its coverage table and unsupported-scenario manifest. Reproduce a check:

```sh
KAJI_RUNTIME_EXPORT=/tmp/kaji-runtime-fixture cargo test -p kaji --test runtime_conformance
node packages/runtime-contract/runner.mjs go /tmp/kaji-runtime-fixture/sdk/go
node --test packages/runtime-contract/test.mjs packages/sdk-delivery-test/test/*.mjs
```

The [installed TypeScript check](../../packages/runtime-contract/README.md#installed-typescript-package)
verifies ESM exports and customer types. Source snapshots and compilation alone
cannot establish that a published package imports successfully.

### Other integration checks

[Postman execution](../../packages/postman-execute/README.md) runs Newman against
a local service. The [Terraform suite](../terraform-provider.md) runs a real CLI
lifecycle against a mock.

The synthetic complex OpenAPI corpus uses the actual Go compiler and executes
Swift models. It covers recursion, Unicode, nulls, unknown fields, unions and
identifier collisions. It does not prove third-party contract support in every
target.

## Pinned public contract compilation

Generate all ten SDKs from two immutable official Open-Meteo contracts:

```sh
export KAJI_PUBLIC_CONTRACT_ROOT=/tmp/kaji-public-contracts
bash scripts/test-public-contracts.sh generate
bash scripts/test-public-contracts.sh check LANGUAGE
```

Use `KAJI_PUBLIC_SPEC_DIR` for a local cache. Exact SHA-256 verification still
runs; pins live in `scripts/fixtures/public-contracts.json` and require deliberate
review when changed.

CI performs native compile/import checks in each language job. Generation and
compilation do not contact the weather APIs. Combine these checks with operation
and wire tests before distributing an SDK.
