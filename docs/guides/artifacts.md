# Generated documentation and MCP tools

An `artifacts` package can generate ReDoc and MCP metadata beside SDKs.

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

ReDoc creates an HTML entry point that loads ReDoc from a CDN; it is not an
offline static-site bundle. The MCP plugin creates operation metadata only. It
does not start a server, handle credentials, or provide transport.

For a runnable stdio server that calls an API origin, use:

```sh
kaji mcp ./openapi.yaml --base-url https://api.example.com
```

`kaji mcp generator` exposes generation controls to a trusted MCP host and can
write files. Read [the MCP guide](../mcp-server.md) for authentication, inputs,
and safety. The [multi-package example](../../examples/cli-multi-package/README.md)
emits both ReDoc and an MCP manifest.
