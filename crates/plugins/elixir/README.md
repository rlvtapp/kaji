# Poolster Elixir plugin

`poolster-plugin-elixir` renders SDK packages from Poolster's neutral API model,
using a Finch/Jason client. All generation runs in Rust.

```rust
use poolster::prelude::*;
use poolster_plugin_elixir::PackageExt as _;
use poolster_plugin_elixir as elixir;

let release = ProfileSet::new("sdk")
    .package(elixir::package("elixir")
        .name("email-sdk")
        .with(elixir::sdk()));
let tree = poolster::generate(&api, release)?;
tree.write_to("generated")?;
```

SDKs are namespaced by default. Choose `elixir::sdk().flat()` or
`.namespaced()` explicitly, or supply a shared `Common` default.
Namespaced packages add modules such as
`<Sdk>.Resources.Contacts.create_contact(client, body: contact)`; flat packages
keep `<Sdk>.API` operations. Both use a configured `<Sdk>.Client` and explicit
`{:ok, value}` / `{:error, reason}` results.

When depending on this plugin without the `poolster` facade, import
`poolster_plugin_elixir::PackageExt` and compose its package through
`poolster_core::engine::Packages`. Supply a security catalog when your API
declares named security schemes.

See [configuration](../../../docs/configuration.md) for every generation option
and the generated package's README for exact operation signatures.
