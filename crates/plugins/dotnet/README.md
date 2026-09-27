# Kaji .NET plugin

`kaji-plugin-dotnet` renders SDK packages from Kaji's neutral API model,
using a .NET 8 HttpClient client. All generation runs in Rust.

```rust
use kaji::{dotnet, prelude::*};

let release = ProfileSet::new("sdk")
    .package(dotnet::package("dotnet")
        .name("email-sdk")
        .with(dotnet::sdk()));
let tree = kaji::generate(&api, release)?;
tree.write_to("generated")?;
```

SDKs are namespaced by default. Choose `dotnet::sdk().flat()` or
`.namespaced()` explicitly, or supply a shared `Common` default.
Namespaced packages expose resources such as
`client.Contacts.CreateContactAsync(...)`; flat packages keep methods directly
on the client. Groups follow the first OpenAPI tag or a meaningful path resource.

When depending on this plugin without the `kaji` facade, import
`kaji_plugin_dotnet::PackageExt` and compose its package through
`kaji_core::engine::Packages`. Supply a security catalog when your API
declares named security schemes.

See [configuration](../../../docs/configuration.md) for every generation option
and the generated package's README for exact operation signatures.
