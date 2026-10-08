# `poolster.json` recipes

`poolster.json` is a committed generation recipe. It records the OpenAPI source,
the output root, and every SDK or artifact package to emit. Paths are resolved
from the recipe directory, not the shell's current directory.

## A practical starting point

```json
{
  "$schema": "https://raw.githubusercontent.com/rlvtapp/kaji/main/schemas/v1/poolster.schema.json",
  "openapi": { "input": "./openapi.yaml", "name": "Pet Store", "version": "1.0.0" },
  "output": { "path": "./generated" },
  "packages": [
    {
      "language": "typescript",
      "path": "typescript",
      "name": "@acme/pet-store",
      "plugins": [
        { "name": "sdk", "transport": "fetch", "client_name": "PetStore" },
        { "name": "zod", "output": "validation" }
      ]
    }
  ]
}
```

The schema provides completion and inline validation in schema-aware editors.

## Focus a large contract

`openapi.paths` slices the contract before Poolster renders any package, mock, or
artifact. `include` is an OR-set; `exclude` always wins. Patterns start with
`/`; `*` matches any sequence (including `/`) and `?` matches one character.

```json
{
  "openapi": {
    "input": "./openapi.yaml",
    "paths": {
      "include": ["/messages*", "/admin/users*"],
      "exclude": ["/admin/users/audit*"]
    }
  }
}
```

Poolster fails rather than writing an empty surface when the selection matches no
operation. The output's `.poolster/generation.lock.json` records the selected
paths and final operation inventory, so a generated API slice is reviewable.

## Package design

Packages are independent. One contract can safely produce a Fetch SDK, Axios
SDK, Go SDK, mock service, and documentation artifact in one run. Give each
package a separate safe relative `path`; `name` is its ecosystem identity.

| Package language | Plugins |
| --- | --- |
| `typescript` | `sdk`, `zod`, TanStack, SWR, Faker, MSW, Cypress |
| `typescript-cli` | `cli` |
| `rust-cli` | `cli` |
| `rust`, `go`, `python`, `php`, `java`, `csharp`, `elixir`, `ruby`, `swift` | `sdk` |
| `mock` | `server` |
| `artifacts` | `redoc`, `mcp` |

`csharp` emits a .NET 8 C# SDK using `HttpClient` and `System.Text.Json`.
`dotnet` is retained as a compatibility alias for existing recipes.

`ruby` emits a Ruby 3.1+ gem using only `Net::HTTP`, `URI`, and `JSON` from
the standard library.

`swift` emits a Swift 5.9+ Swift Package Manager package using Foundation
`URLSession` and `Codable`, without third-party runtime dependencies.

For private remote contracts, use environment references rather than committed
tokens. The full [`poolster.json` reference](../config-file.md) covers every field,
remote inputs, download limits, and plugin-specific options.

## Bundle idempotency keys

A package `idempotency` field resolves supported operations, optional caller keys,
and automatic UUID generation. See [the idempotency guide](../guides/idempotency.md)
for OpenAPI extensions, recipe precedence, server requirements, and runtime limits.
