# Kaji .NET plugin

`kaji-plugin-dotnet` renders a native .NET 8 package from Kaji's neutral
`kaji_core::Api` model. The original entry point keeps its compact,
flat client surface:

```rust
let tree = kaji_plugin_dotnet::generate_dotnet_sdk(
    &api,
    "sdks/dotnet",
    Some("email-sdk"),
)?;
```

Use `generate_dotnet_sdk_with_style` to select an exported resource façade:

```rust
use kaji_plugin_dotnet::generate_dotnet_sdk_with_style;
use kaji_core::SdkClientStyle;

let tree = generate_dotnet_sdk_with_style(
    &api,
    "sdks/dotnet",
    Some("email-sdk"),
    SdkClientStyle::Namespaced,
)?;
```

Flat packages use `client.CreateContactAsync(...)`. Namespaced packages add
`client.Contacts.CreateContactAsync(...)`, grouping by the first OpenAPI tag
or, when tags are absent, a stable path resource. The generated `README.md`
and `STYLE_GUIDE.md` document both layouts.
