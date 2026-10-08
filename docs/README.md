# Poolster docs

**Read a contract. Compose plugins. Ship something useful.**

```text
Input plugins → Poolster → Output plugins → Your packages
```

<a id="sdk-authors-from-a-contract-to-a-released-package"></a>

## Pick your starting point

| You want to… | Start here |
| --- | --- |
| Generate your first SDK | [CLI quickstart](cli/quickstart.md) |
| Configure a repeatable build | [Recipes](cli/config.md) |
| Use an SDK someone gave you | [Generated SDKs](generated-sdks.md) and its package README |
| Add a format or generator | [Plugin development](library/README.md) |
| Embed Poolster in your own tool | [Rust quickstart](library/quickstart.md) |
| Use JavaScript or TypeScript plugins | [Node API](../packages/npm/sdk/README.md) |
| Move an existing SDK project | [Migration](migration.md) |

## Understand the pieces

### Input plugins

Read and validate source documents, then publish typed data.

- [OpenAPI compiler](openapi-compiler.md): the existing HTTP SDK workflow.
- [Input providers](input-plugins.md): GraphQL, AsyncAPI, Arazzo, Protobuf and Cap’n Proto.
- [Discover a contract](discovery.md): find or download public OpenAPI documents.

Outputs must support the published contract. Native parsing does not make every
SDK generator compatible with every format.

### Poolster

Compose packages, resolve dependencies and manage generated files.

[Architecture](architecture.md) · [CLI workflow](cli/README.md) ·
[Rust workflow](library/README.md) · [Node workflow](../packages/npm/sdk/README.md) ·
[Safe regeneration](safe-regeneration.md)

<a id="api-artifacts"></a>

### Output plugins

| Output | Guides |
| --- | --- |
| SDK packages | [Language capabilities](generated-sdks.md) · [Provider composition](native-sdk-providers.md) |
| API commands | [TypeScript CLI](typescript-cli.md) · [Rust CLI](rust-cli.md) |
| Frontend helpers | [Query hooks, validation and fixtures](guides/typescript-helpers.md) |
| Local APIs | [Contract mocks](mocking.md) |
| API tools | [Postman](postman.md) · [Terraform](terraform-provider.md) · [MCP](mcp-server.md) |
| Docs and other artifacts | [Auxiliary generators](auxiliary-generators.md) · [Artifact recipes](guides/artifacts.md) |

<a id="plugin-developers-extend-generation-through-contracts"></a>

[All extension hooks at a glance →](plugin-hooks.md)

## Build a plugin

Start with the [working example](../examples/custom-plugin/README.md).
Then choose a hook:

| Hook | Reference |
| --- | --- |
| Read another input format | [Register an input provider](input-plugins.md#select-or-replace-a-provider) |
| Consume data and emit files | [Write an output plugin](typed-plugins.md#implement-a-reusable-consumer) |
| Swap a model or transport provider | [Compose providers](typed-plugins.md#compose-typescript-providers-independently) |
| Add a language | [Language responsibilities](typed-plugins.md#what-core-owns-and-what-the-language-owns) |
| Add derived artifacts after generation | [Generation phases](typed-plugins.md#generation-phases-and-file-ownership) |
| Bundle middleware or custom source | [SDK customization](sdk-customization.md) |

Plugins are Rust crates linked into a generator application. The shipped CLI
exposes its compiled-in plugins. [Full authoring reference →](typed-plugins.md)

In Node.js, install the input or output packages you want and select their
exports in `poolster.config.mjs`. You can also write JavaScript input and output
plugins. [Node authoring guide →](../packages/npm/sdk/README.md)

## Generate, review, release

1. [Generate](cli/quickstart.md) a package from your contract.
2. [Customize](sdk-customization.md) behavior through your recipe.
3. [Check regeneration](safe-regeneration.md) and [test the SDK](guides/testing.md).
4. [Open SDK update PRs](sdk-automation.md).
5. [Release and publish](sdk-publishing.md) reviewed versions.

```text
Contract change → SDK PR → Release PR → Tag → Checks → Publication
```

[CI setup](ci-integration.md) · [GitHub Actions](github-actions.md) ·
[GitHub App](github-app.md) · [OIDC broker](github-app-broker.md)

<a id="sdk-users-install-and-call-the-delivered-package"></a>

## Use the generated SDK

The package README gives the exact installation, exports and runtime requirements.
Use these guides when you need a specific behavior:

| Task | Guide |
| --- | --- |
| Add HTTP policy | [Middleware](guides/runtime-middleware.md) |
| Follow pages | [Pagination](guides/pagination.md) |
| Retry mutations safely | [Idempotency](guides/idempotency.md) |
| Set headers, deadlines or cancellation | [Request controls](guides/request-controls.md) |
| Upload files or JSON parts | [Uploads](guides/uploads.md) |
| Use OAuth or signed webhooks | [OAuth and webhooks](guides/oauth-webhooks.md) |
| Handle future enum values and fields | [Model compatibility](guides/forward-compatible-models.md) |
| Use OpenAPI 3.2 wire formats | [OpenAPI 3.2](guides/openapi32.md) |

<a id="find-a-focused-guide"></a>

## Ask AI about Poolster

Use the [AI context and copyable prompts](ai.md), or start from the
[README’s quick prompt](../README.md#ask-ai-about-poolster).
Give your assistant your goal, Poolster version and target language.

[Poolster tools through MCP →](mcp-server.md)

## Reference shelf

[All CLI options](cli.md) · [JSON recipe fields](config-file.md) ·
[Rust settings](configuration.md) · [Feature catalog](features.md) ·
[Verification](verification.md) · [Large contracts](large-specs.md)

Need a complete project? Browse [examples](../examples/README.md).
For source builds and forks, see [source customization](source-customization.md).

<details>
<summary>Benchmarks, background and plans</summary>

- [Compatibility results](guru-compatibility.md) and [shared fixtures](shared-sdk-fixtures.md)
- [Why Poolster](why-poolster.md) and [comparison](comparison.md)
- [Roadmap](sdk-roadmap.md) and [generator backlog](generator-backlog.md)
- [Postman plan](postman-generation-plan.md) and [Terraform plan](terraform-provider-plan.md)
- [0.4.0](releases/0.4.0.md) and [0.3.0](releases/0.3.0.md) release notes
- [AI context](ai.md) and [contributing](contributing.md)

</details>

<a id="capability-and-verification-boundaries"></a>

A snapshot, a native build and an API test prove different things.
[Verification](verification.md) records what has been checked and what still needs testing.
