# Kaji Java plugin

`kaji-plugin-java` renders SDK packages from Kaji's neutral API model,
using a Java 17+ HttpClient/Jackson client. All generation runs in Rust.

```rust
use kaji::{java, prelude::*};

let release = ProfileSet::new("sdk")
    .package(java::package("java")
        .name("email-sdk")
        .with(java::sdk()));
let tree = kaji::generate(&api, release)?;
tree.write_to("generated")?;
```

SDKs are namespaced by default. Choose `java::sdk().flat()` or
`.namespaced()` explicitly, or supply a shared `Common` default.
Namespaced clients expose resources such as `client.contacts().get(input)`;
flat clients use `client.getContact(input)`. Generated packages include Maven
and Gradle metadata. See [STYLE_GUIDE.md](STYLE_GUIDE.md).

When depending on this plugin without the `kaji` facade, import
`kaji_plugin_java::PackageExt` and compose its package through
`kaji_core::engine::Packages`. Supply a security catalog when your API
declares named security schemes.

See [configuration](../../../docs/configuration.md) for every generation option
and the generated package's README for exact operation signatures.
