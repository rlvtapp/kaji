# Generated documentation and MCP tools

An `artifacts` package emits ReDoc and MCP metadata beside your SDKs.

## Add an artifacts package

```json
{
  "language": "artifacts",
  "path": "docs",
  "plugins": [
    { "name": "redoc", "openapi_spec": "../openapi.yaml", "title": "Pet Store API" },
    { "name": "mcp", "output": "mcp" }
  ]
}
```

## Understand the output

ReDoc creates an HTML entry point that loads ReDoc from a CDN; it is not an offline
static-site bundle. The MCP plugin creates operation metadata only. It does not start a
server, handle credentials, or provide transport.

## Run an MCP server

For a runnable stdio server that calls an API origin, use:

```sh
poolster mcp ./openapi.yaml --base-url https://api.example.com
```

`poolster mcp generator` exposes generation controls to a trusted MCP host and can write
files. Read [the MCP guide](../mcp-server.md) for authentication, inputs, and safety.
The [multi-package example](../../examples/cli-multi-package/README.md) emits both ReDoc
and an MCP manifest.
