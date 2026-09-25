# Kaji PHP plugin

Generates PHP 8.2+ SDK packages from Kaji's neutral API model. The default
`generate_php_sdk` API keeps the direct-operation client surface for backwards
compatibility. Use `generate_php_sdk_with_style` when selecting a public
surface explicitly.

See [STYLE_GUIDE.md](STYLE_GUIDE.md) for the generated SDK shapes.

The generated PSR-18 client safely retries transient transport failures and
HTTP `408`, `429`, and `5xx` responses. It retries idempotent methods by
default and only retries `POST` when an `Idempotency-Key` header is present.
Configure `maxRetries`, `retryInitialDelayMs`, and `retryMaxDelayMs` on the
client constructor. Optional `beforeRequest`, `afterResponse`, and `onError`
callbacks support instrumentation without changing generated operation calls.
Binary bodies and downloads use PHP strings.

An operation that explicitly declares `x-kaji-pagination` (or compatible
`x-speakeasy-pagination`) with `type: cursor`, an existing cursor parameter,
and `outputs.nextCursor` receives a lazy `\Generator`, for example
`$client->listPetsPages(cursor: null)`. Namespaced clients mirror it at
`$client->pets()->listPages(...)`. Kaji supports declared object fields and
array indexes (including `[-1]`) in the output path and skips any pager that
cannot be resolved against the operation declaration.

Declared `text/event-stream` operations return the untouched PSR-7
`StreamInterface`. Kaji does not buffer, parse, or reconnect it: PSR-18 does
not require every implementation to expose a live network stream. Applications
that need live SSE should select a stream-capable PSR-18 client and own event
decoding plus `Last-Event-ID`/reconnection policy.
