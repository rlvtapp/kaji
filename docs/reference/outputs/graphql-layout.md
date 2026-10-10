# GraphQL package layouts

GraphQL clients use the existing language output plugins. A schema and operation
documents produce selection-specific models, query/mutation functions and transport
code. Choose raw functions, flat client methods or grouped methods; the layout
does not change which fields the supplied documents select.

The [generated SDK specification](../../specifications/generated-sdk.md) defines
target layout, formatting and style requirements for all languages. This page
describes the implemented GraphQL layouts.

## Where generated code lives

| Language | Layout | Guide |
| --- | --- | --- |
| TypeScript | Public barrels; `graphql/models/`, `operations/`, `client/`; separate runtime | [TypeScript](graphql-typescript.md) |
| Rust | Public `graphql` module includes model, operation and client parts; separate runtime | [Rust](graphql-rust.md) |
| Go | Model, operation, group and transport files in one Go package | [Go](graphql-go.md) |
| Python | Model and operation packages; client/group mixins; separate runtime | [Python](graphql-python.md) |
| PHP | Composer loader; models, operations and method traits; separate runtime/client | [PHP](graphql-php.md) |
| Symfony | PHP SDK layout plus separate bundle, HttpClient transport and DI files | [Symfony](graphql-symfony.md) |
| Ruby | Model, operation and group Ruby/RBS files; loader and runtime | [Ruby](graphql-ruby.md) |
| Java | Thin client; `models`, `operations`, `groups`; runtime | [Java](graphql-java.md) |
| C# / DotNet | `Models/`, `Operations/`, `Groups/`, `Client/`, `Runtime/`; partial declarations | [C#](graphql-csharp.md) |
| Swift | Model, operation, group and runtime files under the generated module | [Swift](graphql-swift.md) |
| Elixir | Model, operation, client and runtime modules; partitioned facade exports | [Elixir](graphql-elixir.md) |

## Size and regeneration

Collections of declarations, exports and client helpers are split at semantic
boundaries. The source budget is **128 KiB per file**. An indivisible declaration
that exceeds it is retained and reported in
`.poolster/source-layout-diagnostics.json`; valid input is not truncated.
Stable filenames and ordering keep operation-document reordering from causing
unnecessary changes. Regeneration removes obsolete unchanged owned files and
preserves unrelated user files.

## Migrating existing output

Existing call paths stay available. Java model imports move from nested
`Client.ReadUserVariables` records to `<package>.models.ReadUserVariables`.
Generated-source customizations targeting former monolithic files need new paths;
review ownership conflicts before replacing edited generated files.

These layouts are unreleased checkout changes; published alpha.1 packages do not
include them. Source organization does not expand protocol support: subscriptions,
abstract selections and scalar support still have language-specific boundaries.
See the [support matrix](../../plugin-support-matrix.md#graphql-language-boundaries)
and [test record](../../verification-results/graphql-source-layout-2026-10-09.json).

[Back to reference](../README.md)
