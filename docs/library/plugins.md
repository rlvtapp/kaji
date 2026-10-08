# Native plugin composition

Packages choose a language, directory and identity. Plugins choose what to render.

```rust
use kaji::{prelude::*, ts};

let package = ts::package("typescript")
    .name("@acme/pet-store")
    .with(ts::sdk().fetch())
    .with(ts::composition::zod().output("validation"));
```

## Connect the hooks

| Step | API |
| --- | --- |
| Read a source and publish native contracts | `InputPlugin::load` and `InputContract::publish` |
| Make an input available to a package | `InputProvider<C>` and its `.handle()` |
| Declare a consumer dependency | `Requirement::on::<C>(Some(handle))` |
| Read the selected contract | `cx.inputs.get::<C>()` |
| Publish reusable output metadata | `Provision::of::<C>()` and `cx.publish(value)` |
| Emit an owned file | `cx.files.emit(GeneratedFile::new(...))` |

Core orders providers before consumers. Missing or ambiguous inputs, cycles,
duplicate identities and conflicting paths fail generation. Handles are
package-local. Select separate packages for Fetch and Axios variants.

See [input plugins](../input-plugins.md) for parsing hooks and
[typed plugins](../typed-plugins.md) for complete consumer examples.

## Finalize shared files

During Generate, plugins register shared dependency and export state. The language
finalizer assembles manifests and barrels. `Language::finalize_files` can adapt
assembled files after middleware bundling and before source overlays; its default
is a no-op. See [generation phases](../typed-plugins.md#generation-phases-and-file-ownership).
