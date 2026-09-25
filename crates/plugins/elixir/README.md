# Kaji Elixir plugin

`kaji-plugin-elixir` renders a native Elixir/Mix package from Kaji's neutral
`kaji_core::Api` model. It emits component structs, a Finch-based
transport, typed operation functions, and explicit `{:ok, value}` / `{:error,
reason}` responses without using a JavaScript generator runtime.

```rust
let tree = kaji_plugin_elixir::generate_elixir_sdk(&api, "sdks/elixir", Some("email-sdk"))?;
tree.write_to("generated")?;
```

That compatibility entry point keeps the flat `<Sdk>.API.operation/2` layout.
Select the exported resource façade when desired:

```rust
use kaji_plugin_elixir::generate_elixir_sdk_with_style;
use kaji_core::SdkClientStyle;

let tree = generate_elixir_sdk_with_style(
    &api,
    "sdks/elixir",
    Some("email-sdk"),
    SdkClientStyle::Namespaced,
)?;
```

Namespaced packages add modules such as
`<Sdk>.Resources.Contacts.create_contact(client, body: contact)`. They group by
the first OpenAPI tag or, without tags, a stable path resource. The generated
`README.md` and `STYLE_GUIDE.md` document both layouts. The package depends on
Finch and Jason and uses a normal configured `<Sdk>.Client` in either style.
