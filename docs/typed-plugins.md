# Typed plugin packages (experimental)

This branch introduces language-scoped packages and a generic plugin engine.
Core owns no Rust or TypeScript renderers. All eight maintained languages live
under `crates/plugins/`; community languages need no change to a central enum.

```rust
use kaji::{prelude::*, rust, ts};

let release = ProfileSet::new("sdk")
    .common(Common::default().client_style(SdkClientStyle::Namespaced))
    .package(ts::package("typescript-fetch")
        .name("@acme/sdk")
        .with(ts::sdk().fetch().client_name("Acme")))
    .package(ts::package("typescript-axios")
        .with(ts::sdk().axios().raw()))
    .package(rust::package("rust").with(rust::sdk()));
// kaji::generate(&api, release)?.write_to(output_directory)?;
```

Options belong to the plugin instance. Package identity belongs to the language's
settings, exposed through its `PackageExt` trait. The prelude imports these
extension traits. `Common` carries optional shared defaults: client name, client
style, and package version. Package defaults override release defaults; explicit
plugin options override inherited values. A language consumes only relevant
settings (for example, client name currently affects TypeScript).

## Plugin authors

- Implement `Language` with `Settings`, a mutable `Workspace`, and optionally
  `finalize`. Language workspaces own shared symbols, dependencies, and metadata.
- Implement `Plugin<L>` with a fresh `Meta`, `kind`, and `generate`. A plugin for
  one language cannot be inserted into another language's package.
- Declare outputs using `Provision::of::<C>()`, and inputs using
  `Requirement::on::<C>(handle)`, where `C: Contract` is a typed Rust value.
- Publish declared outputs with `cx.publish(value)` and read declared inputs
  through `cx.inputs.get::<C>()`. Optional requirements use `.optional()` and
  `cx.inputs.optional::<C>()`.
- Emit package-relative files through `cx.files`. Duplicate file owners and
  escaping paths fail. `emit_custom` preserves existing user files on disk.

The resolver validates every package before generation starts. It automatically
binds a requirement when exactly one provider exists, and orders providers before
consumers. Missing providers, ambiguous providers, cycles, duplicate instance
identities, and invalid handles are errors. An explicit `Handle<C>` chooses one
provider when several exist. Bindings are package-local, never cross-package.
Optional requirements can be absent, but ambiguity or a dangling explicit handle
is still an error. Plugins must publish exactly their declared contracts.

`ts::types()` is the first independently composable renderer. It publishes
`ts::TsTypes`, a map of schema names to actual generated `Symbol`s. Its `.handle()`
can bind a community consumer explicitly; `.output("generated/models")` changes
the model module. `Symbol::import_from` computes relative imports. Consumers can
use `cx.workspace.dependency(name, range)` to contribute npm dependencies to the
final manifest. Conflicting ranges fail; semver intersection is not implemented.

The engine tests in `crates/kaji-core/tests/typed_packages.rs` cover provider
resolution, explicit handles, optional requirements, cycles, shared workspace
state, option inheritance, file ownership, and custom-file preservation.

## Migration boundaries

The maintained `sdk()` plugins currently wrap complete existing SDK generators
to preserve output. Fetch and Axios are options on that wrapper, not independent
transport providers yet. Use separate packages for the two variants. Do not
combine `ts::types()` with `ts::sdk()` in the same package: both currently own
model and package files.

Separating models, operation clients, and transport contracts (then consumers
such as TanStack) is a follow-up. No placeholder operation contracts are exposed.
Only TypeScript currently has a symbol/dependency workspace; other language
wrappers retain their existing rendering and use unit workspaces for now.

The old `ProfileSet` shortcuts remain as compatibility adapters into the new
engine. Legacy low-level SDK profiles moved from `kaji_core` to
`kaji::legacy_sdk`; TypeScript options now live in its plugin crate and are
re-exported by the facade. This is a branch experiment, not a stable plugin ABI
or a Node compatibility layer.
