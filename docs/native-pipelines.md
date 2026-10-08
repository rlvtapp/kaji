# Native generation in Poolster

Poolster uses capability contracts between input and output plugins. A format name
selects a parser; a typed requirement selects what a generator can consume.
GraphQL operations are not HTTP `Api` operations. Existing OpenAPI generators keep
their existing API, security catalog, recipes and transports.

## Architecture and extension boundary

`InputPlugin` publishes one or more values implementing `Contract`.
`InputProvider<C>` feeds a selected capability into the existing package graph;
output plugins declare `Requirement<C>` and read it through `cx.inputs`.
Community crates can define new contract types and providers without modifying a
core format enum. Register a replacement provider under its own identifier and
select it explicitly when a format has multiple providers.

`poolster_core::native` owns the GraphQL generation interface. Its model wrappers
preserve nullability independently from field presence; lists preserve element
wrappers, and selections preserve aliases and concrete alternatives. Parser
objects remain in input crates. Native schema and operation text remain available
for details outside the normalized contract. The versioned contract name is an
identity for linked Rust plugins, not a dynamic binary ABI or a JavaScript wire
protocol. Breaking contract changes require a new contract type/version.

`Packages::generate_native` and `poolster::generate_native` reuse graph ordering,
package finalization, file ownership, customization and release assembly. Plugins
must opt into native execution with `supports_native_input`; typed requirements
still determine the protocol accepted. HTTP-only packages are skipped before source loading and emit a warning.
`generate_native_report` exposes package names and reasons as a structured skip
report. When every output is skipped, the CLI succeeds without touching existing
output. Mixed recipes preserve skipped package bytes and prior ownership hashes,
including local edits. Retention is applied after final package assembly, at
materialization time. Invalid inputs, unsupported options in a supported pipeline,
and generator failures remain errors. The
legacy engine context remains internally available with an empty HTTP API for
source compatibility; native generators consume their contract, never a converted
HTTP operation list. Native execution rejects HTTP middleware/idempotency policies.

`ts::graphql(provider)` also publishes `ts::GraphqlClient`. Its typed handle lets
other plugins consume the actual generated operation, variables and result
symbols, operation kinds and runtime module. This supports downstream artifacts
without guessing file names; a tested Post consumer emits imports from those
symbols. A single plugin can declare several optional contract inputs and branch
on the available type, or require multiple contracts when it needs to combine them.
The current CLI recipe is one source; multi-input recipes remain follow-up work.

See the [plugin support matrix](plugin-support-matrix.md) for output-by-output
capabilities and release checklists.

## Delivery boundary

| Pipeline | Native parsing/inspection | Usable package generation | Remaining work |
| --- | --- | --- | --- |
| OpenAPI → existing languages | Existing compiler and adapter | Existing SDK pipelines preserved | Existing target-specific limits still apply |
| GraphQL → TypeScript | Validated SDL and operation documents | First native pipeline in this change | See feature and verification details below |
| Protobuf → Go gRPC | Descriptor pool, imports, unary and streaming RPC metadata | Not implemented | Stable RPC lowering, official protoc/protoc-gen-go/protoc-gen-go-grpc orchestration, local gRPC tests |
| AsyncAPI → TypeScript | Versioned typed source and broker bindings retained | Not implemented | Event lowering, one explicit Kafka broker adapter and local broker integration |
| Arazzo → workflow runner | Versioned source, workflows/steps and unresolved sources retained | Not implemented | Resolve source descriptions and operation identities, validate expressions, execute/test runners |
| Cap’n Proto → Rust | Official compiler request and capabilities retained | Not implemented | Official capnpc Rust generation, capnp-rpc wrappers and local capability RPC tests |

Only GraphQL establishes a new package pipeline here. Inspection and documentation
consumers for other protocols do not establish runnable SDK support. Protocol-owned
RPC, event and workflow generation contracts will be added with those working
pipelines, preserving protocol details rather than committing speculative HTTP-like
interfaces. The existing native parser documents remain available in the meantime.

## GraphQL feature matrix

| Feature | Support and boundary |
| --- | --- |
| SDL + operation files | Schema validated by Apollo; operation files combined and validated against it |
| Selections | Aliases, named/inline fragments, nested lists, enum values and per-concrete abstract result alternatives |
| Presence/nullability | Nullable values use `null`; omitted variable/input/conditional result fields are optional independently |
| Defaults | Native default literal retained; nullable or defaulted variables/input fields may be omitted; server applies defaults |
| Conditional directives | `@skip` and `@include`; literal exclusions removed from result types, variable conditions mark presence optional |
| Query/mutation | Operation-specific variable/result types and functions using an injected `GraphqlTransport` |
| HTTP | Fetch POST transport; configurable fetch/headers and AbortSignal; no automatic retries |
| Results | Discriminated `success`, `partial`, `error` results preserve GraphQL errors, partial data and extensions |
| Transport errors | HTTP failures throw `GraphqlHttpError`; malformed envelopes/JSON throw `GraphqlProtocolError` |
| Subscriptions | Separately enabled, injected `SubscriptionTransport` yielding async results; no bundled WebSocket/SSE transport |
| Custom scalars | Generated as `unknown`; application-specific scalar codecs/mappings are follow-up work |
| Introspection/imports | Introspection JSON and schema imports are unsupported; supply complete SDL |
| Executable extensions | Custom executable directives, defer/stream and operation/variable/fragment-definition directives are rejected |
| TypeScript symbol names | Unsupported identifiers/collisions fail explicitly rather than emitting invalid code |
| Node generation API | Existing parser inspection unchanged; this new usable output is exposed through Rust/CLI, not the npm configuration engine |

```json
{
  "input": {
    "format": "graphql",
    "provider": "graphql.apollo",
    "path": "schema.graphql",
    "options": { "operation_files": ["operations.graphql"] }
  },
  "output": { "path": "generated" },
  "packages": [{
    "language": "typescript",
    "path": "graphql",
    "name": "@example/graphql",
    "version": "1.0.0",
    "plugins": [{ "name": "graphql", "transport": "fetch" }]
  }]
}
```

Paths resolve relative to the recipe. Existing `openapi` recipes remain valid;
set either `input` or `openapi`. Enable injected subscription functions with
`"subscriptions": true` on the GraphQL output plugin. HTTP-specific defaults,
layout, middleware and SDK options are rejected for supported GraphQL packages.

```sh
poolster generate schema.graphql --input-format graphql \
  --provider graphql.apollo --operation operations.graphql \
  --language typescript --output generated
poolster generate --config poolster.json --check
```

`--operation` and `--import-root` repeat. `--broker-config` reads a JSON file and
`--workflow-source name=path` supplies local source mappings. Recipe equivalents
are `input.options.operation_files`, `import_roots`, `broker` and
`workflow_sources`. These resolution options do not create missing output
pipelines: GraphQL rejects import, broker and workflow options; subsequent
providers/output pipelines must implement them explicitly.

Native source/options/config hashes are recorded in
`.poolster-native-generation.json`. Regenerate using the original command or
recipe; `poolster update` continues to support HTTP replay only.

The generated package can be built with its pinned TypeScript development
requirement and imported from `dist/index.js`:

```ts
import { createGraphqlHttpTransport, Viewer } from './generated/graphql/dist/index.js';
const result = await Viewer(createGraphqlHttpTransport('http://localhost:4000/graphql'), {});
if (result.kind === 'success' || result.kind === 'partial') console.log(result.data);
if (result.kind !== 'success') console.error(result.errors);
```

A complete [runnable example](../examples/graphql-native/README.md) includes a
local GraphQL server and consumer that checks successful and partial results.

## Concrete next pipelines

1. **Protobuf / Go:** lower descriptors to Poolster-owned services and method
   contracts retaining client/server streaming independently, proto2 presence,
   proto3 optional/oneof semantics, maps, enums and imports. Invoke pinned official
   generators rather than recreating protobuf encodings. Test unary, each streaming
   direction, cancellation, malformed imports, provider substitution and repeat
   generation against a local gRPC server; compile both generated client and server.
2. **AsyncAPI / TypeScript / Kafka:** choose a single supported broker/version and
   client library explicitly. Lower messages, payload/header schemas, channel
   address, direction, correlation identifiers and Kafka bindings without deleting
   unknown native bindings. Reject other bindings and unsupported schema formats.
   Use a local Kafka broker to test produce/consume, serialization, failure and
   regeneration, with pinned official AsyncAPI source fixtures.
3. **Arazzo:** resolve sourceDescriptions through explicit local mappings before
   generation; bind operationId/operationPath to the loaded source capability.
   Keep dependency order, runtime expressions, success criteria and failure actions
   in a workflow-specific contract. Reject unresolved/ambiguous sources and
   unsupported expressions. Test actual workflow execution against local protocol
   servers, including failure/retry paths and source changes.
4. **Cap’n Proto / Rust:** invoke pinned official `capnp` plus `capnpc`, retain
   schema IDs, imports, ordinals, unions, defaults and capabilities. Generate
   messages and capability clients/servers using `capnp-rpc`, including streaming
   where supported by the official toolchain. Compile and run a local capability
   server; fail clearly if compiler/runtime prerequisites are absent.

For each pipeline, add a narrowly scoped versioned output contract only after a
working implementation establishes its semantics. Share model concepts only when
their meaning agrees; presence, defaults and encoding rules differ by protocol.

## Forge work remains separate

No Forge compatibility claim is made by native input support or existing recipe
migration. The follow-up audit must record a tested capability matrix:

| Capability | Current evidence | Required follow-up |
| --- | --- | --- |
| Schema JSONPath overlays | Not established by source customization | Apply overlays before input validation/lowering; define JSONPath dialect, ordering, missing/multiple matches and provenance; test each parser |
| Forge configuration migration | Existing migration is not a Forge audit | Pin representative Forge configs; classify each field as equivalent, translated, rejected or unsupported; preserve originals and emit a migration report |
| Cap’n Web output | No bundled native output pipeline | Confirm intended scope, pin official runtime/generator, define its capability contract and compile/run generated package |
| Whole Forge compatibility | No tested matrix | Run audited fixtures per capability and publish exact versions, supported combinations and unsupported behavior |

Generated-source add/replace/patch customization remains a separate stage from
schema overlays. It cannot validate that a schema-level JSONPath overlay was
applied. A format parser, workflow inspector or configuration translator alone is
insufficient evidence of Forge compatibility.
