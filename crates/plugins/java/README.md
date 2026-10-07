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

Use `java::sdk().open_enums(true)` to preserve future enum values returned by
an API. This opt-in emits immutable wire-value classes rather than Java enums.
Known constants, `value()` and `values()` remain available; value equality and
Jackson serialization preserve unknown strings. `fromKnownValue()` provides
strict validation for request construction, while `fromValue()` accepts future
response values. `isKnown()` distinguishes the two. Java enum switches and
`Enum` APIs require the default closed-enum policy.

Open object records retain unknown properties in a typed additional-properties
map using Jackson's any-getter/any-setter support. The map serializes at original
JSON keys, preserves null values, and cannot shadow a declared property. When
the schema omits additionalProperties or permits any value, the existing
constructor for declared fields remains available. Explicitly closed schemas
retain their existing generated record behavior. Record fields still do not
distinguish omitted optional properties from explicit null values.

SDK authors can bundle customer middleware through `Package::middleware` or
`kaji.config.json` `middleware` entries with `source`, `path`, and `symbol`.
The source is copied into the generated package and registered automatically;
SDK callers do not configure middleware. Java's symbol names a class in the
client package with `public static HttpClient wrap(HttpClient next)`. Put it at
`src/main/java/<generated/package>/<Symbol>.java`. Return a delegating HttpClient
that preserves its abstract methods, body handler types and interruption.
The first configured factory is outermost, and SDK retries call the resulting
transport for each attempt. `async_symbol` and alternate signatures are rejected.
Files are generator-owned, collisions fail, and regeneration refreshes them from
the author source. Native compilation checks the factory's signature.
