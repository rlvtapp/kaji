# Deprecated Poolster .NET plugin facade

`poolster-plugin-dotnet` is retained only for source compatibility. It re-exports
the canonical [`poolster-plugin-csharp`](../csharp/README.md) crate with no separate
generator implementation.

New integrations should depend on `poolster-plugin-csharp` and use
`poolster::csharp` / the `csharp` target. Existing `dotnet` CLI and configuration
selectors remain accepted as compatibility aliases.
