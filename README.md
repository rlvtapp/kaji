<h1><img src="docs/assets/banner.svg" alt="Poolster: one contract, lots of possibilities, your code" width="100%"></h1>

> [!IMPORTANT]
> **Poolster is under active development. OpenAPI generation is ready to use; GraphQL and other native pipelines are expanding.**
> See the [support matrix](docs/plugin-support-matrix.md) for tested outputs and limitations. Current GraphQL additions are unreleased.

**One contract. Lots of possibilities. Your code.**

Poolster turns API contracts into clients, models, hooks and tools. Choose input
and output plugins, then regenerate when your contract changes.

## Quick start

```sh
npm install -D @relevate/poolster @relevate/poolster-plugin-typescript
```

```js
import { generate } from '@relevate/poolster';
import { pluginTypeScript } from '@relevate/poolster-plugin-typescript';

await generate({
  name: 'Example API',
  version: '1.0.0',
  input: './openapi.yaml',
  output: './generated',
  plugins: [pluginTypeScript({ path: 'client' })],
});
```

[JavaScript quickstart](docs/javascript/quickstart.md) · [Rust SDK quickstart](docs/rust/quickstart.md)

## Documentation

| I want to… | Start here |
| --- | --- |
| Use JavaScript | [JavaScript](docs/javascript/README.md) |
| Use the Rust SDK | [Rust SDK](docs/rust/README.md) |
| Write JavaScript plugins | [JavaScript plugins](docs/plugins/javascript/README.md) |
| Write Rust plugins | [Rust plugins](docs/plugins/rust/README.md) |
| Learn about Poolster | [About](docs/about/README.md) |
| Understand contracts, blocks and lifecycles | [Internals](docs/internals/README.md) |

## Supported outputs

OpenAPI supports the established HTTP SDKs and helpers. GraphQL supports
clients for all ten SDK languages and TypeScript query, validation and testing companions
in this checkout. RPC, events and workflows have their own supported pipelines.

[Plugin support matrix](docs/plugin-support-matrix.md) · [Examples](examples/README.md)

<a id="build-your-own-plugin"></a>

## Extend Poolster

Inputs publish contracts. Outputs consume them. Your plugins can provide whole
contracts, expose optional building blocks and insert transformations.

[JavaScript plugin tutorial](docs/plugins/javascript/output.md) ·
[Rust plugin tutorial](docs/plugins/rust/handlers.md)

<a id="ask-ai-about-poolster"></a>

## Project

[Contributing](CONTRIBUTING.md) · [Changelog](CHANGELOG.md) · [AI context](docs/about/ai.md) · [License](LICENSE)
