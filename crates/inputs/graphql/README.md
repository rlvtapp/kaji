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

Query, mutation and subscription operations are represented separately. All ten
SDK language outputs consume these owned contracts; subscriptions are an opt-in
output capability. See the [support matrix](../../../docs/plugin-support-matrix.md)
and [advanced guide](../../../docs/reference/outputs/graphql-capabilities.md).
Custom scalar representations and runtime callbacks are configured by the output.

Schemas accept SDL or local introspection JSON (`__schema` or `data.__schema`).
Quoted full-file `# import` statements resolve beside each source and through
`InputOptions.import_roots`; operation fragments are deduplicated across files.
`GraphqlDocument.native_documents` retains original imported documents, including
introspection JSON. Selective named imports, broker options and workflow source
options are rejected. Schema-only loading retains inspection contracts and
explicitly unavailable operation blocks; usable generation requires operations.

`InputOptions.graphql_incremental = true` publishes the separate owned
`GraphqlIncrementalOperations` contract for experimental path-based multipart
`deferSpec=20220824` consumers. It preserves deferred/streamed coordinates, labels,
conditions and initial counts without treating partial snapshots as final models.
Abstract incremental selections require an unconditional non-deferred common
`__typename` (aliases supported). Ordinary lowering still rejects `@defer` and
`@stream`. Other custom executable directives, variable directives and fragment
definition directives remain unsupported.
