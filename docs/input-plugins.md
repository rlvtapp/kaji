# Input plugins

Input plugins read source contracts and publish typed capabilities. Output plugins
consume those capabilities through Kaji's existing provider/consumer graph. A
community format can register its own format identifier and contract types without
adding a variant to Kaji core.

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

The current providers support inspection and native contract publication. Existing
HTTP SDKs consume `AdaptedApi`; GraphQL, event, RPC and workflow SDKs need native
output consumers. See [architecture](architecture.md) for the layer boundaries.

The [Node API](../packages/cli/sdk/README.md#input-plugins) exposes the same five
compiled Rust providers through individually installable npm input packages and
the `@relevate/kaji-plugins` bundle. A JavaScript output plugin can consume
their summary and diagnostics. `defineInputPlugin` also lets Node packages
publish a parser; one that returns Kaji's normalized HTTP API can feed the
existing Rust SDK renderers.

## Inspect the built-in inputs

Choose a format explicitly. Use `--provider` when more than one provider supports it.

Build the CLI from this checkout, then run:

```sh
cargo build -p kaji-cli
./target/debug/kaji contract plugins --format json
./target/debug/kaji contract inspect schema.graphql --input-format graphql --format json
./target/debug/kaji contract inspect events.yaml --input-format asyncapi --provider asyncapi.roas
./target/debug/kaji contract inspect workflows.yaml --input-format arazzo --format json
./target/debug/kaji contract inspect service.proto --input-format protobuf
./target/debug/kaji contract inspect service.capnp --input-format capnproto
```

The JSON report contains `provider`, `source`, `summary` and `diagnostics`. Parsing
errors fail the command and write an explanation to stderr. Arazzo source URLs are
reported as unresolved: source operation contracts are not loaded or verified by
this inspection. Workflow inspection does not perform API calls.

| Format | Provider | Published native contract | Scope |
| --- | --- | --- | --- |
| GraphQL SDL | `graphql.apollo` | `GraphqlDocument` | Apollo validated schema; query/mutation/subscription roots, arguments, type wrappers, directives and extensions |
| AsyncAPI | `asyncapi.roas` | `AsyncApiDocument` | 2.6.0, 3.0.0 and 3.1.0 typed models plus source JSON; channels/messages/bindings retained; invalid local references fail, external references require bundling |
| Arazzo | `arazzo.roas` | `ArazzoDocument` | 1.0.0, 1.0.1 and 1.1.0 typed models plus source JSON; dependency/step checks, reusable references, source URLs and criteria retained; no execution |
| Protobuf | `protobuf.protox` | `ProtobufDocument` | Pure Rust compilation to descriptor pool; imports relative to the root directory; message/enum/service types and all RPC streaming forms |
| Cap'n Proto | `capnproto.capnp` | `CapnProtoDocument` | Official external `capnp` compiler; retained binary `CodeGeneratorRequest`, imported types and capability methods |

Cap'n Proto requires the `capnp` executable on PATH. Its binary descriptor parser
is tested independently; real source compilation is a separate check requiring
that executable. Protobuf include directories beyond the root directory are not
configurable in this first interface. GraphQL accepts a complete SDL schema;
introspection JSON and separate operation documents are not accepted as SDL.

## Select or replace a provider

### Include only the formats you need

`InputRegistry` belongs to `kaji-core`; each parser and provider belongs to its own `kaji-input-*` crate under `crates/inputs/`.
The `kaji-inputs` convenience bundle exposes optional Cargo features named `graphql`, `asyncapi`, `arazzo`,
`protobuf` and `capnproto`, enabled by default. An embedded application can build
only the providers it needs:

```toml
kaji-inputs = { path = "../kaji-inputs", default-features = false, features = ["graphql"] }
```

```rust
use std::path::Path;
use kaji_inputs::{default_registry, graphql::GraphqlDocument};

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
use kaji_inputs::{default_registry, graphql::GraphqlDocument, InputProvider};

let input = InputProvider::<GraphqlDocument>::new(
    Arc::new(default_registry()?), "graphql", "schema.graphql",
).using("graphql.apollo");
let input_handle = input.handle();
// In an output plugin: requires -> Requirement::on(Some(input_handle))
// In generate: cx.inputs.get::<GraphqlDocument>()?
// Register the input and consumer in the same Package<L>.
```

Handles remain package-local. Consumers request their native document capability.
See [typed plugin contracts](typed-plugins.md#contracts-describe-actual-outputs)
for dependency declarations and output publication.

## Supply a normalized HTTP input later

An input plugin which normalizes an HTTP contract can publish `AdaptedApi`,
including its security scheme catalog. `kaji::generate_with_input` passes this
contract to the existing SDK packages. A future `openapi.roas` input can therefore
provide the same capability as a compiler-backed input without changing SDK
renderers. That parser integration is not implemented yet.

A native document without `AdaptedApi` fails explicitly when passed to the HTTP
SDK bridge. Input substitution does not establish OpenAPI parser equivalence.

## Verification

```sh
cargo test -p kaji-core input::
cargo test -p kaji-inputs -p kaji-input-graphql -p kaji-input-asyncapi -p kaji-input-arazzo -p kaji-input-protobuf -p kaji-input-capnproto
cargo test -p kaji-cli --test contract
cargo test -p kaji --lib registered_input
cargo check -p kaji-inputs --no-default-features
cargo check -p kaji-inputs --no-default-features --features graphql
```

The suite covers format semantics, invalid references, registry selection,
package graph consumers, CLI routing and the normalized HTTP bridge.
Cap’n Proto source checks need `capnp`; absent tools are reported as skips.
See [verification](verification.md) for toolchain coverage and baseline failures.

## Independent provider crates

| Format | Crate | Provider |
| --- | --- | --- |
| GraphQL | `kaji-input-graphql` | `GraphqlInput` |
| AsyncAPI | `kaji-input-asyncapi` | `AsyncApiInput` |
| Arazzo | `kaji-input-arazzo` | `ArazzoInput` |
| Protobuf | `kaji-input-protobuf` | `ProtobufInput` |
| Cap’n Proto | `kaji-input-capnproto` | `CapnProtoInput` |

Each crate owns its parser dependencies, native document type and provider.
Applications can depend on a provider directly and register it in the core
registry. `kaji-inputs` only reexports providers and assembles the default
registry. Future Roas OpenAPI providers can follow the same structure.

## Upstream corpus and end-to-end coverage

Fixtures are pinned to immutable upstream commits with licenses, file sizes and
SHA-256 digests; normal tests do not download documents.

| Input | Retained upstream corpus | Coverage |
| --- | --- | --- |
| GraphQL | GitHub schema from Octokit, 1.18 MB | Native validation, publication, broken-reference rejection, documentation output |
| AsyncAPI | Official Slack RTM 2.6/3.0/3.1, ADEO Kafka, Kraken WebSocket, 10–25 KB each | Versions, exact operations/types, event output, external-reference rejection |
| Arazzo | Official BNPL, FAPI PAR, OAuth, pet coupons, 5–10 KB each | Exact workflows/steps, source diagnostics, workflow output |
| Protobuf | Official v21.12 conformance schema and imports, 51 KB | 135 types, 2 methods, repository import roots; v29.3 editions rejected explicitly |
| Cap’n Proto | Official v1.1.0 test/schema/RPC corpus, 158 KB, plus 320 KB compiler request | 277 types, 49 methods, streaming identity, binary inspection and source compilation |

`crates/kaji-inputs/tests/native_pipeline.rs` routes each native document through
`InputProvider`, typed requirements, graph ordering and an output consumer that
emits documentation. This establishes source-to-artifact routing. It does not
establish format-specific SDK runtime behavior; those generators need their own
compilation and execution tests as they are implemented.

RPC parsers also expose `load_with_includes` for repository import roots.
The CLI searches the input directory by default. Protobuf supports proto2/proto3
and reports editions as unsupported. GraphQL validates SDL defaults in addition
to Apollo’s schema checks.

For the full test matrix, ignored toolchain checks and known baseline snapshot
failure, see [verification](verification.md). Corpus tests establish parsing and
source-to-documentation routing; generated SDK compilation and runtime behavior
require separate format-specific generators and tests.
