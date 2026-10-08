# Architecture

Kaji connects source contracts to generated files through typed plugins.

```text
Source -> input plugin -> typed contract -> output plugins -> owned files
                              |                |
                         core registry    package graph
```

## Input, core, and output hooks

| Layer | Owns | Extension hook |
| --- | --- | --- |
| Input provider | Parsing, validation, native schema and source diagnostics | Implement `InputPlugin`; publish values implementing `Contract`. |
| Core | Registry selection, typed dependencies, execution order and safe output ownership | Register providers in `InputRegistry`; connect them with `InputProvider<C>`. |
| Output plugin | Language syntax, runtime ABI, artifacts and optional output contracts | Implement `Plugin<L>`; declare requirements, read inputs, emit files. |
| Language package | Settings, shared workspace, manifests and exports | Implement `Language` and its finalization hooks. |

Input providers live in `crates/inputs/`; output providers live in
`crates/plugins/`. Community formats and languages can extend these hooks without
adding a core enum variant. Plugins are Rust code linked into the application.
The shipped CLI exposes the providers compiled into its registry.

## Native contracts and HTTP SDKs

GraphQL, AsyncAPI, Arazzo, Protobuf and Cap’n Proto providers publish native
contracts. These retain queries, events, workflows and RPC streaming semantics.
Their current outputs support inspection and tested documentation consumers;
protocol-specific SDK generators require their own consumers.

Existing HTTP SDKs consume `Api` and its security catalog. The bundled Go OpenAPI
compiler produces local artifacts that Rust loads as `AdaptedApi`. An alternative
HTTP input provider can publish the same contract through `generate_with_input`.
Generated SDKs do not run the source compiler.

See [input providers](input-plugins.md) for supported formats and inspection.

## Package generation

A release groups `Package<L>` instances, each with its own directory, settings and
plugins. Contracts and handles are package-local. Declared requirements determine
provider order; missing or ambiguous inputs and cycles fail before emission.

TypeScript, Rust and Go expose independently selectable model, transport,
operation and client providers. Their `sdk()` convenience plugins reuse maintained
renderers. Languages own their mutable workspaces; TypeScript tracks symbols,
dependencies and exports. Finalizers assemble shared metadata.

The engine then applies Post plugins, bundled middleware, language file
finalization and ordered source overlays. Ownership checks protect authored files
when checking or writing the result.

Continue with [typed plugins](typed-plugins.md), [native SDK providers](native-sdk-providers.md)
and [safe regeneration](safe-regeneration.md).
