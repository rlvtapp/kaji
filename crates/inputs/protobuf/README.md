# poolster-input-protobuf

Native `protobuf` input provider. Register `ProtobufInput` directly with
`poolster_core::input::InputRegistry`, or enable `protobuf` in `poolster-inputs`.
The provider publishes `ProtobufDocument` for typed consumers in Poolster's plugin graph.

The parser supports proto2 and proto3, including imports, standard well-known
schemas, nested messages/enums, field defaults, oneofs and RPC streaming.
Protobuf editions are currently unsupported by protox 0.9. The pinned v29.3
upstream corpus verifies that this limitation returns an explicit diagnostic.

`load_with_includes(path, includes)` accepts explicit import roots for repository
layouts such as `google/protobuf/unittest.proto`. `load(path)` searches the root
file's directory. Provider callers can configure the same roots through `InputOptions.import_roots`.

Tests retain the official protobuf v21.12 conformance schema and its import
closure: 3 source files, 51,334 bytes, 135 summary types and 2 service methods.
The editions negative corpus uses v29.3: 3 source files, 63,634 bytes.
Each corpus includes its upstream license, immutable commit, and per-file
SHA-256 digests in `tests/fixtures/*/PROVENANCE.json`. Tests run offline.

`ProtobufInput` additionally publishes
`poolster_core::native::rpc::RpcContract` (`poolster.protobuf-rpc.v1`). Its owned
service/method metadata distinguishes unary, client-streaming, server-streaming
and bidirectional RPCs. The complete imported descriptor set is retained as
standard Protobuf `FileDescriptorSet` bytes; field presence, oneofs, maps,
extensions, options and wire numbers remain there rather than being flattened
into HTTP or GraphQL schemas. Physical source files are retained as text.

Use `InputOptions.import_roots` for imported files. Other provider options are
rejected. The existing `ProtobufDocument` inspection contract remains available.

The Go `grpc` output uses official pinned protoc and Go plugins to generate
messages, clients and server interfaces. It requires a module path and
`go_package` options (or explicit per-file mappings) for local sources. Protobuf
editions remain unsupported by protox; proto2 and proto3 inputs are supported.

`RpcContract::method_blocks(source_id)` explicitly projects an optional
`Blocks<RpcMethodBlock>` contract. Each block retains its service and file,
streaming capability tags, a stable native method coordinate, and references to
its request/response coordinates in the whole RPC contract. The bundled provider publishes this projection; custom providers are not
required to do so. The official descriptor set remains the
source for Go generation; editing a method projection alone does not modify
wire schemas.

The Go gRPC integration compiles the pinned upstream proto2 import/public-import fixtures without modifying them. The full older upstream conformance `unittest.proto` is an input-parser preservation fixture; its `unverified_lazy` extensions are rejected by the pinned official protoc 34.2 and are not claimed as Go output support.

The provider also publishes `Blocks<RpcMethodBlock>` automatically alongside `RpcContract` and `ProtobufDocument`. Its source identity is the canonical input path with normalized separators, and local IDs are fully qualified method coordinates. Call `RpcContract::method_blocks` explicitly when an application needs its own stable document identity. These blocks retain service and source-file context without inventing shared message shapes.

Public provider APIs live in `poolster_input_protobuf::contracts` and `poolster_input_protobuf::blocks`. These reexport the shared owned definitions, preserving existing graph identities:

```rust
use poolster_core::input::InputPlugin;
use poolster_input_protobuf::{ProtobufInput, contracts::RpcContract, blocks::RpcMethodBlocks};
let input = ProtobufInput.load(std::path::Path::new("service.proto"))?;
let rpc = input.get::<RpcContract>()?;
let methods = input.get::<RpcMethodBlocks>()?;
// Consumers require either exact contract through the typed graph.
```

For an application-owned source identity, use `blocks::method_blocks(rpc, "stable.document.id")`. Whole contracts and parser-native documents remain available independently.
