# Deprecated Kaji .NET plugin facade

`kaji-plugin-dotnet` is retained only for source compatibility. It re-exports
the canonical [`kaji-plugin-csharp`](../csharp/README.md) crate with no separate
generator implementation.

New integrations should depend on `kaji-plugin-csharp` and use
`kaji::csharp` / the `csharp` target. Existing `dotnet` CLI and configuration
selectors remain accepted as compatibility aliases.
