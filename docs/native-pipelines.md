# Native generation in Poolster

Audited **9 October 2026** against the unreleased `alpha-2` working tree.
The manifests still carry `0.5.0-alpha.1`; this is implementation status, not a
claim that the additions are already in that published release.

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

`poolster_core::native` owns the GraphQL, RPC, event and workflow generation interfaces. Its model wrappers
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
| Protobuf → Go gRPC | Proto2/proto3 descriptors, imports and RPC metadata | Official messages, clients and server interfaces; unary and all streaming directions | Editions, broader official fixture coverage and additional output languages |
| AsyncAPI → TypeScript | 2.6/3.0/3.1 native document and message blocks | 3.0/3.1 JSON messages and Kafka producer/consumer | Types-only output, broader schemas/bindings, security and other brokers |
| Arazzo → TypeScript | Native document and explicit local source resolution | Sequential HTTP runners with local workflow dependencies | Actions/retries, richer expressions/criteria and additional source types |
| Cap’n Proto → Rust | Official compiler request and capability/node blocks | Not implemented | Official capnpc Rust generation, capnp-rpc interfaces and local capability tests |

Whole contracts and building blocks are both supported. Every bundled input
publishes its available block collections; missing or partially representable
collections carry completeness diagnostics. Decomposition remains optional for
custom inputs. HTTP output entry points now select whole contracts and optional complete model/endpoint
blocks through the [output migration bridge](output-contract-migration.md).
Protocol-native generators retain their authoritative whole contracts. Blocks
are data for plugins to inspect or transform; a model block does not itself
mean “generate types.” See [contracts and blocks](contracts-and-blocks.md),
[the input flow](input-contract-flow.md) and [symbol planning](symbol-planning.md).

Consumers select providers explicitly where bindings would otherwise be ambiguous.
Stable entity IDs are separate from contract instance/revision references.
Extractors can stamp blocks from the selected transformed contract; paired
handlers reject revision mismatches. Custom contracts are not automatically
decomposed. Deterministic name reservation/resolution is available as an opt-in
stage; existing generators have not all migrated to it. Versioned serialization
codecs are available, but typed Node hook dispatch is still follow-up work.

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
`workflow_sources`. GraphQL accepts operation files; Protobuf accepts import roots; AsyncAPI
accepts Kafka broker configuration; Arazzo accepts workflow source mappings.
Each provider rejects options outside its supported pipeline.

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

## Protobuf → Go gRPC

`go::grpc(module).input(handle)` consumes `RpcContract` and invokes pinned official
**protoc 34.2**, **protoc-gen-go 1.36.11**, and **protoc-gen-go-grpc 1.6.2**.
The generated module pins protobuf 1.36.11 and gRPC 1.83.2 and requires **Go 1.25+**.
Install the tools explicitly; generation checks their versions and does not
install them automatically. Supply `go_package` declarations or per-file mappings
through `.go_package(file, mapping)`; selected local packages must fit the module.
Configure tool paths with `.toolchain(GrpcToolchain { ... })`.

Generated `.pb.go` messages and `_grpc.pb.go` clients/server interfaces preserve
wire semantics through the official descriptor set, including field presence,
oneofs, maps and enums. Applications implement the server interfaces. Unary,
client streaming, server streaming and bidirectional streaming are supported.
The provider exposes method blocks without replacing the authoritative wire
schema; editing those blocks alone does not rewrite descriptor bytes.

The explicitly enabled integration tests compile and execute a local TCP gRPC
server, including streaming, metadata, errors, deadlines, cancellation and race
checks. A pinned upstream proto2 import/public-import fixture also compiles.
Proto editions are unsupported by the parser; the old upstream `unverified_lazy`
conformance extensions are rejected by protoc 34.2. Those parser fixtures are
not claimed as supported generated Go packages.

## AsyncAPI → TypeScript / Kafka

`ts::asyncapi(Some(handle))` consumes `EventOperations` and publishes actual
`KafkaClient` symbols for downstream plugins. It generates message types,
producer/consumer functions and a runtime using **KafkaJS 2.2.4** and **Ajv 8.17.1**.
Generation only writes the package; it does not connect to Kafka.

The supported slice is AsyncAPI **3.0/3.1**, plaintext static Kafka topics, one
JSON message per operation, local acyclic schema references, primitive/object/array
schemas, required/optional and nullable fields, validation constraints, string
keys/headers and fixed singleton group/client binding IDs. Broker options use
`{"kind":"kafka","brokers":["127.0.0.1:29092"],"client_id":"orders"}`;
addresses may also come from supported document servers.

Security, schema registries, dynamic topics, replies, correlation identifiers,
traits, multiple messages, recursive/composed schemas and unsupported schema
keywords/bindings are rejected. AsyncAPI 2.6 is inspectable and exposes available
message blocks but has no executable Kafka lowering. Message blocks can be loaded
without broker configuration; incomplete projections include diagnostics. A
broker-independent types-only generator remains open.

The generated package was compiled and exercised against a real local
Kafka-compatible **Redpanda 25.3.11** broker with a pinned image digest. Tests cover
send/receive, keys, headers, invalid incoming/outgoing messages, provider replacement,
downstream symbols and regeneration. See the [broker reproduction guide](../crates/plugins/typescript/tests/fixtures/KAFKA.md).
This establishes that tested Kafka subset, not every Kafka deployment or AsyncAPI feature.

## Arazzo → TypeScript workflow runners

`ts::workflow(Some(handle))` consumes `WorkflowOperations` and publishes
`WorkflowClient` symbols. Supply `input.options.workflow_sources` mappings from
declared source names to local OpenAPI **3.0/3.1** files. Loading resolves sources
without downloading declared URLs or executing workflow steps.

Arazzo **1.0.0/1.0.1/1.1.0** supports sequential operation steps and local workflow
`dependsOn`, primitive path/query/header parameters, JSON bodies, typed inputs with
presence/defaults, and an exact `$statusCode == <integer>` success criterion.
Without a criterion, HTTP 2xx is required. Responses must be JSON or empty.
Supported expressions reference inputs, prior step/dependency outputs, JSON-pointer
response values and status codes. Bare operation IDs must be unambiguous;
qualified IDs and source-relative operation paths are supported.

Generated runners accept source base URL overrides, headers, Fetch and abort
signals. Failures expose completed steps/dependencies and stop execution.
Workflow outputs remain `unknown`; source schemas do not establish their types.
Nested calls, external dependencies, retries, success/failure actions, reusable
actions/parameters, JSONPath criteria, interpolation, non-JSON bodies, authenticated
operations and non-OpenAPI sources are rejected. See the [input guide](../crates/inputs/arazzo/README.md)
for exact expression syntax and runtime behavior.

Explicit integration tests compile and run a multi-step checkout and an operation
resolved from a checksum-pinned official OAuth OpenAPI source against local HTTP
servers. This does not establish support for the complete upstream OAuth workflow.

## CLI and recipe selection

| Input | Package language | Recipe plugin name | Required configuration |
| --- | --- | --- | --- |
| GraphQL | `typescript` | `graphql` | `input.options.operation_files` |
| Protobuf | `go` | `grpc` | Plugin `module`; toolchain paths if not on PATH; source `go_package` or plugin `go_packages` |
| AsyncAPI | `typescript` | `asyncapi` | Supported Kafka servers or `input.options.broker` |
| Arazzo | `typescript` | `workflow` | `input.options.workflow_sources` |

A supported native CLI package currently selects exactly one matching generator.
Rust composition permits additional typed consumers. Incompatible CLI packages
succeed with warnings and a structured skip report; invalid input or unsupported
features within a selected supported pipeline are errors. CLI recipes still select
one input source, and existing `openapi` recipes remain valid.

## Remaining work

These are implementation gaps, not claims of existing support:

- [ ] **CAPNP-1:** Pin official capnp/capnpc and capnp-rpc; generate Rust messages
  and capability clients/server interfaces while preserving IDs, ordinals, unions,
  defaults and imports. Compile and execute against a local capability server.
- [ ] **EVENT-1:** Add broker-independent AsyncAPI TypeScript message output from
  exposed blocks; define strict/partial collection policy and compile it without a broker.
- [ ] **EVENT-2:** Extend event schemas, correlation IDs and bindings; add explicit
  security support and failure tests before advertising them. Other brokers need
  their own tested transport adapter.
- [ ] **WORKFLOW-1:** Add retries/actions and richer expressions/criteria with
  explicit side-effect/cancellation semantics and local execution tests.
- [ ] **RPC-1:** Assess editions/toolchain alignment and additional RPC outputs;
  broaden pinned upstream fixtures without treating parser support as generation support.
- [ ] **GRAPHQL-1:** Scalar mappings/codecs, introspection/imports, incremental
  delivery and bundled subscription transport; each needs independent runtime tests.
- [ ] **NODE-1:** Expose native generation and versioned typed hook envelopes through
  the npm engine; current wrappers provide inspection, not the Rust/CLI pipelines.
- [ ] **COMPOSE-1:** Multi-input recipes with explicit bindings; preserve current
  single-input recipes. Do not auto-parallelize until shared writes/contracts are planned.
- [ ] **SYMBOL-1:** Migrate existing generators to shared two-phase deterministic
  naming, proving stable output across plugin registration order and regeneration.
- [ ] **VERIFY-1:** Finish the frozen alpha.2 205×10 HTTP corpus sweep, classify
  harness failures separately, and record direct regeneration confirmations.
- [ ] **RELEASE-1:** Set alpha.2 versions, verify built artifacts/installations and
  publish only after the final release checks.

The separate Forge audit below remains open. The [general generator backlog](generator-backlog.md)
tracks output layout, framework helpers and other HTTP artifacts.

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
