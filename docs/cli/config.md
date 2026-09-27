# `kaji.json` recipes

`kaji.json` is a committed generation recipe. It records the OpenAPI source,
the output root, and every SDK or artifact package to emit. Paths are resolved
from the recipe directory, not the shell's current directory.

## A practical starting point

```json
{
  "$schema": "https://raw.githubusercontent.com/rlvtapp/kaji/main/schemas/v1/kaji.schema.json",
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

## Package design

Packages are independent. One contract can safely produce a Fetch SDK, Axios
SDK, Go SDK, mock service, and documentation artifact in one run. Give each
package a separate safe relative `path`; `name` is its ecosystem identity.

| Package language | Plugins |
| --- | --- |
| `typescript` | `sdk`, `zod`, TanStack, SWR, Faker, MSW, Cypress |
| `rust`, `go`, `python`, `php`, `java`, `dotnet`, `elixir` | `sdk` |
| `mock` | `server` |
| `artifacts` | `redoc`, `mcp` |

For private remote contracts, use environment references rather than committed
tokens. The full [`kaji.json` reference](../config-file.md) covers every field,
remote inputs, download limits, and plugin-specific options.
