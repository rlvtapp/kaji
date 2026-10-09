# Write Rust plugins

Use the typed dependency graph to read inputs, publish contracts and emit files.

1. [Contract and block handlers](handlers.md) — the smallest plugin.
2. [Custom contracts](contracts.md) — producers, consumers and transformations.
3. [Input and output interfaces](interfaces.md) — implement a provider or language plugin.

Rust plugins are linked crates. The prebuilt CLI and Node addon expose registered
plugins; installing a new crate does not dynamically load it into those binaries.

[All sections](../../README.md)
