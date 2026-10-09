# Write JavaScript plugins

A plugin is a factory returning a name and handlers. It can emit files, transform
an HTTP API, or exchange data with other JavaScript plugins.

1. [Output plugin](output.md) — emit a file in a few lines.
2. [Input plugin](input.md) — load your own source format.
3. [Contracts and hooks](contracts.md) — share data and choose when to run.

These are JavaScript APIs. Native Rust contract/block handlers are a separate
[Rust plugin interface](../rust/README.md).

[All sections](../../README.md)
