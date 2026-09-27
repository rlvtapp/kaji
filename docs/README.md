# Kaji documentation

Kaji supports two generation workflows. Both use the same compiler and SDK
renderers; choose the one that fits where generation belongs.

| If you want to… | Start here |
| --- | --- |
| Generate from a repository, script, or CI job | [CLI workflow](cli/README.md) |
| Embed generation in a Rust application | [Rust library workflow](library/README.md) |
| Understand generated code | [Generated SDK guide](generated-sdks.md) |
| Add helpers, mocks, or documentation artifacts | [Auxiliary generators](auxiliary-generators.md) |

## Common journeys

- **First SDK:** [CLI quickstart](cli/quickstart.md).
- **Committed recipe:** [`kaji.json` guide](cli/config.md).
- **Several packages from one contract:** [CLI recipes](cli/recipes.md).
- **Zod, TanStack, SWR, Faker, MSW, and Cypress:** [TypeScript helpers](guides/typescript-helpers.md).
- **Mock server and test layers:** [testing generated SDKs](guides/testing.md).
- **ReDoc and MCP output:** [generated artifacts](guides/artifacts.md).
- **Automation:** [CI integration](ci-integration.md).
- **Custom generation:** [library quickstart](library/quickstart.md) and
  [plugin composition](library/plugins.md).

See [examples](../examples/README.md) for copyable projects, from a minimal CLI
recipe to a complete frontend stack and an embedded Rust application.
