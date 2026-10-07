# Generated SDK runtime contract

This suite generates one neutral API and drives its public `getContact` operation through a real loopback HTTP server. Every harness registers SDK-consumer middleware and sends a dummy bearer credential. The server checks the encoded method/path, authentication, middleware header, exact number of transport attempts, and the client's decoded result/error outcome. It does not contact a remote API.

Generate all fixtures from the repository root:

```sh
KAJI_RUNTIME_EXPORT=/tmp/kaji-runtime-fixture cargo test -p kaji --test runtime_conformance
node packages/runtime-contract/runner.mjs go /tmp/kaji-runtime-fixture/sdk/go
node packages/runtime-contract/runner.mjs python /tmp/kaji-runtime-fixture/sdk/python
```

`runner.mjs` accepts a language and that generated package's directory. It copies the package into a temporary workspace before adding the probe/building, and removes the workspace on completion. The generated source artifact remains unchanged. Child failures, timeouts, missing requests and wire mismatches fail the run; they cannot pass as an SDK's expected error outcome.

`scenarios.json` is the shared executable contract. It covers success, future response fields, a safe GET receiving 503 then 200, a nonretryable 400, and malformed JSON. Its coverage map must account for every scenario as supported or explicitly unsupported. “Supported” means the harness asserts this scenario, not that every runtime has passed on every platform.

| Runtime | Harness | Requirements | Shared scenarios |
| --- | --- | --- | --- |
| TypeScript Fetch | Native generated public `contacts.get` | Node 22+, TypeScript compiler path in `KAJI_TSC_JS` | All five |
| Python sync | Native generated `Client.get_contact` | Python 3.10+ | All five |
| Go | Native generated `Client.GetContact` | Go 1.22+ | All five |
| Rust | Native generated `Client.get_contact` | Cargo; generated reqwest/serde/tokio dependencies | All five |
| Swift | Native generated `KajiClient.getContact` | Swift 5.9+ with Foundation | Four; automatic retries unsupported |
| Ruby | Native generated `Client.get_contact` | Ruby 3.1+ | Three; automatic retries and malformed-JSON rejection unsupported |
| Java | Native generated `Client.getContact` and HttpClient decorator | Java 17, Maven | All five; require native CI execution |
| C# | Native generated `KajiClient.GetContactAsync` and DelegatingHandler | .NET 8 SDK | All five; require native CI execution |
| PHP | Native generated `Client.getContact` and PSR-18 decorator | PHP 8.2+, Composer; stream HTTP enabled | All five; require native CI execution |
| Elixir | Native generated `API.get_contact` and Finch middleware | Elixir 1.17+, Mix/Hex | All five; require native CI execution |

TypeScript uses the Fetch transport. Axios, async Python, pagination, SSE, body encodings, concurrency, cancellation, strict response validation and advanced auth are **outside this initial shared wire suite**; language-specific tests currently exercise some of those behaviors. Passing this suite does not claim complete parity. Unknown fields are accepted here; preservation is not asserted in every language.

Run package-level runner tests with `node --test packages/runtime-contract/test.mjs`. These validate manifest coverage, child failure handling and the shared server itself.

For CI, export the fixture once, upload `sdk/` as an artifact, and run each language's probe in the existing native toolchain matrix. Each job needs Node for the orchestrator in addition to its language toolchain. Install the TypeScript compiler explicitly and set `KAJI_TSC_JS`; initialize Hex for Elixir. Maven/Composer/Mix/Cargo builds may download the generated package's declared dependencies. Set `KAJI_RUNTIME_OFFLINE=1` to require Cargo's existing module cache; `KAJI_RUNTIME_RUST_TARGET` and `GOCACHE` may point to reusable CI caches. Unsupported scenarios print their reasons, and missing native tools fail rather than silently skip.
