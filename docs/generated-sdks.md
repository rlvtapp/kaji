# Use a generated SDK

| Task | Start here |
| --- | --- |
| Generate the example packages | [Notes contract](#start-with-a-real-contract) |
| Use a client | [TypeScript](#typescript-install-call-and-inspect-http-results), [Python](#python-native-models-and-optional-async-io), [Go](#go-context-deadlines-and-native-http-configuration) |
| Find another language's client | [Entry points](#find-the-entry-point-in-every-language) |
| Iterate or stream | [Pagination and streaming](#pagination-and-streaming-depend-on-the-contract) |
| Check supported behavior | [Runtime coverage](#know-the-runtime-coverage-you-distribute) |

A generated SDK is a normal package in its language ecosystem: install it, import its
public client, configure your API origin and call an operation from your contract. Any
middleware bundled by the SDK author is already enabled. Application developers only
configure extra middleware when their application needs additional behavior.

If you are generating and distributing these packages, start with [SDK
customization](sdk-customization.md) and [SDK publishing](sdk-publishing.md). This guide
describes the resulting customer experience and can inform the documentation you
distribute with an SDK.

## Start with a real contract

The examples below use [the Notes contract](../examples/cli-basic/openapi.yaml), which
declares:

- `GET /notes/{noteId}`, operation ID `getNote`;
- a required string path parameter `noteId`;
- a JSON `Note` response containing `id` and `body`.

Generate flat clients so all three examples use the direct operation method:

```sh
kaji generate examples/cli-basic/openapi.yaml \
  --output generated --language typescript,python,go \
  --name Notes --client-style flat --typescript-client-name Notes
```

This produces the TypeScript package `@kaji/notes-fetch`, the Python import package
`notes_sdk`, and the Go module `notes`. The package names here are from this command,
not names you should assume for a different SDK. Inspect your generated manifest and
public entry point when adapting the examples.

Run a Notes API at `http://localhost:4010`, or use Kaji's contract mock in another
terminal:

```sh
kaji mock serve examples/cli-basic/openapi.yaml --port 4010
```

The Notes contract does not require credentials. The configuration sections below
explain how to add them for APIs that do.

## TypeScript: install, call and inspect HTTP results

### Build and install

Generated TypeScript packages use ESM. Kaji resolves generated relative imports to
emitted `.js` files, including directory entry points. The installed-package check
verifies native Node root/subpath imports and NodeNext consumer types; see
[verification](verification.md). Authored source overlays should also use `.js` relative
specifiers.

Build the package, then install it in your application:

```sh
cd generated/typescript
npm install
npm run build
# From your application directory:
npm install /absolute/path/to/generated/typescript
```

The package root exports `Notes`, the raw `getNote` function, models and `createClient`.
Here is a complete application module:

```ts
import { Notes } from '@kaji/notes-fetch'

const client = new Notes({ baseUrl: 'http://localhost:4010' })
const result = await client.getNote({
  path: { noteId: 'note_123' },
  throwOnError: false,
})

if (result.status === 200) {
  console.log(result.data.id, result.data.body)
}
console.log('HTTP status:', result.status)
```

### Choose decoded bodies or HTTP envelopes

Without `throwOnError: false`, a successful ordinary operation resolves to its decoded
body and a non-success response throws. The envelope form keeps HTTP status, content
type, headers and decoded data available. The TypeScript envelope union describes
declared statuses; an undeclared server response can fall outside that static contract.

Transport, decoding and validation failures can still reject the promise; handle those
with your application's usual `try/catch`.

### Use raw operations

The raw function uses the same request shape:

```ts
import { createClient, getNote } from '@kaji/notes-fetch'

const transport = createClient({ baseUrl: 'http://localhost:4010' })
const note = await getNote({ client: transport, path: { noteId: 'note_123' } })
console.log(note.body)
```

For a generated full client, `client.transport` exposes that configured transport to
framework helpers or raw functions. Raw-only generation omits the product client class;
generate it with `ts::sdk().fetch().raw()` or the CLI's `--typescript-surface raw`.

### Configure authentication and request controls

Configure `apiKey`, `apiKeyHeader`, `apiKeyPrefix`, `headers` or the runtime's
structured `auth` credentials as appropriate to the contract. Fetch accepts a custom
`fetch` implementation; Axios accepts its native instance through `client`. Fetch and
Axios accept per-call `requestOptions: { headers, timeoutMs, signal }` and a client
default `timeoutMs`.

See [request controls](guides/request-controls.md) for logical deadlines and native
cancellation. Injected drivers can also add their own timeout policy. For example:

```ts
import { Notes } from '@kaji/notes-fetch'

const client = new Notes({
  baseUrl: 'http://localhost:4010',
  fetch: async (input, init) => {
    const controller = new AbortController()
    const timer = setTimeout(() => controller.abort(), 5000)
    try {
      return await fetch(input, { ...init, signal: controller.signal })
    } finally {
      clearTimeout(timer)
    }
  },
  retry: { maxAttempts: 1 },
})
```

That timer bounds each Fetch attempt through response headers. It does not bound
subsequent stream consumption or the entire logical operation.

## Python: native models and optional async I/O

Install the generated package into your application's environment:

```sh
python -m pip install ./generated/python
```

The package root exports `Client`, `ApiError`, generated models and OAuth helpers:

```python
from notes_sdk import ApiError, Client

client = Client("http://localhost:4010", timeout=5.0, max_retries=0)
try:
    note = client.get_note(note_id="note_123")
    print(note.id, note.body)
except ApiError as error:
    print("HTTP response:", error.status_code, error.body)
```

A declared HTTP error may use a more specific generated subclass. Transport errors
retain their native exception behavior. JSON success objects become generated
dataclasses; use `Model.from_dict(...)` to decode a dictionary and `to_wire(model)` from
the package's `runtime` module to recover the wire representation.

Unknown properties and missing-versus-null handling follow the generated schema, rather
than treating every object as closed.

Use `bearer_token=...`, `api_key=...` or `headers={...}` for static credentials.
`token_provider` receives `None` on initial lookup and a rejected token when the SDK
requests refresh. `OAuthClientCredentials` provides coordinated caching for
client-credentials authentication; a managed 401 refresh is bounded to one replay.

Explicit Authorization headers and a static bearer token take precedence over a
provider.

Async output is an author choice: enable Python's SDK plugin `async_client` option, then
install the generated optional dependency:

```sh
python -m pip install './generated/python[async]'
```

The async package exports `AsyncClient` and uses native httpx I/O with the same models:

```python
import asyncio
from notes_sdk import AsyncClient

async def main():
    async with AsyncClient("http://localhost:4010", timeout=5.0) as client:
        note = await client.get_note(note_id="note_123")
        print(note.body)

asyncio.run(main())
```

Use an async token provider with `AsyncClient`. If you inject `http_client`, you own
that client's lifetime; an SDK-created httpx client is closed by `async with`.
Cancellation propagates through the async transport and closes SDK-owned responses.
Synchronous Python uses urllib, and its timeout is a transport timeout rather than an
operation-wide deadline.

## Go: context deadlines and native HTTP configuration

For the local generated module, add a replacement from your application module:

```sh
go mod edit -require=notes@v0.0.0
go mod edit -replace=notes=/absolute/path/to/generated/go
```

For a published SDK, use the actual `module` path from its `go.mod` and install its
release with `go get MODULE@VERSION` instead. This local example is a complete
`main.go`:

```go
package main

import (
    "context"
    "fmt"
    "log"
    "net/http"
    "time"

    sdk "notes"
)

func main() {
    client, err := sdk.NewClient(sdk.ClientConfig{
        BaseURL: "http://localhost:4010",
        HTTPClient: &http.Client{Timeout: 5 * time.Second},
        Retry: &sdk.RetryConfig{MaxAttempts: 1},
    })
    if err != nil { log.Fatal(err) }

    ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
    defer cancel()
    note, err := client.GetNote(ctx, &sdk.GetNoteRequest{NoteID: "note_123"})
    if err != nil { log.Fatal(err) }
    fmt.Println(note.ID, note.Body)
}
```

After saving `main.go`, run `go mod tidy` and `go run .`.

The context bounds the logical call, including retry waits; `http.Client.Timeout` bounds
a native request. Configure static credentials with `APIKey`, `APIKeyHeader` and
`APIKeyPrefix`—set `APIKeyPrefix: "Bearer"` when your service expects a bearer prefix.

`HTTPClient` accepts the generated `KajiHTTPClient` interface, so a native client or
decorator can replace execution without changing operation signatures.

Declared non-success responses become operation-specific errors; inspect the generated
operation error types with `errors.As`. Encoding/transport/decoding errors are returned
through Go's normal `error` result. The generated client consumes and closes buffered
response bodies.

## Find the entry point in every language

The generated README, native manifest and source declarations are the authority for your
package's exact name. Common entry points are:

| Target | Installation/build | Public customer entry point |
| --- | --- | --- |
| TypeScript | `npm install`, `npm run build` | Package root: configured client class, raw operations, models, `createClient` |
| Python | `python -m pip install PATH`; optional `[async]` extra | Native import package: `Client`, optional `AsyncClient`, models, `ApiError` |
| Go | `go get MODULE@VERSION`, or local module replacement | Package `NewClient(ClientConfig)`, context-taking methods, request structs |
| Rust | Cargo dependency with version or `path` | Crate `Client`, generated operation/model modules; `with_transport` for execution |
| PHP | Composer path/released package dependency | Generated namespace `Client`, PSR-18 client passed to its constructor |
| Java | Generated Maven or Gradle package | Generated package `Client`, `ClientConfig`, model package |
| C# | Project/package reference | Generated namespace `KajiClient(HttpClient, KajiClientOptions)` |
| Ruby | `gem build`, local/released gem installation | Generated require name and module `Client.new(...)` |
| Swift | Swift Package Manager dependency | `KajiClient(options: KajiClientOptions(...))` |
| Elixir | Local/released Mix dependency | Generated namespace `Client.new(...)` and API/resource modules |

Flat and namespaced clients use the same underlying operation implementation. Namespaces
derive from tags or meaningful path segments. Direct methods remain available. Parameter
spelling is native: the Notes fixture uses TypeScript `noteId`, Python `note_id` and Go
`NoteID`; do not copy method names from an unrelated contract.

## Pagination and streaming depend on the contract

A list response alone does not create a pager. Kaji needs declared pagination inputs and
response selectors, through `x-kaji-pagination` or compatible `x-speakeasy-pagination`
metadata. Pagination helpers keep using the generated operation's authentication,
serialization and error handling.

URL continuation helpers validate the origin before carrying credentials forward.

See the [pagination guide](guides/pagination.md) for declarations, native helper forms,
tested behavior and target limits.

For supported Python contracts, direct helpers are named `<operation>_pages`; namespaced
resources also expose their page helper. Synchronous helpers yield pages; async helpers
use `async for`. TypeScript, Go and Rust expose their own generated page helper
types/functions.

Inspect the generated operation reference to distinguish a page from an item: a pager
does not universally flatten arrays into individual models.

An operation declaring `text/event-stream` can generate an event-stream surface in
supported targets. Treat it as a resource whose consumption must finish or close.
Retries cover connection establishment, not application reconnection after events
arrive. Resume tokens, deduplication and `Last-Event-ID` policy belong to the
API/application.

The Notes fixture has neither pagination nor SSE, so no such methods are invented in its
examples.

## Know the runtime coverage you distribute

| Target | Retry/paging/streaming summary | Extension boundary |
| --- | --- | --- |
| TypeScript Fetch/Axios | Conservative retries; declared pagination and SSE | Logical-call middleware, hooks, native driver injection, optional codecs/validation and opt-in `validateResponses` structural buffered-success checks |
| Python | Conservative retries; declared cursor/offset/URL pages and SSE; async is opt-in | Per-attempt sync/async middleware and native async driver; lifecycle callbacks; managed OAuth |
| Go | Conservative retries; declared cursor/offset/URL pages and SSE | Per-attempt `KajiMiddleware`, injectable `KajiHTTPClient`, lifecycle hooks |
| Rust | Conservative retries; declared pagination/SSE supported by generated operation surface | Per-attempt `MiddlewareTransport`, native `Transport`, lifecycle hooks |
| PHP / Java / C# | Native retry and contract-dependent paging/streaming surfaces | Native HTTP client/decorator injection; target-specific hooks where emitted |
| Elixir | Native retries; contract-dependent paging/SSE | Buffered request middleware; separate streaming driver; lifecycle callbacks |
| Swift | Buffered async calls; no generated automatic retry/SSE middleware surface | `KajiTransport`, `KajiMiddlewareTransport`, URLSession, lifecycle hooks |
| Ruby | Buffered calls; no generated automatic retry/SSE surface | Callable transport and middleware |

This table identifies usable boundaries rather than asserting identical capabilities.
Check generated source and package tests for the specific contract/media type you
distribute. Native transports classify status and decode errors at different points; a
middleware chain is not automatically a schema validator.

For runtime retries, TypeScript/Rust/Go default to three total attempts. Python
expresses the corresponding setting as `max_retries=2`. Eligible methods/statuses still
depend on the generated operation and replay safety; POST and PATCH require an
idempotency key.

SDK authors can bundle automatic keys through [idempotency
configuration](guides/idempotency.md), with native caller overrides. Set TypeScript
`retry: false`, Go/Rust one total attempt, or Python `max_retries=0` when the
application owns retry policy. Do not assume Swift or Ruby shares that default.

Lifecycle hooks are notifications with native timing, not a portable replacement for
middleware. Python's `before_request` runs per attempt; TypeScript/Go/Rust logical-call
hooks have different timing. Use the [runtime middleware
guide](guides/runtime-middleware.md) to choose the correct extension point.

For TypeScript, `validateResponses: true` opts into checks of declared successful
buffered JSON shapes after middleware. It also checks synthetic/cache results and
custom-decoder outputs. It accepts extra fields and new enum strings, and does not
validate all OpenAPI constraints or SSE events.

See [response shape
checks](guides/runtime-middleware.md#opt-into-typescript-response-shape-checks) for the
scope. Author-bundled middleware is already active in an ordinary client; a customer's
`middleware` option adds application-specific behavior.

Go's `ClientConfig.ValidateResponses` similarly defaults to false. Its structural JSON
checks cover named generated models and containers of them after native middleware, with
a 10 MiB limit and one JSON value. Inline anonymous objects, composition constraints and
SSE are outside that scope.

See [Go response shape
checks](guides/runtime-middleware.md#opt-into-go-response-shape-checks) for the exact
boundaries.

## Keep improvements when regenerating

Put generator-author customization in the source-controlled recipe or plugin, including
bundled middleware sources and explicit code overlays. Regeneration can then reproduce
the distributed behavior. Ordinary SDK consumers keep application wrappers outside the
installed package.

Read [SDK customization](sdk-customization.md) for author-owned source registration, and
[SDK publishing](sdk-publishing.md) for packaging, exact-tag verification and release
automation.

## Rust future union values

Opt in during generation when a server may add union variants:

```json
{"language":"rust","path":"rust","plugins":[{"name":"sdk","open_unions":true}]}
```

The Rust package API exposes `.open_unions(true)` for both the SDK and independent model
provider. Named `oneOf`/`anyOf` enums gain a final `Unknown(serde_json::Value)` arm.
Values that match a known branch still decode normally; unmatched values retain their
complete JSON for serialization. The default remains strict and rejects values that
match no branch.

This option does not change discriminator selection or make every model extensible.
