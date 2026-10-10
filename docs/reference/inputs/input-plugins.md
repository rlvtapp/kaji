# Input plugins

Input plugins read source contracts and publish typed capabilities. Output plugins
consume those capabilities through Poolster's existing provider/consumer graph. A
community format can register its own format identifier and contract types without
adding a variant to Poolster core.

## Input → core → output

```text
Source -> InputPlugin -> InputContract -> InputProvider<C> -> Plugin<L> -> files
```

| Hook | Responsibility |
| --- | --- |
| `InputPlugin::load` | Parse and validate a source; publish native typed values. |
| `InputRegistry` | Select a registered provider by format and optional ID. |
| `InputProvider<C>` | Publish the selected input capability in a package graph. |
| `Plugin<L>` | Declare requirements, consume contracts and emit output. |

All six bundled inputs publish native contracts and available building blocks.
OpenAPI publishes `AdaptedApi` for existing HTTP generators. GraphQL operations,
RPC services, events and workflows use their own owned contracts, retaining native
documents for details outside them. The unreleased alpha.2 implementation supports GraphQL
→ TypeScript/Rust through CLI and npm, Protobuf → Go gRPC, AsyncAPI → TypeScript/Kafka and Arazzo → TypeScript
runners; these additions are not an alpha.2 release yet. Cap’n Proto output is open.
See the [support matrix](../../plugin-support-matrix.md), [remaining work](native-pipelines.md#remaining-work)
and [contracts/block flow](../../internals/contracts.md).

The [Node API](../../../packages/npm/sdk/README.md#input-plugins) exposes the same five
compiled Rust providers through individually installable npm input packages and
the `@relevate/poolster-plugins` bundle. A JavaScript output plugin can consume
their summary and diagnostics. `defineInputPlugin` also lets Node packages
publish a parser; one that returns Poolster's normalized HTTP API can feed the
existing Rust SDK renderers.

## Inspect the built-in inputs

Choose a format explicitly. Use `--provider` when more than one provider supports it.

Build the CLI from this checkout, then run:

```sh
cargo build -p poolster-cli
./target/debug/poolster contract plugins --format json
./target/debug/poolster contract inspect schema.graphql --input-format graphql --format json
./target/debug/poolster contract inspect events.yaml --input-format asyncapi --provider asyncapi.roas
./target/debug/poolster contract inspect workflows.yaml --input-format arazzo --format json
./target/debug/poolster contract inspect service.proto --input-format protobuf
./target/debug/poolster contract inspect service.capnp --input-format capnproto
```

The JSON report contains `provider`, `source`, `summary` and `diagnostics`. Parsing
errors fail the command and write an explanation to stderr. Arazzo source URLs are
reported as unresolved: source operation contracts are not loaded or verified by
this inspection. Workflow inspection does not perform API calls.

| Format | Provider | Published native contract | Scope |
| --- | --- | --- | --- |
| OpenAPI | `openapi.compiler-artifacts` | `AdaptedApi`, schema/operation blocks | Independent input crate reads compiler artifacts; existing CLI compiles source first. `openapi.compiler` is an explicitly registered raw-source provider. |
| GraphQL SDL / introspection JSON + operations | `graphql.apollo` | `GraphqlDocument`, input-model blocks, `GraphqlOperations` and operation blocks when documents are supplied | Validated schema, selection-specific results/variables; schema-only operation collections are unavailable |
| AsyncAPI | `asyncapi.roas` | `AsyncApiDocument`, message blocks, `EventOperations` when Kafka lowering succeeds | Inspection 2.6/3.0/3.1; executable 3.0/3.1 Kafka subset; partial message blocks retain diagnostics |
| Arazzo | `arazzo.roas` | `ArazzoDocument`, resolved `WorkflowOperations` and step blocks | Inspection 1.0.0/1.0.1/1.1.0; explicit local OpenAPI mappings enable the supported sequential runner subset; unresolved step collections are unavailable |
| Protobuf | `protobuf.protox` | `ProtobufDocument`, `RpcContract`, method blocks | Proto2/proto3 descriptors, import roots and all RPC streaming directions; official Go gRPC output |
| Cap’n Proto | `capnproto.capnp` | `CapnProtoDocument`, node/capability blocks | Official binary compiler request; real source compilation needs `capnp`; Rust output remains unimplemented |

Cap’n Proto requires `capnp` on PATH. Protobuf and Cap’n Proto support explicit
`InputOptions.import_roots`. GraphQL uses `InputOptions.operation_files` with
SDL or local introspection JSON. Quoted full-file schema/operation imports use
`import_roots`; selective named imports remain unsupported. AsyncAPI uses `broker`
configuration; Arazzo uses `workflow_sources`. Unsupported provider options fail.
Input inspection does not connect to a broker or execute workflow steps.

Each input crate exposes public `contracts` and `blocks` modules. Shared owned
definitions preserve graph type identity; output interfaces do not require parser
library types. Custom inputs may publish whole contracts without any blocks.
Collections distinguish `Complete`, `Partial { diagnostics }` and `Unavailable`;
block handlers declare the completeness they need. See [contract safety and hooks](../../internals/contracts-and-blocks.md).

## Select or replace a provider

### Include only the formats you need

`InputRegistry` belongs to `poolster-core`; each parser and provider belongs to its own `poolster-input-*` crate under `crates/inputs/`.
The `poolster-inputs` convenience bundle exposes optional Cargo features named `openapi`, `graphql`, `asyncapi`, `arazzo`,
`protobuf` and `capnproto`, enabled by default. An embedded application can build
only the providers it needs:

```toml
poolster-inputs = { path = "../poolster-inputs", default-features = false, features = ["graphql"] }
```

```rust
use std::path::Path;
use poolster_inputs::{default_registry, graphql::GraphqlDocument};

let registry = default_registry()?;
let loaded = registry.load("graphql", Some("graphql.apollo"), Path::new("schema.graphql"))?;
let schema = loaded.contract.get::<GraphqlDocument>()?;
// schema.schema retains Apollo's validated native schema.
```

### Register a replacement

Each input implements `InputPlugin`: a unique `id`, a format identifier, and a
`load` method returning `InputContract`. It can publish several values implementing
the same `Contract` trait used by output providers. `get::<C>()` and `take::<C>()`
fail with the missing capability's name. Duplicate publications fail and retain
the original value.

Register an alternative with `registry.register(MyInputPlugin)?`. When multiple
providers serve a format, select one by ID. Automatic selection fails on ambiguity.
Duplicate IDs, mismatched format claims and unknown providers also fail. Registered
plugins are native Rust linked into the application; a JSON configuration cannot
load an arbitrary crate or binary plugin.

## Feed an input into the output graph

`InputProvider<C>` wraps a registry/source selection and publishes contract `C` in
a package. Consumers declare `Requirement::on(Some(provider.handle()))`, then read
`cx.inputs.get::<C>()`. The engine orders the parser before its consumers, even
when the consumer was registered first.

```rust
use std::sync::Arc;
use poolster_inputs::{default_registry, graphql::GraphqlDocument, InputProvider};

let input = InputProvider::<GraphqlDocument>::new(
    Arc::new(default_registry()?), "graphql", "schema.graphql",
).using("graphql.apollo");
let input_handle = input.handle();
// In an output plugin: requires -> Requirement::on(Some(input_handle))
// In generate: cx.inputs.get::<GraphqlDocument>()?
// Register the input and consumer in the same Package<L>.
```

Handles remain package-local. Generation consumers should request the
Poolster-owned protocol capability; inspection consumers may request native parser
documents. See [native generation](native-pipelines.md) for the supported recipes,
warning/skip policy and subscription transport boundary.
See [typed plugin contracts](../../internals/typed-plugins.md#contracts-describe-actual-outputs)
for dependency declarations and output publication.

## Supply or replace the normalized HTTP input

An input plugin which normalizes an HTTP contract can publish `AdaptedApi`,
including its security scheme catalog. `poolster::generate_with_input` passes this
contract to the existing SDK packages. A future `openapi.roas` input can therefore
provide the same capability as a compiler-backed input without changing SDK
renderers. That parser integration is not implemented yet.

A native document without `AdaptedApi` fails explicitly when passed to the HTTP
SDK bridge. Input substitution does not establish OpenAPI parser equivalence.

## Verification

```sh
cargo test -p poolster-core input::
cargo test -p poolster-inputs -p poolster-input-openapi -p poolster-input-graphql -p poolster-input-asyncapi -p poolster-input-arazzo -p poolster-input-protobuf -p poolster-input-capnproto
cargo test -p poolster-cli --test contract
cargo test -p poolster --lib registered_input
cargo check -p poolster-inputs --no-default-features
cargo check -p poolster-inputs --no-default-features --features graphql
```

The suite covers format semantics, invalid references, registry selection,
package graph consumers, CLI routing and the normalized HTTP bridge.
Cap’n Proto source checks need `capnp`; absent tools are reported as skips.
See [verification](../../verification/verification.md) for toolchain coverage and baseline failures.

## Independent provider crates

| Format | Crate | Provider |
| --- | --- | --- |
| OpenAPI | `poolster-input-openapi` | `OpenApiInput` |
| GraphQL | `poolster-input-graphql` | `GraphqlInput` |
| AsyncAPI | `poolster-input-asyncapi` | `AsyncApiInput` |
| Arazzo | `poolster-input-arazzo` | `ArazzoInput` |
| Protobuf | `poolster-input-protobuf` | `ProtobufInput` |
| Cap’n Proto | `poolster-input-capnproto` | `CapnProtoInput` |

Each crate owns its parser dependencies, native document type and provider.
Applications can depend on a provider directly and register it in the core
registry. `poolster-inputs` only reexports providers and assembles the default
registry. Future Roas OpenAPI providers can follow the same structure.

## Upstream corpus and end-to-end coverage

Fixtures are pinned to immutable upstream commits with licenses, file sizes and
SHA-256 digests; normal tests do not download documents.

| Input | Retained upstream corpus | Coverage |
| --- | --- | --- |
| GraphQL | GitHub schema from Octokit, 1.18 MB | Native validation/publication, operation lowering, broken-reference rejection, documentation output; generated package checks described separately |
| AsyncAPI | Official Slack RTM 2.6/3.0/3.1, ADEO Kafka, Kraken WebSocket, 10–25 KB each | Versions, exact operations/types, event output, external-reference rejection |
| Arazzo | Official BNPL, FAPI PAR, OAuth, pet coupons, 5–10 KB each | Exact workflows/steps, source diagnostics, workflow output |
| Protobuf | Official v21.12 conformance schema and imports, 51 KB | 135 types, 2 methods, repository import roots; v29.3 editions rejected explicitly |
| Cap’n Proto | Official v1.1.0 test/schema/RPC corpus, 158 KB, plus 320 KB compiler request | 277 types, 49 methods, streaming identity, binary inspection and source compilation |

`crates/inputs/bundle/tests/native_pipeline.rs` routes each native document through
`InputProvider`, typed requirements, graph ordering and an output consumer that
emits documentation. This establishes source-to-artifact routing. It does not
establish format-specific SDK runtime behavior; those generators need their own
compilation and execution tests as they are implemented.

RPC parsers also expose `load_with_includes` for repository import roots.
The CLI searches the input directory by default. Protobuf supports proto2/proto3
and reports editions as unsupported. GraphQL validates SDL defaults in addition
to Apollo’s schema checks.

For implemented generation capabilities and concrete protocol/Forge follow-ups, see
[native pipelines](native-pipelines.md). For the full test matrix, ignored
toolchain checks and baseline failures, see [verification](../../verification/verification.md). Corpus tests establish parsing and
source-to-documentation routing; generated SDK compilation and runtime behavior
require separate format-specific generators and tests.
