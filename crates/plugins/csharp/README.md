# Kaji C#/.NET plugin

`kaji-plugin-csharp` renders C#/.NET SDK packages from Kaji's neutral API model,
using a .NET 8 HttpClient client. All generation runs in Rust.

```rust
use kaji::{csharp, prelude::*};

let release = ProfileSet::new("sdk")
    .package(csharp::package("csharp")
        .name("email-sdk")
        .with(csharp::sdk()));
let tree = kaji::generate(&api, release)?;
tree.write_to("generated")?;
```

SDKs are namespaced by default. Choose `csharp::sdk().flat()` or
`.namespaced()` explicitly, or supply a shared `Common` default.
Namespaced packages expose resources such as
`client.Contacts.CreateContactAsync(...)`; flat packages keep methods directly
on the client. Groups follow the first OpenAPI tag or a meaningful path resource.

`kaji::dotnet` remains a backwards-compatible facade alias. When depending on
this plugin without the `kaji` facade, import
`kaji_plugin_csharp::PackageExt` and compose its package through
`kaji_core::engine::Packages`. Supply a security catalog when your API
declares named security schemes.

See [configuration](../../../docs/configuration.md) for every generation option
and the generated package's README for exact operation signatures.

SDK authors can bundle customer middleware through `Package::middleware` or
`kaji.config.json` `middleware` entries with `source`, `path`, and `symbol`.
The source is copied and registered automatically; SDK callers do not configure
middleware. The symbol names a class in the generated namespace with
`public static HttpClient Wrap(HttpClient next)`. Use a compiled `.cs` path
outside `bin`/`obj`. Return a client backed by a customer handler that forwards
to the supplied client. Clone messages before forwarding into another HttpClient
because the outer client already marks its message as sent. Preserve cancellation,
content and streaming ownership; do not dispose a caller-owned client.
The first configured factory is outermost. SDK retries invoke the wrapped
transport for each attempt. Both CSharp and legacy DotNet targets support this
contract; `async_symbol` and alternate signatures are rejected. Files are owned
by the generator, collisions fail, and native compilation checks the signature.
