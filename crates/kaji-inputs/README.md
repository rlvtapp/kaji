# Kaji input plugins

Native input providers for GraphQL, AsyncAPI, Arazzo, Protobuf and Cap'n Proto.
Providers register through `kaji_core::input::InputRegistry` and publish native
typed contracts to generator consumers. Each provider lives in its own `kaji-input-*` crate under `crates/inputs/`.
This crate reexports them and builds a default registry. Features select
which provider crates are linked; all five are enabled by default.

See [input plugins](../../docs/input-plugins.md) for CLI commands, replacement
providers, typed graph integration, supported versions and verification limits.
