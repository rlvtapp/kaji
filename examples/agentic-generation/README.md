# Agentic generation

This is the smallest trusted-workspace example for Kaji's generator MCP
server. It lets an agent ask Kaji which languages it supports and generate an
SDK from an explicit local OpenAPI file.

## Start the MCP server

Use `mcp-server.json` as the command fragment for your MCP host, or start it
directly while testing the stdio protocol:

```sh
cd examples/agentic-generation
npx @relevate/kaji mcp generator
```

The server exposes only two tools:

| Tool | Purpose |
| --- | --- |
| `kaji_languages` | Lists maintained SDK targets. |
| `kaji_generate` | Generates one or more SDKs from a local OpenAPI file. |

An MCP host should call `kaji_generate` with paths that it is allowed to read
and write. A representative tool input is:

```json
{
  "source": "/absolute/path/to/examples/agentic-generation/openapi.yaml",
  "output": "/absolute/path/to/examples/agentic-generation/generated",
  "languages": ["typescript", "csharp"],
  "name": "Tasks",
  "sdkVersion": "1.0.0",
  "clientStyle": "namespaced",
  "typescriptTransport": "fetch"
}
```

The output path is deliberate: generation overwrites Kaji-owned files below
it while preserving custom starter files. The generated output includes
`.kaji/generation.lock.json`, so an agent can report the selected operations,
targets, input hash, and Kaji version for review.

## Safe operating boundary

Run this only for a trusted workspace. The generator server accepts local file
paths and writes the requested output; it does not fetch contracts, infer API
credentials, or expose arbitrary shell execution. For an agent that should
call an API instead, use the separate [MCP API tools example](../mcp-api-tools/README.md)
against the native [mock scenarios example](../mock-scenarios/README.md).

See the [MCP server guide](../../docs/mcp-server.md) for protocol and
authentication details.
