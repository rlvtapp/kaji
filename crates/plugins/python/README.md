# Kaji Python plugin

`kaji-plugin-python` renders an installable, typed standard-library Python SDK
from Kaji's neutral `kaji_core::Api` model.

```rust
let tree = kaji_plugin_python::generate_python_sdk(&api, "sdks/python", Some("email-sdk"))?;
tree.write_to("generated")?;
```

The default `SdkClientStyle::Flat` produces direct methods such as
`client.get_contact(...)`. To emit the product-style resource facade, use:

```rust
use kaji_core::SdkClientStyle;

let tree = kaji_plugin_python::generate_python_sdk_with_style(
    &api,
    "sdks/python",
    Some("email-sdk"),
    SdkClientStyle::Namespaced,
)?;
```

The namespaced client exposes typed operation groups such as
`client.contacts.get(...)` while retaining the flat methods for migration.

Generated clients retry safe transient failures by default. `GET`, `PUT`,
`PATCH`, and `DELETE` are retryable; `POST` requires a declared and supplied
`Idempotency-Key`. Configure `max_retries`, `retry_initial_delay`, and
`retry_max_delay` on `Client`, or attach `before_request`, `after_response`,
and `on_error` callbacks for telemetry. Binary bodies and downloads remain
native Python `bytes`.

When an operation explicitly declares `x-kaji-pagination` (or compatible
`x-speakeasy-pagination`) with `type: cursor`, an existing cursor parameter,
and `outputs.nextCursor`, Kaji also emits a synchronous page iterator such as
`client.list_contacts_pages(cursor=None)`. In namespaced mode the same helper
is available as `client.contacts.list_pages(...)`. Paths support object fields
and array indexes, including `[-1]`; undeclared or unresolvable pagers are not
generated.

`type: url` declarations produce the same page iterator. Kaji follows the
declared link only through the original generated operation, retaining its
method, headers, authentication and body encoding. A continuation whose scheme
or authority differs from the initial API request is rejected rather than
forwarding credentials to another origin.
