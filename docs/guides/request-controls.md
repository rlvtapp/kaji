# Per-call HTTP controls

Generated SDK customers can override headers and request timeouts directly. These are
runtime options: SDK authors do not need to supply generation middleware to enable them.
Middleware remains available for reusable rewriting, instrumentation and other behavior.

## TypeScript Fetch and Axios

Supply `requestOptions` alongside the API's typed inputs:

```ts
await sdk.contacts.get({
  path: { contact_id: 'contact_123' },
  requestOptions: {
    headers: { 'X-Trace-ID': 'trace_123' },
    timeoutMs: 5_000,
    signal: controller.signal,
  },
})
```

Use your generated operation's actual path/input names. `requestOptions.headers` accepts
`HeadersInit` and overrides client defaults and API-declared headers. Header input is
copied before middleware runs. API-declared headers remain separately typed in
`headers`.

Set `timeoutMs` on `ClientConfig` for a default. Per-call values override it. The
timeout bounds the logical request, including middleware, native driver execution,
response decoding and retry backoff. Cancellation reaches native Fetch/Axios drivers and
stops subsequent attempts.

OAuth clients also apply the same controls to token waiting and the bounded
authentication replay.

Timeouts must be finite positive milliseconds. `PoolsterRequestTimeoutError` has `name:
'TimeoutError'`; caller cancellation preserves the signal's reason. Streaming timeouts
cover establishing the response.

Cancel a stream with a caller signal to stop further reading; modern platforms with
`AbortSignal.any` preserve that link when timeout and cancellation are combined.

## Ruby

Supply optional `request_options:` to direct operations, resource facades or pagination
helpers:

```ruby
client.contacts.get(
  contact_id: 'contact_123',
  request_options: {
    headers: { 'X-Trace-ID' => 'trace_123' },
    timeout: 5,
    cancelled: -> { cancellation_requested? }
  }
)
```

Timeouts are finite positive seconds. The client `timeout:` is the default; per-call
values override it. The logical timeout covers token waiting, middleware, native HTTP
execution and retry backoff. Native Net::HTTP also receives the selected open/read
timeout. `PoolsterTimeoutError` derives from Ruby's `Timeout::Error`.

The cancellation callback supplements the client callback and is checked before
execution and during retries/token waits. Active synchronous cancellation remains
driver-owned; the logical timeout can interrupt a stalled call. Explicit `Authorization`
headers remain caller-owned when OAuth is configured.

Controls are local to each call, so concurrent requests can choose different timeouts
without mutating the client. Header/options hashes are retained unchanged. If an API
declares a parameter that normalizes to `request_options`, the generated control keyword
gains leading underscores to keep the API parameter accessible.

## Go

Carry controls in the operation's ordinary context:

```go
ctx, cancel, err := sdk.WithRequestOptions(context.Background(), sdk.PoolsterCallOptions{
    Headers: http.Header{"X-Trace-ID": []string{"trace_123"}},
    Timeout: 5 * time.Second,
})
if err != nil { return err }
defer cancel()
result, err := client.Contacts.Get(ctx, input)
```

Headers are copied and override generated headers and static credentials. A zero timeout
preserves the caller's deadline; negative timeouts fail. A positive timeout can shorten
an existing deadline. Token acquisition, backoff, body reading and pagination retain the
same context. If you reuse it across pages, the deadline covers the entire traversal.

Keep the context alive until a returned stream is closed. The client and native driver
remain shared and unchanged.

## Python, Rust, Java and C#

These targets expose independent call scopes that share the native driver, middleware,
hooks and authentication provider. Use a scope for one operation or retain it for
several operations with the same controls:

| Target | Native scope API | Timeout meaning |
| --- | --- | --- |
| Python | `client.for_call(headers={'X-Trace-ID': 'trace_123'}, timeout=5)` | Seconds; urllib/httpx transport timeout for each attempt |
| Rust | `client.for_call(CallOptions { headers, timeout: Some(Duration::from_secs(5)) })` | Native reqwest timeout for each attempt; custom transports must honor the request timeout |
| Java | `client.forCall(new ClientCallOptions(Map.of("X-Trace-ID", "trace_123"), Duration.ofSeconds(5)))` | JDK HTTP request timeout for each attempt |
| C# | `client.ForCall(new PoolsterCallOptions { Headers = headers, Timeout = TimeSpan.FromSeconds(5) })` | Linked cancellation deadline across a buffered call, retries and SSE; a new paginated call starts a new deadline |

The original client remains unchanged. In Python/Rust/Java, generated declared header
arguments can override scoped defaults. C# scoped headers override generated headers.
Explicit Authorization remains caller-owned. Use the native cancellation mechanism for
your language alongside timeouts.

Python async scopes share the original httpx driver. Keep its owner alive and close that
owner normally; the scoped client does not take ownership of an injected driver. Java
stream consumption and custom drivers retain their native timeout and cancellation
responsibilities.

Per-attempt timeouts in Python/Rust/Java do not impose a deadline on token acquisition
or the complete retry loop.

## Swift, PHP and Elixir scopes

| Target | Native scope API | Timeout meaning |
| --- | --- | --- |
| Swift | `try client.forCall(headers: ["X-Trace-ID": "trace_123"], timeout: 5)` | Seconds; URLRequest timeout per attempt; custom transports retain native responsibility |
| PHP | `$client->forCall(headers: ['X-Trace-ID' => 'trace_123'], httpClient: $boundedDriver)` | PSR-18 has no standard per-request timeout; supply a driver configured with your timeout policy |
| Elixir | `Client.for_call(client, headers: [{"x-trace-id", "trace_123"}], timeout: 5_000)` | Milliseconds; Finch receive timeout per request attempt |

Each scope leaves the original client unchanged. PHP scopes share or replace the PSR
driver; this is deliberate native driver injection, not a portable numeric timeout
argument. Swift/Elixir scopes retain their native cancellation and streaming behavior.
Timeout values do not impose a deadline on the entire pagination traversal.
