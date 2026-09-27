# Kaji examples

Every example keeps its contract, recipe or application code, and instructions
together. Start small, then move to the complete stacks.

| Example | Shows | Best for |
| --- | --- | --- |
| [CLI basic](cli-basic/README.md) | One config-first TypeScript SDK | First generation |
| [CLI multi-package](cli-multi-package/README.md) | TypeScript, Go, docs, and a mock from one contract | Team and CI setup |
| [Mock scenarios](mock-scenarios/README.md) | Docker HTTP mock plus conditional contract responses | SDK integration tests |
| [React Query consumer](react-query-consumer/README.md) | Generated Fetch SDK and TanStack React Query hooks | Frontend integration |
| [MCP API tools](mcp-api-tools/README.md) | OpenAPI operations exposed through a stdio MCP server | Agent/API evaluation |
| [Rust embedded](rust-embedded/README.md) | Calling Kaji from a Rust application | Integrators and plugin authors |
| [TypeScript stack](typescript-stack/README.md) | Transports, validation, hooks, and testing helpers | Frontend consumers |
| [Microsoft Graph](microsoft-graph/README.md) | Remote large contract and Go scaling | Large specifications |

The small examples do not commit their generated output. Run the documented
command, inspect the result, and decide whether generated SDKs belong in your
own source control.
