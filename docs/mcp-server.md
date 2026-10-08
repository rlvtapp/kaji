# MCP server

[Artifact recipes](guides/artifacts.md) · [CLI commands](cli/commands.md)

`kaji mcp` turns an OpenAPI document into a stdio [Model Context Protocol](https://modelcontextprotocol.io/) server. It exposes each operation as a tool and sends calls to the API origin you choose.

```sh
kaji mcp ./openapi.yaml --base-url https://api.example.com
```

The command compiles the document once at startup, then speaks newline-delimited JSON-RPC on standard input/output. Configure it as a stdio server in an MCP host; do not run it in a terminal that writes anything else to standard output.

Each tool accepts grouped inputs that follow the generated SDK shape:

```json
{
  "path": { "petId": "pet_123" },
  "query": { "include": ["owner", "vaccinations"] },
  "headers": { "authorization": "Bearer …" },
  "cookies": { "session": "…" },
  "contentType": "application/json",
  "body": { "name": "Miso" }
}
```

`path`, `query`, `headers`, and `cookies` appear only when an operation declares them. Their JSON Schemas are derived from the OpenAPI contract, including required object fields, enums, formats, and composed schemas. Required parameters and bodies are checked before a request is sent.

Set `contentType` when an operation declares more than one request media type. JSON, `application/x-www-form-urlencoded`, `multipart/form-data` (text fields), and raw string bodies are handled directly. Responses are returned as MCP text content; JSON is formatted for readability and is limited to 1 MiB.

## Safety and authentication

Kaji does not infer credentials or read them from the OpenAPI document. Supply them per call in `headers` or `cookies`, or place a credential-injecting proxy at `--base-url`. Treat every tool that maps to a mutating HTTP method as production-capable: use a test API origin while evaluating agents.

MCP calls support ordinary scalar and repeated query values. The generated SDK runtime offers the fuller OpenAPI parameter-serialization and codec surface.

## Generator-control server

Run `kaji mcp generator` when an MCP host needs to operate Kaji itself rather
than call an API described by a spec. It exposes `kaji_languages` and
`kaji_generate`; the latter accepts an explicit local source path, output path,
and selected language targets. It invokes the same CLI generation command and
therefore overwrites generated output under the path the MCP caller supplies.

```sh
kaji mcp generator
```

Keep this server scoped to a trusted workspace: `kaji_generate` writes SDK
output, just like running `kaji generate` directly.

The [agentic generation example](../examples/agentic-generation/README.md)
includes a copyable MCP command fragment, a minimal contract, representative
tool input, and the recommended workspace boundary.

## Author workflow and supported subset

Keep the API-calling server and generator-control server as separate MCP entries.
Use a small local contract/mock origin first: list tools, invoke a read operation,
then invoke a reviewed mutation and inspect the local request log. For an API
whose authentication needs signing or refresh, use an author-controlled proxy;
the MCP server does not execute SDK middleware or OAuth acquisition code.

The API-calling protocol is newline-delimited JSON-RPC over stdio. It is not an
HTTP/SSE MCP deployment or a hosted credential manager. Request media and ordinary
parameter support are described above; do not assume generated SDK runtime hooks,
retry settings, pagination iterators, streaming response handling or language
customizations apply to these tool calls. Verify the specific operation/media
combination against a local server before giving an agent a mutating production
tool. Generator-control writes require an explicitly trusted workspace and keep
normal generation diagnostics/ownership checks.
