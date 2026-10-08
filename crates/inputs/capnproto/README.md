# kaji-input-capnproto

Native `capnproto` input provider. Register `CapnProtoInput` directly with
`kaji_core::input::InputRegistry`, or enable `capnproto` in `kaji-inputs`.
The provider publishes `CapnProtoDocument` for typed consumers in Kaji's plugin graph.

Source inspection requires the official `capnp` compiler on PATH. Use
`load_with_includes(path, includes)` to resolve repository-wide absolute imports.
`CapnProtoDocument::from_schema_request(bytes, title)` loads an official binary
CodeGeneratorRequest directly. Both preserve native descriptor bytes; summaries
distinguish ordinary capability RPC from streaming methods using the standard
StreamResult schema identity.

The pinned official v1.1.0 corpus includes `test.capnp`, `schema.capnp`, `rpc.capnp`
and `rpc-twoparty.capnp`, their imports and embedded data: 9 source/data files,
158,056 bytes. Its retained 320,016-byte descriptor request contains 277 summary
types and 49 methods and is tested offline without an installed compiler.
Source compilation is also tested whenever `capnp` is available; otherwise that
external check prints an explicit skip message.

`tests/fixtures/upstream/PROVENANCE.json` records the immutable upstream commit,
license, file SHA-256 digests, compiler version, compiler bottle checksum, and
reproduction command. The retained request was produced with Cap'n Proto 1.5.0.
