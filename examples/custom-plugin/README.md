# Author a typed plugin

This standalone Rust generator depends only on `kaji-core`. Run:

```sh
cargo test --manifest-path examples/custom-plugin/Cargo.toml
cargo run --manifest-path examples/custom-plugin/Cargo.toml -- /tmp/kaji-plugin-example
```

`Names` is a typed contract. Two independent providers publish it. The consumer
binds the replacement's exact handle, so declaration order and the other provider
do not affect selection. It emits `operations.txt`; the generic API reference
plugin independently emits `API_REFERENCE.md`. The test verifies the selected
provider actually supplies the consumer's output.

The custom language has no SDK transport or publisher. Use the same engine with
an existing language contract when replacing an SDK provider. A recipe JSON
plugin name cannot load arbitrary Rust code: embed and compile the generator.
See [typed plugin authoring](../../docs/typed-plugins.md) for the full boundaries.
