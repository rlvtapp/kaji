# Poolster Python plugin

`poolster-plugin-python` renders SDK packages from Poolster's neutral API model,
using a standard-library Python client. All generation runs in Rust.

```rust
use poolster::prelude::*;
use poolster_plugin_python::PackageExt as _;
use poolster_plugin_python as python;

let release = ProfileSet::new("sdk")
    .package(python::package("python")
        .name("email-sdk")
        .with(python::sdk()));
let tree = poolster::generate(&api, release)?;
tree.write_to("generated")?;
```

SDKs are namespaced by default. Choose `python::sdk().flat()` or
`.namespaced()` explicitly, or supply a shared `Common` default.
Namespaced clients expose methods such as `client.contacts.get(...)`;
flat clients use `client.get_contact(...)`. Operation arguments and model names
come from your API contract.

When depending on this plugin without the `poolster` facade, import
`poolster_plugin_python::PackageExt` and compose its package through
`poolster_core::engine::Packages`. Supply a security catalog when your API
declares named security schemes.

See [configuration](../../../docs/configuration.md) for every generation option
and the generated package's README for exact operation signatures.

Generated clients retry safe transient failures by default. `GET`, `PUT`,
`PATCH`, and `DELETE` are retryable; `POST` requires a declared and supplied
`Idempotency-Key`. Configure `max_retries`, `retry_initial_delay`, and
`retry_max_delay` on `Client`, or attach `before_request`, `after_response`,
and `on_error` callbacks for telemetry. Binary bodies and downloads remain
native Python `bytes`.

When an operation explicitly declares `x-poolster-pagination` (or legacy
`x-kaji-pagination`/`x-speakeasy-pagination`) with `type: cursor`, an existing cursor parameter,
and `outputs.nextCursor`, Poolster also emits a synchronous page iterator such as
`client.list_contacts_pages(cursor=None)`. In namespaced mode the same helper
is available as `client.contacts.list_pages(...)`. Paths support object fields
and array indexes, including `[-1]`; undeclared or unresolvable pagers are not
generated.

`type: url` declarations produce the same page iterator. Poolster follows the
declared link only through the original generated operation, retaining its
method, headers, authentication and body encoding. A continuation whose scheme
or authority differs from the initial API request is rejected rather than
forwarding credentials to another origin.

Enable native asynchronous generation with `python::sdk().async_client(true)`.
Install the generated package's `async` extra to use its optional `httpx`
transport. `AsyncClient` shares models and wire codecs with `Client`, exposes
awaitable flat and namespaced operations, and emits async page iterators.
Streaming operations return async iterators; close iterators when stopping
iteration early. Cancellation releases the streaming response. Use
`async with AsyncClient(...)` or `await client.aclose()` for transport cleanup.
Injected `http_client` instances stay caller-owned.

`Client(..., token_provider=OAuthClientCredentials(token_url, client_id,
client_secret, scopes=["read"]))` manages cached OAuth2 client credentials.
Concurrent refreshes share one request. A rejected token causes one refresh
and replay, then a second 401 is surfaced normally. Explicit authorization
headers and static bearer tokens take precedence. Async clients require an
async provider such as `AsyncOAuthClientCredentials`; close an async OAuth
provider separately with `await provider.aclose()` when finished. Custom
providers accept `None` for normal token retrieval or the rejected token for
refresh. OAuth providers use HTTP Basic client authentication.

Open model additional properties (including unspecified OpenAPI defaults)
are retained by `from_dict` and emitted at their original JSON keys. Explicit
`additionalProperties: false` continues to use closed models. The synthetic
map uses dataclass metadata so it cannot be mistaken for an ordinary API
property named `additional_properties`.

Add `.with(python::roundtrips())` to generate bounded JSON fixtures and an
executable `tests/test_model_roundtrips.py` consumer. It resolves the SDK's
`PythonModels` contract automatically; `.models_from(&sdk)` selects an explicit
provider. Run `PYTHONPATH=src python tests/test_model_roundtrips.py`. The fixture
file includes diagnostics for schemas that cannot be sampled within bounds.
The consumer currently covers generated object models, including nested wire
values, enums, unions, nulls and additional properties within those models.

`.with(python::webhooks())` independently adds `webhooks.py`. Its
`verify_webhook(raw_body, headers, secrets)` accepts trusted `whsec_` HMAC keys,
checks a bounded timestamp window and verifies the original bytes before JSON
decoding. `verify_and_decode(..., model=YourModel)` then applies a generated
model's decoder. Store verified webhook IDs in your application's idempotency
store. This plugin implements HMAC v1; asymmetric v1a requires another verifier.
The wire format follows the [Standard Webhooks specification](https://github.com/standard-webhooks/standard-webhooks/blob/main/spec/standard-webhooks.md).

### Customer transport middleware

Configure `Client(..., middleware=(first, second))` with callables accepting `(request, next)` and returning a native `urllib` response. The first middleware is outermost. Each retry/auth replay runs the chain again after authentication and body encoding. Middleware may change the `urllib.request.Request`, wrap or replace the response, catch/rewrite transport errors, or return a response without calling `next`. Synthetic error responses follow urllib conventions: raise `HTTPError` to enter generated API-error handling. Existing observation hooks continue to work.

```python
def add_header(request, next):
    request.add_header("X-Customer", "acme")
    return next(request)

client = Client("https://api.example.com", middleware=(add_header,))
```

For `AsyncClient`, use `async_middleware=(...)` with async `(request, next)` callables and `await next(request)`. Requests/responses are native httpx objects. Synchronous middleware is rejected by the async client. Cancellation propagates; middleware should not swallow `CancelledError`. The chain also wraps SSE connection establishment, without buffering the stream. Normal SDK response cleanup closes the returned response; middleware replacing a response must close any discarded response itself. Shared middleware state must support concurrent calls, and middleware can see credentials, so log deliberately.

### Bundle author middleware during generation

Use the package builder's `.middleware(BundledMiddleware { path, contents, symbol, async_symbol })` to ship an author-supplied Python source module and register its wrappers automatically. The path is package-relative, for example `src/example_api_sdk/customer.py`, beside `runtime.py`. `symbol` names the synchronous `(request, next)` callable. Async output requires an explicit `async_symbol` naming an async callable in the same module; sync-only output rejects it. Consumers instantiate `Client(base_url)` normally. Bundled wrappers run before optional customer constructor wrappers, in configuration order. Avoid importing the SDK runtime back into the middleware module at import time. Invalid native paths, missing async registration or generated-file collisions fail generation.
