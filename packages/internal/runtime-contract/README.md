# Generated SDK runtime contract

This suite generates one neutral API and drives its public read, mutation, pagination and serialization operations through a real loopback HTTP server. Every harness registers SDK-consumer middleware and sends a dummy bearer credential. The server checks the encoded method/path, authentication, middleware header, exact number of transport attempts, and the client's decoded result/error outcome. It does not contact a remote API.

Generate all fixtures from the repository root:

```sh
KAJI_RUNTIME_EXPORT=/tmp/poolster-runtime-fixture cargo test -p poolster --test runtime_conformance
node packages/internal/runtime-contract/runner.mjs go /tmp/poolster-runtime-fixture/sdk/go
node packages/internal/runtime-contract/runner.mjs python /tmp/poolster-runtime-fixture/sdk/python
```

`runner.mjs` accepts a language and that generated package's directory. It copies the package into a temporary workspace before adding the probe/building, and removes the workspace on completion. The generated source artifact remains unchanged. Child failures, timeouts, missing requests and wire mismatches fail the run; they cannot pass as an SDK's expected error outcome.

`scenarios.json` is the shared executable contract with 17 scenarios. It checks successful and failed responses, unknown fields, safe retries, POST/PATCH replay protection, automatic UUID keys, caller overrides (including empty/blank keys), fresh keys between calls, page iteration, strict missing/wrong-field rejection, and serialization of Unicode paths, repeated query arrays, false, zero, null and omitted values. The server checks exact attempt counts and stable keys across retries.

| Runtime | Supported / 17 | Explicit limitations |
| --- | --- | --- |
| TypeScript Fetch | 17 | Axios has separate native probes |
| Python sync | 17 | Async/OAuth have separate native probes |
| Go | 17 | |
| Rust | 17 | |
| C# | 17 | Native execution requires .NET CI |
| Java | 15 | Permissive missing/wrong-field decoding; native execution requires Java/Maven CI |
| PHP | 15 | Permissive missing/wrong-field decoding; native execution requires PHP/Composer CI |
| Elixir | 15 | Permissive missing/wrong-field decoding; native execution requires Elixir/Mix CI |
| Swift | 17 | Harness enables opt-in retries |
| Ruby | 16 | Harness enables opt-in retries; default malformed JSON is returned as text |

“Supported” means implemented assertions, not proof of a passing run on every platform. TypeScript, Python, Go, Rust, Swift and Ruby were executed locally (101 supported scenarios). Java, C#, PHP and Elixir await native CI execution. Every scenario must have a supported or unsupported entry in the manifest; missing tools fail the runner.

This corpus covers a small neutral API. It does not establish exhaustive SSE, multipart, date, union, wide-integer or authentication coverage. Unknown fields are accepted; preservation is not asserted in every language. Dedicated native probes additionally cover middleware, cancellation, OAuth refresh concurrency, nested models and pagination. A passing suite does not claim complete runtime parity.

TypeScript requires `KAJI_TSC_JS` pointing at an installed TypeScript compiler. Other targets need their native compiler/runtime and the generated package's dependencies. See `.github/workflows/ci.yml` for the pinned matrix setup.

Run package-level runner tests with `node --test packages/internal/runtime-contract/test.mjs`. These validate manifest coverage, child failure handling and the shared server itself.

For CI, export the fixture once, upload `sdk/` as an artifact, and run each language's probe in the existing native toolchain matrix. Each job needs Node for the orchestrator in addition to its language toolchain. Install the TypeScript compiler explicitly and set `KAJI_TSC_JS`; initialize Hex for Elixir. Maven/Composer/Mix/Cargo builds may download the generated package's declared dependencies. Set `KAJI_RUNTIME_OFFLINE=1` to require Cargo's existing module cache; `KAJI_RUNTIME_RUST_TARGET` and `GOCACHE` may point to reusable CI caches. Unsupported scenarios print their reasons, and missing native tools fail rather than silently skip.

## Installed TypeScript package

Test the artifact a customer actually installs, including ESM root/subpath exports and NodeNext consumer types:

```sh
KAJI_PACKAGE_FIXTURE=/tmp/poolster-runtime-fixture/sdk/typescript \
KAJI_TSC_JS=/path/to/typescript/lib/tsc.js \
node --test packages/internal/runtime-contract/package-consumer.test.mjs
```

This builds, packs and installs a local archive without publishing. The test is skipped when either variable is absent; CI supplies both explicitly.
