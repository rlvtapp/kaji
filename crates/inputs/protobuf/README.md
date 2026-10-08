# kaji-input-protobuf

Native `protobuf` input provider. Register `ProtobufInput` directly with
`kaji_core::input::InputRegistry`, or enable `protobuf` in `kaji-inputs`.
The provider publishes `ProtobufDocument` for typed consumers in Kaji's plugin graph.

The parser supports proto2 and proto3, including imports, standard well-known
schemas, nested messages/enums, field defaults, oneofs and RPC streaming.
Protobuf editions are currently unsupported by protox 0.9. The pinned v29.3
upstream corpus verifies that this limitation returns an explicit diagnostic.

`load_with_includes(path, includes)` accepts explicit import roots for repository
layouts such as `google/protobuf/unittest.proto`. `load(path)` searches the root
file's directory. The CLI provider currently uses that default directory search.

Tests retain the official protobuf v21.12 conformance schema and its import
closure: 3 source files, 51,334 bytes, 135 summary types and 2 service methods.
The editions negative corpus uses v29.3: 3 source files, 63,634 bytes.
Each corpus includes its upstream license, immutable commit, and per-file
SHA-256 digests in `tests/fixtures/*/PROVENANCE.json`. Tests run offline.
