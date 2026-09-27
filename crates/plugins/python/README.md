# Kaji Python plugin

`kaji-plugin-python` renders SDK packages from Kaji's neutral API model,
using a standard-library Python client. All generation runs in Rust.

```rust
use kaji::{python, prelude::*};

let release = ProfileSet::new("sdk")
    .package(python::package("python")
        .name("email-sdk")
        .with(python::sdk()));
let tree = kaji::generate(&api, release)?;
tree.write_to("generated")?;
```

SDKs are namespaced by default. Choose `python::sdk().flat()` or
`.namespaced()` explicitly, or supply a shared `Common` default.
Namespaced clients expose methods such as `client.contacts.get(...)`;
flat clients use `client.get_contact(...)`. Operation arguments and model names
come from your API contract.

When depending on this plugin without the `kaji` facade, import
`kaji_plugin_python::PackageExt` and compose its package through
`kaji_core::engine::Packages`. Supply a security catalog when your API
declares named security schemes.

See [configuration](../../../docs/configuration.md) for every generation option
and the generated package's README for exact operation signatures.

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
