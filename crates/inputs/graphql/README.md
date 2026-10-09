# poolster-input-graphql

Native `graphql` input provider. Register `GraphqlInput` directly with
`poolster_core::input::InputRegistry`, or enable `graphql` in `poolster-inputs`.
The provider publishes `GraphqlDocument` for typed consumers in Poolster's plugin graph.

For generation, call `InputRegistry::load_with_options` or configure
`InputProvider::<poolster_core::native::GraphqlOperations>::with_options` with
`InputOptions.operation_files`. The schema is the primary source; all operation
files are combined and validated together so fragments may span files. Paths
are resolved by the caller (the CLI resolves them relative to its recipe).

The additional `poolster.graphql-operations.v1` contract contains owned model
shapes, operation kinds and individual executable documents. Outputs do not
need Apollo compiler types. It preserves nested list nullability, input presence
and default literals, aliases, merged selections, named/inline fragments,
concrete variants of interfaces/unions, and conditional presence from
`@skip`/`@include`. Selected `__typename` fields become concrete string literals.
Schema and combined operation text remain available for native details.

Query, mutation and subscription operations are represented separately;
subscription transport support is an output capability. TypeScript and Rust HTTP
clients support queries and mutations through the CLI and npm language plugins.
See the [TypeScript guide](../../../docs/graphql-typescript.md) and
[Rust guide](../../../docs/graphql-rust.md). Custom scalar representations are
unknown unless an output provides a mapping. Executable directives other than
`@skip` and `@include`, variable directives, and fragment definition directives
are rejected rather than silently assigned invented execution semantics.
GraphQL import roots, broker options and workflow source options are rejected.
Schema-only loading retains its existing inspection contract; generation
requires at least one executable operation.
