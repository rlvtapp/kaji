# Kaji

A native command-line tool for generating typed SDKs from OpenAPI.

One specification. Multiple languages. Ready-to-build packages.

Generate **TypeScript, Rust, Go, Python, PHP, Java, .NET, and Elixir** SDKs
from a Swagger 2.0 or OpenAPI 3.0/3.1 document. Choose Fetch or Axios for TypeScript, raw
operation functions or a full client, and generate several languages in one run.

## Quick start

Kaji is pre-1.0. Install or run it through the published npm facade; it downloads
the platform-native generator and bundled OpenAPI compiler automatically:

```sh
npx @relevate/kaji init --input ./openapi.yaml --output ./generated --name "Email" --sdk-version 1.0.0
# edit kaji.json, then:
npx @relevate/kaji generate
```

`npx @relevate/kaji init` creates a JSON recipe with its OpenAPI source, output root, SDK
packages and plugins. Add as many independently configured packages as you
need, then run `npx @relevate/kaji generate` again whenever the contract changes.

`@relevate/kaji` is a small Node launcher for bundled native executables;
installed npm users do not need Rust or Go.
The generator itself is Rust, with a bundled Go OpenAPI compiler.

## Common commands

```sh
# See available SDK targets
npx @relevate/kaji languages

# Start or use an explicit JSON recipe
npx @relevate/kaji init --input openapi.yaml
npx @relevate/kaji generate --config ./kaji.json

# Generate every SDK language (TypeScript uses Fetch by default)
npx @relevate/kaji generate openapi.yaml --output ./generated --language all

# Generate TypeScript operation functions without an SDK class
npx @relevate/kaji generate openapi.yaml --output ./generated \
  --language typescript --typescript-surface raw

# Generate a flat client instead of resource namespaces
npx @relevate/kaji generate openapi.yaml --output ./generated \
  --language go --client-style flat
```

The JSON recipe is the recommended route. Direct command-line generation stays
useful for one-off output and CI experiments. All options and every built-in
config plugin are documented in the [CLI guide](docs/cli.md).

Both modes accept a local OpenAPI file or an HTTPS URL. For example:

```sh
npx @relevate/kaji generate https://aka.ms/graph/v1.0/openapi.yaml \
  --output ./graph-sdk --language go --name "Microsoft Graph"
```

## What you get

A generated TypeScript client can look like this:

```ts
const client = new Email({
  baseUrl: "https://api.example.com",
  apiKey,
});

const contact = await client.contacts.get({
  path: { contactId: "contact_123" },
}).unwrap();
```

Names and parameters come from your API. Each generated package includes
its own usage guide and build metadata.

## Choose what you ship

- **Full SDKs:** configured clients, typed requests/responses, resource namespaces,
  declared errors, and contract-driven runtime features.
- **Direct operations:** select a flat client, or TypeScript raw functions with
  `--typescript-surface raw`.
- **Fetch or Axios:** separate TypeScript packages with the same source contract.
- **Large Go APIs:** split model/operation files and bounded rendering workers.
- **Validation and frontend helpers:** add Zod, TanStack React/Vue Query, SWR,
  Faker, MSW, and Cypress beside a TypeScript package through `kaji.json`.
- **Documentation artifacts:** add ReDoc or an MCP tool manifest through the
  same recipe.
- **Contract mocks:** add an optional `httpmock` Docker package usable by every
  generated SDK.
- **Custom generators:** language-scoped Rust plugins and typed dependencies,
  without a JavaScript generation runtime.

Supported auth, pagination, retries, streaming and file handling depend on the
contract and target. Read the [generated SDK guide](docs/generated-sdks.md)
before choosing a runtime integration; this is not a promise of identical
features or API spelling in every language.

## Rust interface

For embedding Kaji or writing custom plugins, a typed Rust interface is also
available. See the [Rust API guide](docs/getting-started.md) and
[plugin authoring reference](docs/typed-plugins.md).

## Documentation

| Guide | What you will learn |
| --- | --- |
| [CLI](docs/cli.md) | Build/run the CLI, select languages, and use every command-line option. |
| [`kaji.json`](docs/config-file.md) | Config-first versus direct generation, every JSON field, package, and plugin. |
| [Rust interface](docs/getting-started.md) | Embed generation or configure packages programmatically. |
| [Configuration](docs/configuration.md) | All public package, plugin, shared, and model options. |
| [Generated SDKs](docs/generated-sdks.md) | Raw versus full clients, language requirements, and runtime behavior. |
| [CI integration](docs/ci-integration.md) | GitHub Actions and GitLab CI templates for generation. |
| [Auxiliary generators](docs/auxiliary-generators.md) | Zod, TanStack, SWR, fixtures, mocks, and documentation artifacts. |
| [Contract mocking](docs/mocking.md) | Run local mocks and declare conditional responses. |
| [Large specs](docs/large-specs.md) | Go file splitting, parallelism, and Microsoft Graph testing. |
| [Plugin authoring](docs/typed-plugins.md) | Add a language or consume another plugin's typed output. |
| [Contributing](docs/contributing.md) | Repository layout, development setup, and verification. |

## Runnable examples

- [Complete TypeScript stack](examples/typescript-stack/README.md): Fetch and
  Axios SDKs, plus Zod, TanStack React/Vue Query, SWR, Faker, MSW, Cypress, and
  the shared HTTP mock server from one small OpenAPI contract.
- [Microsoft Graph](examples/microsoft-graph/README.md): a URL-backed,
  large-contract Fetch and Go generation demo.

## License

Licensed under the [MIT License](LICENSE).
