# Test generated SDKs

Choose the test layer that matches the behavior you need.

| Tool | Best for | Not for |
| --- | --- | --- |
| Faker | Schema-shaped test data | Validated/scenario-specific fixtures |
| MSW | In-process frontend tests | Shared HTTP behavior across languages |
| Docker mock | SDK integration through real HTTP | Stateful product behavior |
| Cypress scaffold | Starting API smoke tests | A complete e2e suite |

## Docker contract mock

Add a separate mock package to the recipe:

```json
{ "language": "mock", "path": "mock-server", "plugins": [{ "name": "server", "port": 4010 }] }
```

Then run the generated service and point SDK clients at it:

```sh
cd generated/mock-server
docker compose up --build
```

It returns deterministic contract-derived happy paths. Add durable conditional
responses with `x-kaji-mock` in the OpenAPI source. Keep stateful workflows in
a dedicated test service. Read [contract mocking](../mocking.md) for the full
scenario format.

MSW handlers are editable in-process scaffolding. Cypress output needs real
paths, credentials, bodies, and assertions before use; never run generated
mutation tests against production. The [TypeScript stack example](../../examples/typescript-stack/README.md)
shows Faker, MSW, Cypress, and the Docker mock together.

## Generate operation smoke tests

Python authors can add an `operation-tests` consumer beside their SDK:

```json
{
  "language": "python",
  "path": "python",
  "package_name": "my-api-sdk",
  "api_reference": true,
  "plugins": [{ "name": "sdk" }, { "name": "operation-tests" }]
}
```

Run `python -m unittest discover -s tests -v` from the generated package after
installing it locally. The emitted tests call public SDK operations through a
fake HTTP driver and assert contract-derived method, path, parameters, JSON body
and decoded result. They do not call a real API. `.kaji/operation-test-diagnostics.json`
lists operations that cannot safely be sampled; generated skips are not coverage.
Samples are bounded and avoid copying specification examples or defaults.

The Rust library also exposes `kaji::go::operation_tests()`; select its client or
operations provider with a typed handle. Run the resulting Go tests with `go test
./...`. These smoke tests supplement a service integration suite.

## Verify runtime behavior across languages

The [runtime contract](../../packages/runtime-contract/README.md) generates one
API for ten targets and exercises authentication, middleware, errors, retries and
JSON decoding through a loopback server. The CI matrix runs the native harnesses;
its manifest explicitly records unsupported scenarios. See its coverage table
before treating a passing job as full runtime parity.

[Postman execution](../../packages/postman-execute/README.md) runs generated
collections through Newman against a local server. Terraform's native suite also
runs a real Terraform CLI lifecycle against a local service; see the
[provider guide](../terraform-provider.md).

For a reproducible native check, export the corpus and run a target:

```sh
KAJI_RUNTIME_EXPORT=/tmp/kaji-runtime-fixture cargo test -p kaji --test runtime_conformance
node packages/runtime-contract/runner.mjs go /tmp/kaji-runtime-fixture/sdk/go
node --test packages/runtime-contract/test.mjs packages/sdk-delivery-test/test/*.mjs
```

The corpus includes 17 scenarios covering mutation replay safety, idempotency key
lifetime, page iteration, decoding failures and exact path/query/body serialization.
Use its [installed package check](../../packages/runtime-contract/README.md#installed-typescript-package)
to verify TypeScript ESM exports and customer types. Source snapshots and compilation
alone cannot establish that an installed package imports successfully.

## Opt-in generated operation checks

All ten SDK language recipes accept `{"name":"operation-tests"}`
beside `sdk`. TypeScript consumers can bind `uses.models`, `uses.operations` and
`uses.transport` to named compatible providers. Native custom providers can opt
in through the typed handles documented by the plugin.

The emitted `OPERATION_TESTS.md` (TypeScript/Rust) describes execution. TypeScript
builds with its package tsconfig and runs `node dist/tests/operation-tests.js`;
Rust runs `cargo test`; Go runs `go test ./...`; Python uses unittest discovery.
They call actual public operations through in-memory native HTTP drivers, assert
serialization and decoded success, and never contact a live API. Bounded structural
samples omit specification examples/secrets. Explicit diagnostics list unsupported
operations and sample bounds; skipped operations are not test coverage.

The complex checked-in OpenAPI corpus additionally passes through the actual Go
compiler and executes Swift models. It covers recursive references, Unicode,
nullable values, unknown fields, unions and rejected identifier collisions. This
is a synthetic regression corpus, not proof that large third-party specifications
compile in every target.


The remaining targets also emit executable public-operation probes with in-memory
HTTP drivers. Select `{"name":"operation-tests"}` beside `{"name":"sdk"}`:

| Target | Generated command |
| --- | --- |
| Ruby | `ruby test/operation_tests.rb` |
| Swift | See `OPERATION_TESTS.md` for the native compiler command |
| PHP | `composer install && php tests/operations.php` |
| Elixir | `mix deps.get && mix run test/operations_test.exs` |
| Java | See `OPERATION_TESTS.md` for Maven test-classpath execution |
| C# | `dotnet run --project tests/OperationTests/OperationTests.csproj` |

Each target emits a report of supported cases and exclusions. Generated samples
cover bounded buffered-operation serialization; authentication, streaming, complex
constraints and real server behavior require dedicated fixtures. An empty supported
set is not evidence of operation coverage. The C# test project is excluded from the
SDK project's compile inputs. PHP/Elixir/Java/C# native execution is selected in CI
and has not been run on the local machine. Ruby and Swift have native local probes.
These six JSON recipes infer the bundled SDK; custom typed-provider selection, where
exposed by the plugin, uses the Rust API instead of ignored JSON `uses` bindings.

## Pinned public contract compilation

`bash scripts/test-public-contracts.sh generate` downloads two immutable official
Open-Meteo contracts, verifies their SHA-256 hashes and generates all ten native
SDK targets. Set `KAJI_PUBLIC_CONTRACT_ROOT` to a disposable directory. Use
`bash scripts/test-public-contracts.sh check LANGUAGE` for each generated language.
`KAJI_PUBLIC_SPEC_DIR` allows a local input cache but still verifies exact hashes.
Neither generation nor compilation contacts the weather APIs. CI compiles each
contract in the language job with its native toolchain; these are compile/import
checks, complementing the separate operation/wire tests. Pins are recorded in
`scripts/fixtures/public-contracts.json`; changing a pin requires deliberate review.
