# Kaji

## Generate your API's entire developer ecosystem from one contract.

Kaji turns a Swagger 2.0 or OpenAPI 3.0/3.1 document into SDK source,
API CLIs, mocks, validation, framework integrations, documentation, and MCP
tools. Generate for TypeScript, Rust, Go, Python, PHP, Java, C#/.NET, Elixir,
Ruby, and Swift from the same contract, with Symfony integration packages.

```text
OpenAPI
  │
  ▼
Kaji
  ├── SDKs          TypeScript · Go · Python · Rust · Java · C#/.NET · PHP · Elixir · Ruby · Swift
  ├── Integrations  Symfony (wraps the generated PHP SDK)
  ├── API CLIs      TypeScript (Node.js) · Rust (native)
  ├── Clients       Fetch · Axios
  ├── Frontend      TanStack React Query · Vue Query · SWR
  ├── Schema        Zod · Faker
  ├── Testing       MSW · Cypress · HTTP mocks
  ├── Docs          ReDoc
  └── AI            MCP
```

The goal is not just to produce an SDK. It is to keep every developer-facing
surface of an API aligned with the contract that defines it. Read [why Relevate
built Kaji](docs/why-kaji.md) for the problem that made us build it and what we
are committing to keep free.

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

## For SDK authors

Start with [the SDK-author documentation](docs/README.md). The complete workflow
is to generate a package, ship your own policies, test the generated code, then
review SDK and release pull requests before publication.

| Next step | Guide |
| --- | --- |
| Generate and understand your first SDK | [CLI quickstart](docs/cli/quickstart.md) |
| Bundle middleware that customers do not have to register | [Executable author example](examples/bundled-middleware/README.md) |
| Customize one language package | [SDK customization](docs/sdk-customization.md) |
| Preserve custom work when the contract changes | [Safe regeneration](docs/safe-regeneration.md) |
| Generate CI, sync, and release workflows | [SDK repository automation](docs/sdk-automation.md) |
| Export Postman collections and typed Terraform providers | [API artifacts example](examples/api-artifacts/README.md) |
| Publish checked release tags | [SDK publishing](docs/sdk-publishing.md) |

The newer middleware and delivery APIs are available in this source tree. Use
the [source build](docs/source-customization.md) until your chosen published
launcher includes them. Workflow scaffolding does not provision a GitHub App or
registry account; the delivery guides explain that setup and its verification.

Python users can install the same native CLI through pip:

```sh
python -m pip install kaji-cli
kaji init --input ./openapi.yaml --output ./generated
kaji generate
```

`kaji-cli` includes the executable for its platform; Python is only the console
entry point. Wheels currently support macOS ARM64/x64, Linux x64 with glibc 2.35+
and Windows x64.

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
});
```

Names and parameters come from your API. Each generated package includes
its own usage guide and build metadata. TypeScript operations resolve to the
decoded success body by default.

A generated CLI follows the same contract, but turns operations into commands:

```sh
email messages send --from hello@example.com --to customer@example.com \
  --subject "Welcome"
email admin users list
```

Its `auth` group supports OAuth login, OpenAPI API keys, named credential
profiles, and environment variables for CI. See the [TypeScript API CLI](docs/typescript-cli.md)
and [Rust API CLI](docs/rust-cli.md) guides.

## Choose what you ship

- **Full SDKs:** configured clients, typed requests/responses, resource namespaces,
  declared errors, and contract-driven runtime features.
- **Direct operations:** select a flat client, or TypeScript raw functions with
  `--typescript-surface raw`.
- **API CLIs:** generate a publishable Node.js or native Rust executable from
  the same paths, parameters, request bodies, and security requirements.
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

SDK customers can customize transport behavior with middleware. SDK authors can
add, replace, or patch source in one language package through their recipe;
regeneration reapplies the override. See [SDK customization](docs/sdk-customization.md)
for language-specific middleware and package-scoped source examples.

## Rust interface

For embedding Kaji or writing custom plugins, a typed Rust interface is also
available. See the [Rust API guide](docs/getting-started.md) and
[plugin authoring reference](docs/typed-plugins.md).

## New in 0.4.0

- Existing npm manifests retain custom scripts, metadata, pinned dependencies,
  and dependency categories while Kaji appends missing generated requirements.
- Symfony bundles wrap the portable PHP SDK for container-managed clients.
- Generated TypeScript and Rust API CLIs prompt in interactive terminals, mask
  credentials, and stay non-interactive for pipes and `--json`.
- Generated CLI packages include OpenAPI-derived command references.

See the [0.4.0 release notes](docs/releases/0.4.0.md) for details.

## Documentation

Start with the [documentation home](docs/README.md), then choose a workflow:

- **[Why Kaji](docs/why-kaji.md):** the problem Kaji solves, its scope, and its
  no-monetization commitment.

- **[CLI workflow](docs/cli/README.md):** quickstart, recipes, command reference,
  and reproducible `kaji.json` configuration.
- **[Rust library workflow](docs/library/README.md):** embedded generation,
  package composition, and native plugin development.
- **[Generated SDK guide](docs/generated-sdks.md):** runtime behavior and target
  requirements.
- **[TypeScript API CLI](docs/typescript-cli.md) and [Rust API CLI](docs/rust-cli.md):**
  distributable API commands, OAuth, API keys, profiles, and extensions.
- **[TypeScript helpers](docs/guides/typescript-helpers.md),
  [testing](docs/guides/testing.md), and [generated artifacts](docs/guides/artifacts.md):**
  validation, hooks, fixtures, mocks, API docs, and MCP output.
- **[CI integration](docs/ci-integration.md), [mocking](docs/mocking.md), and
  [large-spec guidance](docs/large-specs.md):** adopt generation safely in a
  production repository.

Existing detailed references remain available: [complete CLI reference](docs/cli.md),
[`kaji.json` schema reference](docs/config-file.md), [configuration](docs/configuration.md),
[auxiliary generators](docs/auxiliary-generators.md), and
[plugin authoring](docs/typed-plugins.md). The current
[safe regeneration](docs/safe-regeneration.md), and
[SDK automation](docs/sdk-automation.md), including remaining verification gaps.

## Runnable examples

- [Symfony SDK](examples/symfony-sdk/README.md): generate a PHP SDK and its
  Symfony integration bundle together.
- [Manifest merging](examples/manifest-merging/README.md): preserve custom npm
  scripts, pinned TypeScript, and optional TanStack peers across generation.
- [Native Rust CLI](examples/rust-cli/README.md): generate and build a native
  command-line client with terminal prompts and command references.

- [Examples index](examples/README.md): choose a minimal CLI recipe, a
  multi-package build, or an embedded Rust application.
- [CLI basic](examples/cli-basic/README.md): a small contract and one
  reproducible TypeScript SDK recipe.
- [CLI multi-package](examples/cli-multi-package/README.md): TypeScript, Go,
  documentation, and a mock service from one contract.
- [API CLI](examples/typescript-cli/README.md): TypeScript and Rust command-line
  clients with OAuth, profiles, and nested commands.
- [Mock scenarios](examples/mock-scenarios/README.md): run a native mock
  locally or generate Docker fixtures with contract-owned conditional responses.
- [React Query consumer](examples/react-query-consumer/README.md): wire a
  generated SDK and TanStack hooks into an application.
- [MCP API tools](examples/mcp-api-tools/README.md): expose a contract through
  a local stdio MCP server.
- [Rust embedded](examples/rust-embedded/README.md): call Kaji from an
  application instead of a shell command.
- [Complete TypeScript stack](examples/typescript-stack/README.md): Fetch and
  Axios SDKs, plus Zod, TanStack React/Vue Query, SWR, Faker, MSW, Cypress, and
  the shared HTTP mock server from one small OpenAPI contract.
- [Microsoft Graph](examples/microsoft-graph/README.md): a URL-backed,
  large-contract Fetch and Go generation demo.

## License

Licensed under the [MIT License](LICENSE).

### Commercial license

Need a commercial license for Kaji? We've got you covered.

**[Get a commercial license →](https://www.youtube.com/watch?v=dQw4w9WgXcQ)**
