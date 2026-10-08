# CLI recipes

## One contract, many packages

```json
{
  "openapi": { "input": "./openapi.yaml", "name": "Billing", "version": "1.0.0" },
  "output": { "path": "./generated" },
  "packages": [
    { "language": "typescript", "path": "typescript/fetch", "plugins": [{ "name": "sdk", "transport": "fetch" }] },
    { "language": "typescript", "path": "typescript/axios", "plugins": [{ "name": "sdk", "transport": "axios" }] },
    { "language": "go", "path": "go", "plugins": [{ "name": "sdk", "jobs": 4 }] },
    { "language": "mock", "path": "mock-server", "plugins": [{ "name": "server", "port": 4010 }] }
  ]
}
```

Run `poolster generate` once. Fetch and Axios remain separate packages; place
framework helpers alongside the transport they import. Copy the complete
[multi-package example](../../examples/cli-multi-package/README.md).

## Private remote contract

```json
{
  "openapi": {
    "input": {
      "url": "https://partner.example.com/openapi.json",
      "headers": { "X-Organization": "acme" },
      "auth": { "type": "bearer", "token": { "env": "PARTNER_OPENAPI_TOKEN" } }
    }
  }
}
```

Set the token in your shell or CI secret store, never in JSON:

```sh
PARTNER_OPENAPI_TOKEN=… npx poolster generate
```

## Compile once, render repeatedly

For local package-configuration experiments, retain compiler artifacts and set
`openapi.artifacts` in your recipe. Artifacts are internal, versioned output;
regenerate them when the contract or Poolster version changes.


## Native GraphQL recipes

Use top-level `input` instead of `openapi` to select a native source and provider.
GraphQL operation documents are configured through `input.options.operation_files`;
paths resolve relative to the recipe. Select the TypeScript `graphql` plugin.
Unsupported format/output pairs warn, skip and preserve existing skipped outputs.
Malformed sources and unsupported options for an otherwise supported pipeline fail.

See the [native generation matrix and recipe](../native-pipelines.md) and
[runnable local GraphQL example](../../examples/graphql-native/README.md).
The existing OpenAPI recipe shape and behavior are unchanged.
