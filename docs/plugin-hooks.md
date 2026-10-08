# Plugin hooks at a glance

Choose the layer you want to extend. Follow the linked guide for a working example.

[Plugin walkthrough](typed-plugins.md) · [Input providers](input-plugins.md) · [Architecture](architecture.md)

## Read source contracts

| API | What it does | Details |
| --- | --- | --- |
| `InputPlugin::{id, format, load}` | Identify a parser and load a file into typed contracts | [Register a provider](input-plugins.md#select-or-replace-a-provider) |
| `InputRegistry::{register, plugins, load}` | Register, list and select providers; ambiguous formats need explicit selection | [Provider selection](input-plugins.md#select-or-replace-a-provider) |
| `InputContract::{publish, get, take}` | Store, borrow or remove a native capability; duplicate publication fails | [Input graph](input-plugins.md#feed-an-input-into-the-output-graph) |
| `InputProvider<C>::new(...).using(...).handle()` | Publish one selected input capability into a package graph | [Input bridge](input-plugins.md#feed-an-input-into-the-output-graph) |
| `Adapter::adapt() -> Result<AdaptedApi>` | Normalize a source directly to the existing HTTP API and security catalog | [Adapter source](../crates/kaji-core/src/adapter/mod.rs) |
| `kaji::generate_with_adapter` / `kaji::generate_with_input` | Generate HTTP packages from an adapter or published `AdaptedApi` | [HTTP capability](input-plugins.md#supply-a-normalized-http-input-later) |

`InputContract::summary` and `diagnostics` support inspection. Native GraphQL,
event, workflow and RPC data needs consumers for its own contract types.
`InputProvider<C>` publishes the selected `C`; it does not automatically forward
all diagnostics or every capability stored by the parser.

## Implement an output plugin

| `Plugin<L>` method | Requirement/default | Purpose |
| --- | --- | --- |
| `kind()` | Required | Name used in diagnostics |
| `meta()` | Required; keep one `Meta` per instance | Identity, optional label and typed handles |
| `requires()` | Defaults to no requirements | Declare contracts the plugin reads |
| `provides()` | Defaults to no provisions | Declare contracts the plugin publishes |
| `generate(cx)` | Required | Read inputs, update the workspace, publish contracts and emit files |
| `enforce()` | Defaults to `Enforce::Default` | Use `Enforce::Post` for a Post plugin |
| `phase()` | Defaults to `self.enforce().phase()` | Override when phase selection needs custom logic |

Implementing `phase()` overrides the phase shorthand. Post uses the same
`generate` method; there is no separate `post()` callback.

[Minimal executable plugin](../examples/custom-plugin/README.md) ·
[Consumer example](typed-plugins.md#implement-a-reusable-consumer)

### Bind contracts

Implement `Contract` with a diagnostic `NAME`. Rust types identify capabilities.

- `Provision::of::<C>()` declares an output; `cx.publish(value)` publishes it once.
- `Requirement::on::<C>(None)` selects the unique compatible provider.
- `Requirement::on(Some(handle))` selects an instance explicitly.
- `.optional()` allows a missing automatic provider. Ambiguous providers,
  dangling handles and dependency cycles still fail.
- `cx.inputs.get::<C>()` requires a value; `optional::<C>()` returns an option.
  Both reject undeclared reads.

Handles stay within a package. Generate plugins cannot depend on Post providers.
Every declared provision must actually be published.
[Binding rules →](typed-plugins.md#contracts-describe-actual-outputs)

### Use the generation context

| `PluginContext` field | Available data |
| --- | --- |
| `api` | Borrowed normalized HTTP `Api` |
| `semantics` | Shared SDK semantics derived for this package |
| `security_schemes` | Optional HTTP security catalog |
| `common` / `settings` | Resolved shared defaults and language settings |
| `inputs` | Declared, bound typed contracts |
| `workspace` | Mutable language-owned state for shared metadata |
| `files` | Package-relative, ownership-aware emitter |

Read native source data through `inputs`; `api` does not automatically become
the document published by an input provider. Update dependencies and exports
before language finalization.
[Provider composition →](typed-plugins.md#compose-typescript-providers-independently)

## Add or finalize a language

A `Language` declares `NAME`, defaultable `Settings` and defaultable `Workspace`.
Its hooks run once per package:

| Hook | Runs when | Default |
| --- | --- | --- |
| `finalize(&mut FinalizeContext<Self>)` | After Generate plugins, before Post | No-op |
| `bundle_middleware(&mut GeneratedTree, &[BundledMiddleware])` | After Post, only when middleware is configured | Error: unsupported middleware |
| `finalize_files(&mut GeneratedTree)` | After middleware, before source overlays | No-op |

`FinalizeContext` provides `api`, `common`, `settings`, mutable `workspace` and
`files`. It does not provide graph `inputs`, SDK `semantics`, a security catalog
or contract publication. Post plugins still use `PluginContext` and can consume
contracts published earlier; finalization does not run again afterwards.

```text
Validate graph → Generate → finalize → Post → bundle middleware
               → finalize_files → source overlays → output ownership checks
```

[Language responsibilities](typed-plugins.md#what-core-owns-and-what-the-language-owns) ·
[Middleware hook](typed-plugins.md#add-a-languages-bundled-middleware-hook)

## Emit or adapt files

| API | Use it for |
| --- | --- |
| `cx.files.emit(file)` | An owned file that updates on regeneration |
| `cx.files.emit_custom(file)` | A create-once starter file that preserves customer edits |
| `cx.files.append(tree)` | Merge rendered files while retaining ownership and create-once behavior |
| `cx.files.append_from(tree, prefix)` | Remove a renderer's output prefix; files outside it fail |
| `GeneratedTree::replace(file)` | Deliberately change a file already in the staged tree; missing files fail |
| `tree.check(root)` / `tree.write_to(root)` | Inspect drift or materialize the owned output |

Paths are relative to the package. Escaping paths and duplicate emitters fail.
A finalizer editing `GeneratedTree` must preserve ownership and use explicit
replacement for existing files.
[File safety →](safe-regeneration.md)

## Configure packages and shipping

| Extension | Guide |
| --- | --- |
| Package settings and shared defaults | [Rust configuration](configuration.md) |
| Added, replaced or patched source | [SDK customization](sdk-customization.md) |
| Native HTTP middleware and transport injection | [Runtime middleware](guides/runtime-middleware.md) |
| Release metadata as a Post plugin | [SDK automation](sdk-automation.md) |
| Custom publisher command vectors | [Publishing](sdk-publishing.md) |

Plugins are linked Rust crates. The shipped CLI has a compiled registry;
there is no dynamic plugin installer or generic mutable-API callback.
Use input normalization for source adaptation and typed consumers for native formats.

## Check an extension

Test binding errors, publication failures, file collisions and package isolation.
Compile and exercise the emitted consumer when you change a language or transport ABI.
[Authoring checks →](typed-plugins.md#validate-the-extension-before-releasing-it)

The public definitions are in [engine.rs](../crates/kaji-core/src/engine.rs),
[input.rs](../crates/kaji-core/src/input.rs) and [files.rs](../crates/kaji-core/src/files.rs).
