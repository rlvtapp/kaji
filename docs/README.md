# Kaji documentation

Kaji turns an OpenAPI contract into SDK packages and related artifacts. The main workflow is for **SDK authors who own the generated code and its releases**: keep a recipe, bundle your policies, review regeneration, and deliver tested versions to customers.

Use the [full feature catalog](features.md) to compare SDK targets, runtime policies, plugins, artifacts and delivery options. It distinguishes implemented features from verified behavior and prepared workflows.

If you already received an SDK, start with [using generated SDKs](generated-sdks.md) and its package-local README. You do not need Kaji to use that package.

## SDK authors: from a contract to a released package

Start with a local Swagger 2.0 or OpenAPI 3.0/3.1 document and a supported Kaji installation. Building the output also needs the target language's toolchain. Publishing later requires an SDK repository, registry identity, and configured authentication; generation itself does not create those accounts or publish anything.

| Step | Read and do | What you have afterwards |
| --- | --- | --- |
| 1. Generate | [CLI quickstart](cli/quickstart.md) uses the small local Notes contract. [CLI workflow](cli/README.md) explains installation and repeatable recipes. | A `kaji.json`, generated package, and an output check you can repeat. |
| 2. Shape your SDK | [Recipe configuration](cli/config.md) selects packages/plugins. [SDK customization](sdk-customization.md) bundles author middleware and source overrides. | Package-specific behavior shipped to customers by default, with customization sources kept beside the recipe. |
| 3. Validate changes | [Safe regeneration](safe-regeneration.md), [testing guide](guides/testing.md), and [verification overview](verification.md). | A reviewable generated diff and native build/behavioral checks. |
| 4. Send an SDK update | [SDK repository automation](sdk-automation.md) configures metadata, local setup, and generated SDK PRs. [GitHub Actions arrangements](github-actions.md) cover spec relay, scheduled fetches and remote inspection. | Editable checks and synchronization workflows for the source or a separate SDK repository. |
| 5. Release and publish | [Publishing generated SDKs](sdk-publishing.md) explains Release Please integration, native manifests/tags, registry prerequisites, and publication checks. | Independently versioned, tested packages published through the chosen registry workflow. |

The delivery sequence is **contract change → generated SDK PR → merge → Release Please version/changelog PR → merge → release tag → checks → publication**. The automation and publishing guides explain which files are scaffolded locally and which repository permissions, environments, and registry trust relationships you must configure yourself. Use a launcher containing the commands in those guides; [source customization](source-customization.md) explains unpublished builds and forks.

For complete projects, use [examples](../examples/README.md). If your contract is remote or private, read [discovery and download](discovery.md) or [CLI recipes](cli/recipes.md) before committing source credentials or workflow settings.

## SDK users: install and call the delivered package

The generated package's README is the first reference for its installation, exports, authentication, and native requirements. The [generated SDK guide](generated-sdks.md) explains client layouts, models, errors, and capabilities across languages. For application-level request policies, follow [runtime middleware](guides/runtime-middleware.md); an author's bundled policy is already enabled.

Choose the guide for the task around your client: [pagination](guides/pagination.md) for declared continuation and target capabilities, [idempotency](guides/idempotency.md) for bundled keys and safe mutation retries, [TypeScript helpers](guides/typescript-helpers.md) for validation/data fetching, [contract mocking](mocking.md) for local APIs, or [testing generated SDKs](guides/testing.md) for executable fixtures. Those artifacts are selected by the SDK author; they are not automatically present in every package.

## Plugin developers: extend generation through contracts

Use the [Rust library workflow](library/README.md) when generation belongs in a tool or when you need a native custom plugin. Start from the [standalone custom plugin example](../examples/custom-plugin/README.md). Follow [plugin composition](library/plugins.md), then the [typed plugin reference](typed-plugins.md) to provide or consume contracts and control package finalization. [Architecture](architecture.md) explains the compiler boundary and neutral model; [native SDK providers](native-sdk-providers.md) documents Rust/Go transport composition and runtime extension boundaries.

Native Rust plugins are composed through the library API. Installing an arbitrary plugin does not register it in `kaji.json`; the CLI exposes its bundled registry. Keep target-specific behavior in the plugin, and use [source customization](source-customization.md) if you need to extend the CLI or delivery actions.

## Find a focused guide

| Topic | Guide |
| --- | --- |
| Full capabilities and target differences | [Feature catalog](features.md) |
| Per-call headers, timeouts and cancellation | [Request controls](guides/request-controls.md) |
| OAuth providers and signed webhooks | [OAuth/webhooks](guides/oauth-webhooks.md) |
| All CLI commands or typed settings | [CLI reference](cli.md), [configuration reference](configuration.md) |
| Large contracts and compiler artifacts | [Large specifications](large-specs.md), [OpenAPI compiler](openapi-compiler.md) |
| Extra generated outputs | [Auxiliary generators](auxiliary-generators.md), [ReDoc/MCP artifacts](guides/artifacts.md) |
| API command-line clients | [TypeScript API CLI](typescript-cli.md), [Rust API CLI](rust-cli.md) |
| CI, App authentication, and broker setup | [CI integration](ci-integration.md), [GitHub App](github-app.md), [OIDC broker](github-app-broker.md) |
| Native model fixtures | [Shared SDK fixtures](shared-sdk-fixtures.md) |
| Product background and release changes | [Why Kaji](why-kaji.md), [0.4.0 release](releases/0.4.0.md), [0.3.0 release](releases/0.3.0.md) |

## Capability and verification boundaries

A generated source snapshot, a native compile, a mock lifecycle test, and a live registry upload prove different things. Read [verification](verification.md), [native providers](native-sdk-providers.md), and the relevant language guide before choosing production checks. Some native tests require a toolchain and run explicitly; bounded [schema fixtures](shared-sdk-fixtures.md) are not exhaustive protocol tests. Add tests for your API's authentication, errors, pagination, and any bundled policy.

Generate portable [Postman collections](postman.md) and a supported subset of [typed Terraform providers](terraform-provider.md) through the CLI or native plugins. Their design plans retain advanced follow-up work; the [roadmap](sdk-roadmap.md) distinguishes implemented behavior from future stages.

## API artifacts

Generate [Postman collections](postman.md) and [typed Terraform providers](terraform-provider.md), with a [Speakeasy comparison](terraform-speakeasy.md), beside your SDKs. The [combined example](../examples/api-artifacts/README.md) includes a recipe and editable CI checks.

Compare the options in [Why choose Kaji?](comparison.md), including the strengths
of its plugin architecture, source ownership and integrated delivery.

For file and JSON-part uploads, see [multipart uploads](guides/uploads.md).
