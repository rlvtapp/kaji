# Native Rust and Go SDK providers

Rust and Go expose independent model, transport, operation and client providers.
Their `sdk()` convenience plugins reuse the maintained renderers and publish
matching contracts.

## Choose the composition

| Need | Provider |
| --- | --- |
| Complete SDK | `sdk()` |
| Models without a client | `models()` |
| Replace HTTP execution | `transport()` or a compatible community provider |
| Raw operation methods | `operations()` with model and transport handles |
| Resource facades | `client()` with the operations handle |
| Bounded codec checks | `roundtrip_tests()` with the model handle |

### Rust recipe

```rust
let models = poolster_plugin_rust::models();
let models_handle = models.models_handle();
let transport = poolster_plugin_rust::transport();
let transport_handle = transport.transport_handle();
let operations = poolster_plugin_rust::operations()
    .using_models(models_handle)
    .using_transport(transport_handle);
let operations_handle = operations.operations_handle();
let package = poolster_plugin_rust::package("rust")
    .with(models)
    .with(transport)
    .with(operations)
    .with(poolster_plugin_rust::client().using_operations(operations_handle))
    .with(poolster_plugin_rust::roundtrip_tests().using_models(models_handle));
```

The corresponding Go API uses `poolster_plugin_go` and exports contracts through `poolster_plugin_go::providers`; Rust contracts are in `poolster_plugin_rust::composition`. Explicit handles select providers and preserve useful ambiguity diagnostics. Model-only packages and raw operation packages are supported. In native clients, operations remain methods on `Client`; selecting the client provider adds resource facades rather than changing their calling convention.

## Community transports

### Rust ABI

Rust transport plugins emit a root module and publish `composition::Transport { module, constructor }`.
The module exports a `Transport: Send + Sync` trait whose `execute(reqwest::Request)` returns a boxed, `Send` future producing `Result<reqwest::Response, reqwest::Error>`.
The constructor expression creates the default executor.

Generated operations continue handling encoding, authentication, retries, response decoding, and typed errors.
Clients also expose `with_transport` for per-instance substitution.
The maintained request/response ABI still depends on reqwest; replacing the HTTP execution boundary does not make Rust clients independent of reqwest.

### Go ABI

Go transport plugins emit an implementation in the generated package and publish `providers::Transport { constructor }`. Its constructor returns `PoolsterHTTPClient`, an interface with `Do(*http.Request) (*http.Response, error)`. `ClientConfig.HTTPClient` accepts that interface as a per-instance override. Requests carry context cancellation. Compatible model providers retain the conventional native module/package symbols; arbitrary module relocation is not implemented.

### Verify a replacement

Both crates include native generated-package tests demonstrating explicit community transport selection. Go executes requests through a fake provider and validates returned JSON. Rust compiles a custom executor and polls an operation to prove the selected executor is called, without contacting an external service.

## Executable model fixtures

`roundtrip_tests()` consumes the actual published model symbols and emits bounded decode/encode assertions from shared schema samples. Rust emits a unit-test module linked by package finalization; Go emits a `_test.go` file. `.poolster/roundtrip-diagnostics.json` records constraints or recursive shapes needing custom fixtures. Assertions compare JSON values and retain integer digits rather than comparing object key order. Unknown response enum/union policies still need separate unknown-value fixtures.

Round-trip coverage includes additional properties and optional nullable fields. Rust retains open additional properties through serde flattening and preserves the distinction between omitted and null optional values. Go generated object codecs preserve additional wire keys and retain decoded explicit nulls on optional nullable fields. Missing constraints, unsupported schema formats, and overlapping unions remain limitations documented in the shared fixture guide.

## Pagination migration boundaries

Go cursor and URL renderers consume shared normalized plans, including selector projection and schema checks. Offset rendering shares input and selector validation while retaining legacy opaque response envelopes. Optional Go JSON request bodies retain their existing continuation behavior. Unsupported native capabilities still omit a pager instead of implementing guessed behavior.

Rust supports native page-number streams for scalar integer parameter controls,
including required headers/paths and optional starts. Optional legacy page/offset
controls now default to 1/0, and checked arithmetic prevents saturating-counter
loops. Shared declarations and selectors preserve the native public stream API.
Body and next-URL continuations remain unsupported and are reported in pagination
diagnostics. See the [target capability table](../../guides/pagination.md).

Rust cursor rendering now shares scalar input and selector normalization. Rust response-reference validation and legacy offset/page rules remain in its maintained renderer. Neither this work nor the shared plan automatically adds every pagination kind to both language runtimes. The standalone normalized-plan API provides stricter diagnostics for consumers that require complete declaration validation.

These provider recipes are native Rust generator APIs. The CLI configuration has not yet gained explicit Rust/Go provider bindings.

## Rust runtime middleware

The default generated `transport` module also exposes `Middleware` and `MiddlewareTransport`. A customer implements `handle(request, next)`, returning the same `TransportFuture` as an executor. It owns the fully encoded request, can change headers/body, await `next.execute(request)`, change the response, propagate or recover a transport error, or return a synthetic response without calling `next`. Nest wrappers to compose layers:

```rust
use std::sync::Arc;
use generated_sdk::transport::{DefaultTransport, MiddlewareTransport};
let transport = MiddlewareTransport::new(logging_layer,
    MiddlewareTransport::new(customer_auth_layer, DefaultTransport::default()));
let client = generated_sdk::Client::new(base_url).with_transport(Arc::new(transport));
```

`logging_layer` and `customer_auth_layer` are customer implementations of the generated `Middleware` trait.
Outer layers run first for requests and last for responses.
Middleware runs inside the SDK retry loop, once per executor attempt; it should not independently retry unsafe operations.
Dropping the composed future cancels its pending work.

HTTP status and response decode errors are classified after middleware, while transport errors retain the existing `reqwest::Error` ABI.
Existing observational `ClientHooks` remain available.
Custom generator transport providers can expose their own middleware surface; the default middleware types are not automatically inserted into a replacement module.

Executable generated tests verify request mutation, nesting order, response-header mutation, transport-error propagation/recovery, and a synthetic response through a generated operation with zero terminal transport calls.

## Other native transport extension boundaries

This is a source audit, not an execution claim for every runtime.

| Language | Customer extension point | Practical boundary |
|---|---|---|
| Java | `ClientConfig.httpClient` accepts a JDK `HttpClient`; a custom subclass can intercept execution. | Lifecycle callbacks only receive method/URI, status, and error; they cannot replace requests/responses directly. |
| C# | Injected `HttpClient` supports ordinary `DelegatingHandler` chains, including synthetic responses. | Generated hooks are separate notifications; binary/SSE paths do not consistently invoke the before-request hook. |
| PHP | Injected PSR-18 `ClientInterface` supports transport decorators and middleware adapters. | Generated callbacks observe context/outcomes; their return values do not replace requests or responses. |
| Swift | `PoolsterTransport` and `PoolsterMiddlewareTransport` support request/response/error transformations and short circuits; `URLSession` initialization remains supported. | Buffered transport only; status and decode errors occur after middleware. Notification hooks remain observational. |
| Ruby | Callable `transport` and ordered `middleware` wrap Net::HTTP execution. | Buffered calls; request/response/error rewriting and short circuits. Opt-in bounded replay-safe retries; cancellation callback between attempts. No SSE surface. |
| Elixir | `transport` function and ordered `middleware` continuations support buffered transformations/recovery/short circuits; `stream_transport` decorates Finch-style SSE execution/events. | Buffered middleware runs per retry attempt and does not process SSE frames; callbacks remain observational. |

Swift generated README examples show `PoolsterMiddlewareTransport(inner:middleware:)` with a mutable `URLRequest` and async continuation. The `PoolsterTransport` protocol returns `(Data, URLResponse)` and preserves existing `session:` callers. A Swift 6 warnings-as-errors executable test verifies header mutation, response transformation, transport-error recovery, ordering, and a short circuit without terminal execution.

Elixir generated README examples show ordered `middleware: [fn request, next -> ... end]`, optional `transport: fn request, options -> ... end`, and the separate `stream_transport` signature.
The generated dependency-free ExUnit probe covers buffered mutation/recovery/short circuits, error notifications, and synthetic SSE.

Elixir is unavailable in the verification environment; that probe is committed as an ignored toolchain test and has not been executed here.
Run `cargo test -p poolster-plugin-elixir elixir_customer_middleware_executes -- --ignored` with Elixir installed.
Source-generation tests execute normally.

## Generic documentation and custom consumers

Native source formats enter through [input providers](../inputs/input-plugins.md). A consumer
requests their native contracts; HTTP SDK providers continue consuming the
normalized HTTP API.

The core `api_reference::<L>()` consumer works with native providers, the SDK
convenience plugin and community languages. Add it through `.with(...)` or enable
package `api_reference: true` in CLI configuration. It emits an owned
`API_REFERENCE.md` with operation/parameter/status/media/schema structure from the
normalized API. It has no hard-coded SDK imports or model-name assumptions.
Examples/defaults and credential-bearing external reference URIs are omitted.

For a compiled provider substitution example with no SDK dependencies, see
[custom plugin authoring](../../../examples/custom-plugin/README.md). For the API
reference contract and custom output path, see
[typed plugin documentation](../../internals/typed-plugins.md#add-a-target-neutral-api-reference).
Generated documentation and source probes do not replace native consumer/runtime
conformance tests.
