# Embed Poolster in Rust

Use the library to compose generators in your own tool or link custom Rust
plugins. Generation returns a virtual file tree; your application checks or
writes it.

[Hook reference: inputs, plugins, contexts, finalizers and files →](../plugin-hooks.md)

## Choose a starting point

| Task | Guide |
| --- | --- |
| Generate a first SDK | [Quickstart](quickstart.md) |
| Copy a runnable application | [Embedded example](../../examples/rust-embedded/README.md) |
| Combine maintained output plugins | [Plugin composition](plugins.md) |
| Parse another source format | [Input plugins](../input-plugins.md) |
| Author a consumer or language | [Typed plugins](../typed-plugins.md) |
| Find package and SDK options | [Configuration](../configuration.md) |

For recipes and releases through the bundled CLI, start with the
[CLI workflow](../cli/README.md).

<a id="prerequisites-and-first-result"></a>

## Requirements

Use a workspace-compatible Rust toolchain and local Cargo path dependencies from
a Poolster checkout. The API is pre-1.0; pin compatible revisions and review
[release notes](../releases/0.4.0.md) when updating.

The OpenAPI artifact compiler requires Go. Native input providers have their own
requirements; Cap’n Proto source compilation requires `capnp` on PATH. See
[input support](../input-plugins.md).

<a id="plugin-developers-learn-the-contracts-before-the-renderer"></a>

## The extension path

```text
InputPlugin -> InputContract -> InputProvider<C> -> Plugin<L> -> GeneratedTree
```

Input providers publish typed native documents. Output consumers declare required
contracts, read those values and emit package-relative files. Core resolves
bindings and execution order; each language owns its syntax and workspace.
See [architecture](../architecture.md) for layer ownership.

A plugin is compiled into your application. Naming a community crate in
`poolster.json` does not load it dynamically.

<a id="sdk-authors-compose-customize-and-verify"></a>
<a id="choose-checks-that-prove-the-intended-behavior"></a>

## Check and deliver

Use `tree.check(directory)` to inspect drift, then `tree.write_to(directory)` to
materialize it. [Safe regeneration](../safe-regeneration.md) explains file owners,
manual-edit conflicts and create-once starter files.

Compile generated packages and exercise their transports with native tools.
Generation does not install dependencies or publish packages. Follow
[SDK customization](../sdk-customization.md), [verification](../verification.md),
[release automation](../sdk-automation.md) and [publishing](../sdk-publishing.md)
for those steps.
