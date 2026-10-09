# Write Rust plugins

Use the typed dependency graph to read inputs, publish contracts and emit files.

1. [Complete output tutorial](output.md) — publish data and generate your first file.
2. [Contract and block handlers](handlers.md) — consume native protocol data.
3. [Custom contracts](contracts.md) — producers, consumers and transformations.
4. [Input and output interfaces](interfaces.md) — implement a provider or language plugin.

Rust plugins are linked crates. The prebuilt CLI and Node addon expose registered
plugins; installing a new crate does not dynamically load it into those binaries.

[All sections](../../README.md)
