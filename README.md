<h1><img src="docs/assets/banner.svg" alt="Poolster: change the contract once, every SDK, mock and doc follows" width="100%"></h1>

**One contract. Lots of possibilities. Your code.**

Poolster turns API contracts into SDKs, CLIs, docs and tools. Pick your plugins,
compose a recipe, and regenerate when your API changes.

[Get started](#quick-start) · [Explore outputs](#output-plugins) · [Build a plugin](#build-your-own-plugin) · [Read the docs](docs/README.md)

## How Poolster works

```text
Your contract
     │
     ▼
Input plugin     Read, validate and publish typed data
     │
     ▼
Poolster             Connect plugins, order the work, manage files
     │
     ▼
Output plugins   Generate SDKs, CLIs, docs and more
     │
     ▼
Your packages    Build, test, review and ship
```

Bring a format. Choose an output. Add your own behavior at either end.
Plugins share typed contracts, so a consumer can ask for exactly the data it needs.

### Input plugins

OpenAPI powers the existing SDK generation workflow. These additional providers
can parse, validate and publish their native contracts:

| Input | Provider and details |
| --- | --- |
| OpenAPI | [Go compiler and HTTP model](docs/openapi-compiler.md) |
| GraphQL | [Apollo provider](crates/inputs/graphql/README.md) |
| AsyncAPI | [Roas event provider](crates/inputs/asyncapi/README.md) |
| Arazzo | [Roas workflow provider](crates/inputs/arazzo/README.md) |
| Protobuf | [Protox provider](crates/inputs/protobuf/README.md) |
| Cap’n Proto | [Official compiler provider](crates/inputs/capnproto/README.md) |

Each new provider is its own Rust crate. Use one directly, or pick features in
the `poolster-inputs` bundle. **Outputs must support the input's contract:** GraphQL,
event, workflow and RPC SDK generators still need their own consumers.
[Input support and inspection →](docs/input-plugins.md)

### Poolster itself

Poolster resolves dependencies, runs providers before consumers, and assembles each
package. It tracks generated files so regeneration can protect your custom work.

Use a JSON recipe through the CLI or compose packages in Rust.
[Architecture](docs/architecture.md) · [Recipes](docs/config-file.md) · [Rust interface](docs/library/quickstart.md)

### Output plugins

| Make something | Explore the plugins |
| --- | --- |
| SDKs | [TypeScript](crates/plugins/typescript/README.md) · [Rust](crates/plugins/rust/README.md) · [Go](crates/plugins/go/README.md) · [Python](crates/plugins/python/README.md) · [PHP](crates/plugins/php/README.md) |
| More SDKs | [Java](crates/plugins/java/README.md) · [C#/.NET](crates/plugins/csharp/README.md) · [Elixir](crates/plugins/elixir/README.md) · [Ruby](crates/plugins/ruby/README.md) · [Swift](crates/plugins/swift/README.md) |
| API commands | [TypeScript CLI](docs/typescript-cli.md) · [Rust CLI](docs/rust-cli.md) |
| Infrastructure and API tools | [Terraform](docs/terraform-provider.md) · [Postman](crates/plugins/postman/README.md) · [MCP](docs/mcp-server.md) |
| Frontend and testing helpers | [Query hooks, Zod, Faker, MSW and Cypress](docs/guides/typescript-helpers.md) · [HTTP mocks](docs/mocking.md) |
| Documentation | [API references and ReDoc](docs/auxiliary-generators.md) |

Choose the pieces you need. Features vary by language; the
[feature catalog](docs/features.md) has the full details.

## Quick start

```sh
npm install -D poolster
npx poolster init --input ./openapi.yaml --output ./generated --name "Email" --sdk-version 1.0.0
# Pick packages and plugins in poolster.json, then:
npx poolster generate
```

The `poolster` package downloads the native CLI and OpenAPI compiler. It does
not install the Node SDK addon. No Rust or Go
installation needed. Python users can use `pip install poolster` instead.

New input providers and unreleased 0.5.0 features require a
[source build](docs/source-customization.md).
[Your first SDK →](docs/cli/quickstart.md)

## Use Poolster from Node.js

The [Node API](packages/cli/sdk/README.md) embeds the Rust renderers and loads a
`poolster.config.mjs`. Install `@relevate/poolster` for this API, then add individual
language and input packages, or the
`@relevate/poolster-plugins` bundle, then add only the exports you use. JavaScript
output plugins can work alongside Rust SDK plugins. GraphQL, AsyncAPI, Arazzo,
Protobuf and Cap'n Proto input packages expose the Rust parsers to Node; their
native summaries can feed JavaScript output plugins. You can also write a
JavaScript input plugin and publish a normalized HTTP API for Rust SDK renderers.

[Runnable Node examples →](examples/node-embedded/README.md)

## Build your own plugin

Plugins are Rust crates linked into your generator. Start with the
[working custom-plugin example](examples/custom-plugin/README.md), then choose
where to hook in. Community crates can be composed in a Rust application;
the shipped CLI uses its compiled-in plugin set.

### Read a new format

Implement `InputPlugin`, register your provider, and publish native typed data.
You can also offer an alternative parser for an existing format.
[Input provider guide →](docs/input-plugins.md#select-or-replace-a-provider)

### Generate something new

Implement `Plugin<Language>` to consume contracts and emit files. Build an SDK,
a framework integration, a test fixture, or something we haven't thought of.
[Output plugin guide →](docs/typed-plugins.md#implement-a-reusable-consumer)

### Replace one piece

Swap a model, transport or operation provider while keeping its consumers.
Explicit handles let a recipe choose the provider it wants.
[Provider composition →](docs/typed-plugins.md#compose-typescript-providers-independently)

### Add a language

Define a `Language`, its workspace and renderers. Finalization can assemble
manifests, exports and shared files.
[Language responsibilities →](docs/typed-plugins.md#what-core-owns-and-what-the-language-owns)

### Run after generation

Use a Post plugin for derived artifacts such as release metadata.
[Phases and file ownership →](docs/typed-plugins.md#generation-phases-and-file-ownership)

### Ship your own behavior

Bundle HTTP middleware or add, replace and patch package source through a recipe.
[Customization →](docs/sdk-customization.md) · [Middleware example →](examples/bundled-middleware/README.md)

## From generated to shipped

[Protect custom work](docs/safe-regeneration.md) ·
[Test SDK behavior](docs/guides/testing.md) ·
[Automate SDK PRs](docs/sdk-automation.md) ·
[Publish releases](docs/sdk-publishing.md)

## Keep exploring

[Examples](examples/README.md) · [Migration](docs/migration.md) ·
[Why Poolster?](docs/why-poolster.md) · [Comparisons](docs/comparison.md) ·
[Changelog](CHANGELOG.md) · [AI context](docs/ai.md)

Questions or a bug? [Open an issue](https://github.com/rlvtapp/kaji/issues/new)
with your version, command and a small reproducible example.

## Ask AI about Poolster

[Open ChatGPT](https://chatgpt.com/) or use your preferred assistant with our
[AI context and prompts](docs/ai.md). Copy this and fill in the brackets:

```text
Help me use Poolster to [my goal] for [language]. I am using version [version].
Read https://github.com/rlvtapp/kaji/blob/main/docs/ai.md, then use the docs
and source matching my version. Give me a minimal working recipe,
verification steps, and any documented limitations.
```

If your assistant cannot browse, share the guide or your local checkout.
For direct access to Poolster tools, see [MCP](docs/mcp-server.md).

## License

[MIT](LICENSE). Build something good.

### Commercial license

Need a commercial license for Poolster? We've got you covered.

**[Get a commercial license →](https://www.youtube.com/watch?v=dQw4w9WgXcQ)**
