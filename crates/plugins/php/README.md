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
