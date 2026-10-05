# Kaji documentation

Release notes: [Kaji 0.4.0](releases/0.4.0.md).
Previous release: [Kaji 0.3.0](releases/0.3.0.md).

Kaji supports two generation workflows. Both use the same compiler and SDK
renderers; choose the one that fits where generation belongs.

| If you want to… | Start here |
| --- | --- |
| Understand why Relevate built Kaji | [Why Relevate built Kaji](why-kaji.md) |
| Generate from a repository, script, or CI job | [CLI workflow](cli/README.md) |
| Embed generation in a Rust application | [Rust library workflow](library/README.md) |
| Understand generated code | [Generated SDK guide](generated-sdks.md) |
| Add helpers, mocks, or documentation artifacts | [Auxiliary generators](auxiliary-generators.md) |

## Common journeys

- **First SDK:** [CLI quickstart](cli/quickstart.md).
- **Find a public contract:** [OpenAPI discovery and download](discovery.md).
- **Committed recipe:** [`kaji.json` guide](cli/config.md).
- **Several packages from one contract:** [CLI recipes](cli/recipes.md).
- **Zod, TanStack, SWR, Faker, MSW, and Cypress:** [TypeScript helpers](guides/typescript-helpers.md).
- **A Node.js API command-line client:** [TypeScript API CLI](typescript-cli.md).
- **A distributable native API CLI:** [Rust API CLI](rust-cli.md).
- **Native mock API, scenarios, and Docker fixtures:** [contract mocking](mocking.md).
- **Mock server and test layers:** [testing generated SDKs](guides/testing.md).
- **ReDoc and MCP output:** [generated artifacts](guides/artifacts.md).
- **Automation:** [CI integration](ci-integration.md).
- **Custom generation:** [library quickstart](library/quickstart.md) and
  [plugin composition](library/plugins.md).

See [examples](../examples/README.md) for copyable projects, from a minimal CLI
recipe to a complete frontend stack and an embedded Rust application.
