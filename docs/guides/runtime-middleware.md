# Runtime middleware: shipped defaults and application wrappers

Use middleware when a request needs a policy around HTTP execution: add a tenant header, sign a native request, recover a transport failure, rewrite a response or return cached data. Use lifecycle hooks to observe calls. Use a custom driver when you need to replace the HTTP implementation itself.

For SDK authors, supply the middleware source during generation. Kaji bundles it as a separate readable SDK module and registers it by default. Customers instantiate the client normally; they do not import a policy or populate a middleware array to activate the behavior you distribute. The source, exported symbol and registration recipe are covered in [SDK customization](../sdk-customization.md).

Application-level registration is optional and adds application behavior around an installed SDK. The examples here show that extra surface. Bundled defaults are prepended to application wrappers: author defaults are outermost, and their requests reach application wrappers before transport execution.

## Add a header to a TypeScript application

This example uses the flat `Notes` client generated in [Use a generated SDK](../generated-sdks.md):

```ts
import { Notes, type ClientMiddleware } from '@kaji/notes-fetch'

const tenantHeader: ClientMiddleware = async (request, next) => {
  const headers = new Headers(request.headers as HeadersInit | undefined)
  headers.set('X-Tenant', 'acme')
  return next({ ...request, headers })
}

const client = new Notes({
  baseUrl: 'http://localhost:4010',
  middleware: [tenantHeader],
})
const note = await client.getNote({ path: { noteId: 'note_123' } })
console.log(note.body)
```

The wrapper receives operation-level request data before serialization and authentication. Fetch and Axios expose `ClientMiddleware(request, next)`; ordinary results use a `MiddlewareResponse` envelope containing `status`, `contentType`, `data` and `headers`. Transform that envelope to replace response data, or return an envelope immediately to skip the remaining chain. Preserve native Fetch `Response` objects for streams and Axios envelopes containing their readable stream.

TypeScript middleware runs once per logical call. Its `next` contains the transport's retry loop. Call `next(updatedRequest)` at most once: application replay policy belongs outside this continuation. A short circuit bypasses remaining middleware, driver execution, transport hooks and transport-level validation callbacks. The optional structural response check runs after the completed middleware chain, including short circuits. Generated operation/result adapters still consume the returned shape, so return a compatible response.

## Opt into TypeScript response shape checks

A customer can enable structural checking when constructing a Fetch or Axios
client. This option is independent of the middleware already bundled by the SDK
author:

```ts
const client = new Notes({
  baseUrl: 'http://localhost:4010',
  validateResponses: true,
  middleware: [tenantHeader],
})
```

The default is disabled. When enabled, declared successful buffered JSON response
bodies are checked after the outer middleware chain completes. Results rewritten
by a middleware layer, returned from its cache, or produced by a custom decoder
must satisfy the same declared structural shape. A validation failure is terminal;
it does not replay a successful HTTP call. Diagnostics identify the failing path
without including the response payload.

Checks cover required fields, primitive types, nullability, references, nested
arrays/objects and supported intersections/unions. Extra object fields and new
enum strings remain accepted for forward compatibility. Union checks cover the
supported generated alternatives; they do not promise full JSON Schema `oneOf`
exclusivity or every discriminator/constraint combination. This is not complete
OpenAPI validation: bounds, patterns, formats and other schema constraints are
outside this option. SSE events, error responses, HEAD/204 responses, undeclared/no-schema response
bodies and mismatched content types are outside the buffered successful-response
scope. A per-request `validateResponses: false` can disable a client’s default.

Existing `validation` callbacks/Standard Schema assertions remain separate and
retain their transport timing. Generating Zod schemas alone does not register
operation response validation. Lossless integer parsing handles numeric precision;
it is not proof that the response satisfies its schema.

Generated Fetch/Axios executable tests cover terminal malformed-wire failures
without an extra transport attempt, custom-codec outputs, cached/middleware
rewrites, request overrides, required/null/array/reference checks, supported
unions/intersections, status selection and open enum/extra-field compatibility.
These fake-driver tests exercise generated runtime behavior; they do not establish
live API compatibility or complete schema validation.

## Use Python's native request boundary

Synchronous wrappers accept urllib requests and return urllib-compatible responses:

```python
from notes_sdk import Client

def tenant_header(request, next):
    request.add_header("X-Tenant", "acme")
    return next(request)

client = Client("http://localhost:4010", middleware=(tenant_header,))
note = client.get_note(note_id="note_123")
print(note.body)
```

Python invokes this chain for each transport attempt after request encoding and authentication. A wrapper can catch and replace native transport exceptions, return a synthetic response without calling `next`, or wrap a response. urllib represents an HTTP failure by raising `HTTPError`; returning a synthetic response with an error status alone does not reproduce that exception boundary. Generated declared-error conversion runs after the native exception returns through middleware.

For generated async clients, use a distinct async callable and native httpx objects:

```python
import asyncio
from notes_sdk import AsyncClient

async def tenant_header(request, next):
    request.headers["X-Tenant"] = "acme"
    return await next(request)

async def main():
    async with AsyncClient(
        "http://localhost:4010", async_middleware=(tenant_header,)
    ) as client:
        note = await client.get_note(note_id="note_123")
        print(note.body)

asyncio.run(main())
```

AsyncClient rejects synchronous constructor middleware. Native cancellation propagates; a recovery wrapper should preserve `CancelledError`. An injected `http_client` remains owned by the application. The SDK closes returned responses, including on decode failure and cancellation; a wrapper replacing a response closes any discarded response itself. Middleware wraps SSE establishment without buffering event frames.

## Decorate a Go transport

Use the generated module import from the Notes example:

```go
import (
    "net/http"
    sdk "notes"
)

func tenantHeader(next sdk.KajiHTTPClient) sdk.KajiHTTPClient {
    return sdk.KajiHTTPClientFunc(func(request *http.Request) (*http.Response, error) {
        rewritten := request.Clone(request.Context())
        rewritten.Header.Set("X-Tenant", "acme")
        return next.Do(rewritten)
    })
}

// In your application initialization:
client, err := sdk.NewClient(sdk.ClientConfig{
    BaseURL: "http://localhost:4010",
    Middleware: []sdk.KajiMiddleware{tenantHeader},
})
```

`KajiMiddleware` wraps `KajiHTTPClient`, whose native ABI is `Do(*http.Request) (*http.Response, error)`. `KajiHTTPClientFunc` adapts a function to that interface. Wrappers run for each attempt and may rewrite requests/responses, recover errors or short circuit. Keep the request context: it carries cancellation/deadlines. The SDK consumes returned response bodies; wrappers close discarded bodies. Native status and decoding errors are classified after this boundary, so a wrapper receives the HTTP response rather than the eventual typed operation error.

## Opt into Go response shape checks

Set `ClientConfig.ValidateResponses` when creating the generated client:

```go
client, err := NewClient(ClientConfig{
    BaseURL: "http://localhost:4010",
    ValidateResponses: true,
})
if err != nil {
    return err
}
```

The default is false. Buffered JSON is checked after the native HTTP driver and
its middleware return, before the typed result reaches the caller. This covers
named generated model responses, including anonymous arrays/maps whose elements
are named models. Required fields, nonnullable nulls, scalar types and nested
objects/arrays/references are checked. Required write-only fields are exempt
from responses, including referenced write-only schemas. Declared typed additional
properties are checked; otherwise unknown fields and future enum strings are accepted. Validation reads at most 10 MiB, permits one JSON value and bounds
recursive validation at depth 128.

`ResponseValidationError` exposes `Path` and `Expected`, without response values.
Composition constraints, enum membership, formats and bounds are outside this
check. Anonymous inline response objects retain native decoder checks only;
SSE, binary and text responses bypass structural validation. A driver wrapper can
replace a response, but it cannot catch validation performed later by the
operation decoder. Anonymous scalars and containers also receive native shape checks.
Nullable pointer destinations accept null; nullable named object value structs
still cannot distinguish a root null from their zero value. Enabling validation
does not repair that representation limit or change method return types.
Preserve bodies and context as described above.

## Use the native extension in your language

These are customer extension interfaces, not a requirement to register SDK-author defaults:

| Target | Middleware or driver interface | What the wrapper receives |
| --- | --- | --- |
| TypeScript Fetch / Axios | `ClientConfig.middleware: ClientMiddleware[]`; `fetch` or Axios `client` replaces the driver | Operation request data and continuation; response envelope or native stream response |
| Python sync | `middleware=(...)` | urllib `Request`, synchronous continuation; urllib response/exception |
| Python async | `async_middleware=(...)`; `http_client` replaces the driver | httpx request, awaitable continuation; httpx response/exception |
| Go | `ClientConfig.Middleware []KajiMiddleware`; `HTTPClient KajiHTTPClient` | Native `*http.Request`, response and error |
| Rust | `Middleware::handle`; compose `MiddlewareTransport::new(policy, inner)`; client `with_transport(Arc<dyn Transport>)` | Owned reqwest request, `TransportFuture`, reqwest response/error |
| PHP | Inject `Psr\Http\Client\ClientInterface` decorator into generated client | PSR-7 messages and PSR-18 exceptions |
| Java | Supply `java.net.http.HttpClient` in `ClientConfig`; decorate that client | Native request, body handler, response, async futures/interruption |
| C# | Supply `HttpClient` built with a `DelegatingHandler` chain | Native request/response and CancellationToken |
| Ruby | `middleware: [callable]`; `transport: callable` replaces execution | Net::HTTP request and continuation; response exposing `code`/`body` |
| Swift | `KajiMiddlewareTransport(inner:middleware:)`; client `transport:` | URLRequest and async continuation returning `(Data, URLResponse)` |
| Elixir | `middleware: [fn request, next -> ... end]`; `transport:` replaces buffered execution | Finch.Request and `{:ok, Finch.Response}` / `{:error, reason}` |

PHP, Java and C# build on native decorator patterns rather than an identical SDK middleware-array API. Author bundling wraps the supplied native client automatically; language-specific author wrapper factories are documented in the customization guide. Native ABI compatibility matters: preserve Java body handlers/async methods, PSR exception interfaces and .NET cancellation when writing a driver decorator.

Ruby's default driver uses Net::HTTP and existing timeout settings. Its middleware receives fully assembled requests; a replacement native request can change its URI. There is no generated automatic retry or SSE surface in this runtime.

Swift middleware runs around buffered async execution. `KajiNext` is an async throwing `(URLRequest) -> (Data, URLResponse)` continuation; `KajiMiddleware` adds the initial request argument. Compose wrappers by nesting `KajiMiddlewareTransport`. Existing `session:` injection remains available. Swift status validation and decoding happen afterwards; middleware adds neither automatic retries nor streaming.

Elixir buffered middleware runs per attempt. Its custom buffered transport receives `(request, options)`. SSE has a separate `stream_transport` receiving `(request, options, accumulator, callback)` with Finch-compatible `{:status, code}`, `{:headers, headers}` and `{:data, binary}` events. Buffered request middleware does not transform SSE frames; decorate the streaming callback for that behavior.

## Understand where your policy runs

A logical call may perform multiple attempts. This distinction changes logging, caching and signing:

| Boundary | Timing and consequence |
| --- | --- |
| TypeScript middleware | Once around the logical transport call. Request edits precede serialization/authentication; retries are inside `next`. |
| Python / Go / Rust middleware | Each native transport attempt after request construction. Signing sees that attempt's native request. |
| PHP / Java / C# client decorator | Each native send performed by the SDK; status/decode classification remains in generated operation code. |
| Elixir buffered middleware | Each attempt around the encoded Finch request; SSE uses its independent driver. |
| Ruby / Swift middleware | Each native buffered execution; these runtimes do not add automatic retries. |
| Lifecycle hooks | Notifications at target-specific points. They do not universally receive every attempt or mutate the actual native request. |

HTTP status, response decoding and schema validation are different stages. Catching transport failures cannot necessarily recover a later generated-model decode failure. TypeScript response codecs/Standard Schema validation live in its runtime; native middleware in other languages does not silently validate against the complete OpenAPI schema.

For streams, retain the native body/iterator and its lifetime. A response wrapper that eagerly reads the stream changes streaming behavior. A short-circuit cache must return the shape the operation expects, including a live stream for a streaming operation. The caller closes or finishes consumption; middleware closes responses it discards. Preserve cancellation rather than turning it into a cache miss or retry.

The configured chain is copied or compiled into the client. Shared mutable wrapper state must support concurrent calls. Native wrappers can see resolved authentication headers; choose logged fields deliberately. If a policy should ship to every SDK customer, move it into the generator recipe rather than asking every application to register it.

Continue with [SDK customization](../sdk-customization.md) to bundle the policy, [generated SDK usage](../generated-sdks.md) to document the customer's entry point, and [SDK publishing](../sdk-publishing.md) to distribute the tested package.

## Python and Ruby structural response checks

Python SDK consumers can construct `Client(..., validate_responses=True)` or
`AsyncClient(..., validate_responses=True)`. Ruby uses
`Client.new(..., validate_responses: true)`. These checks run after middleware,
including responses returned by a cache or replacement policy, before model
conversion. The default remains permissive.

The generated checks cover declared primitive types, required response fields,
nullability, arrays, references and supported compositions. Future enum strings
and additional object properties remain accepted. Write-only fields are not
required in responses. They reject malformed JSON with a redacted
`ResponseDecodeError`; error messages identify a field path without echoing the
payload. They are structural checks, not complete JSON Schema validation of
formats, numeric bounds or every composition rule.

Python sync decoding bounds bytes read; async HTTPX decoding checks size after
reading the response. Both close responses and report terminal decoding failures
without treating them as retryable transport errors.
