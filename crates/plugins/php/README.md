# Kaji PHP plugin

`kaji-plugin-php` renders SDK packages from Kaji's neutral API model,
using a PHP 8.2+ PSR-18/PSR-7 client. All generation runs in Rust.

```rust
use kaji::{php, prelude::*};

let release = ProfileSet::new("sdk")
    .package(php::package("php")
        .name("email-sdk")
        .with(php::sdk()));
let tree = kaji::generate(&api, release)?;
tree.write_to("generated")?;
```

SDKs are namespaced by default. Choose `php::sdk().flat()` or
`.namespaced()` explicitly, or supply a shared `Common` default.
Namespaced clients expose accessors such as `$client->contacts()->get(...)`;
flat clients use `$client->getContact(...)`. See [STYLE_GUIDE.md](STYLE_GUIDE.md).

When depending on this plugin without the `kaji` facade, import
`kaji_plugin_php::PackageExt` and compose its package through
`kaji_core::engine::Packages`. Supply a security catalog when your API
declares named security schemes.

See [configuration](../../../docs/configuration.md) for every generation option
and the generated package's README for exact operation signatures.

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

Object models retain allowed unknown keys in the additional-properties map,
exclude declared fields from that map, and serialize extras at original wire
keys. Decoded models distinguish omitted optional fields from explicit nulls.
Empty object models serialize as `{}`. The synthetic extra map is renamed if
an API property would collide with its generated name. Unknown JSON object/array
identity is still limited by associative-array decoding in the PHP runtime.

SDK authors can bundle customer middleware through `Package::middleware` or
`kaji.config.json` `middleware` entries with `source`, `path`, and `symbol`.
Use a `.php` path under `src` and a class in the generated namespace with
`public static function wrap(\Psr\Http\Client\ClientInterface $next): \Psr\Http\Client\ClientInterface`.
The source is required and the returned PSR-18 decorator is enabled automatically
in the generated constructor; SDK callers do not configure middleware. The first
configured factory is outermost. Preserve PSR-18 exceptions and PSR-7 stream
positions; SDK retries call the decorated transport for each attempt.
`async_symbol` and alternate signatures are rejected. Sources are generator-owned,
collisions fail and regeneration updates them. Configure middleware on the base
PHP SDK package when using a Symfony integration package.
