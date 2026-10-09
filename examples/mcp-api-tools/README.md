# MCP API tools example

Expose this small OpenAPI contract as a stdio MCP server:

```sh
cd examples/mcp-api-tools
npx poolster mcp ./openapi.yaml --base-url http://localhost:4010
```

Configure that command as a stdio server in an MCP host. It publishes tools for
each operation and forwards calls to the supplied origin. Run it against the
[mock scenarios example](../mock-scenarios/README.md) to evaluate tools without
a production service.

The server never infers credentials from the document. Supply request headers
or cookies per MCP tool call, or use a credential-injecting proxy. The detailed
protocol, supported body encodings, and safety notes are in the
[MCP server guide](../../docs/reference/automation/mcp-server.md).
