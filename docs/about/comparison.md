# Why choose Poolster?

[Why Poolster](why-poolster.md) · [Feature catalog](features.md) · [Try it](../cli/quickstart.md)

Poolster is the best fit for SDK authors who want to own and extend their entire
OpenAPI generation workflow: native SDKs, bundled HTTP policies, API artifacts,
checks and releases, composed through one typed plugin graph.

You can replace a provider, add a consumer, ship your own source per language,
eject and rebuild the generator, and keep delivery in readable GitHub workflows.
That combination is Poolster's main advantage.
The choice is about control and scope;
it is not a claim that every generated language or OpenAPI construct has the same
coverage.

See the [feature catalog](features.md) and [verification](../verification/verification.md).

## What makes Poolster a strong choice

| What you need | What Poolster gives you | Evidence and entry point |
| --- | --- | --- |
| Generation you can extend | Typed Rust capabilities, named providers and consumers, dependency checks | [Plugin development](../internals/typed-plugins.md) |
| Policies customers get automatically | Authored modules bundled and registered during generation, plus customer-side middleware or native driver injection | [SDK customization](../reference/regeneration/sdk-customization.md) |
| A generator you can modify | Rebuildable source ejection with a source manifest | [Source customization](../reference/regeneration/source-customization.md) |
| One contract, multiple deliverables | Ten SDK targets, TypeScript integrations, Postman, typed Terraform, API CLIs and MCP artifacts | [Features](features.md) |
| Safe repeated generation | Output ownership, edited-file protection, guarded patches and read-only drift checks | [Safe regeneration](../reference/regeneration/safe-regeneration.md) |
| Delivery you can inspect | Editable per-language checks, sync PRs, release configuration and publishing actions | [Automation](../reference/automation/sdk-automation.md) |
| Claims you can verify | Native compilation, local HTTP conformance and pinned public-contract regression | [Verification](../verification/verification.md) |

These are implemented capabilities, with the target-specific limits described in
the linked guides. A plugin API, runtime middleware and editable generator sources
serve different purposes; Poolster provides all three rather than treating them as
interchangeable customization settings.

## Compare the main alternatives

Reviewed against official documentation on **8 October 2026**. This is a broad
selection of established tools, not an exhaustive census. The rows describe each
project's documented focus; they are not performance benchmarks or universal
support judgments. “Choose Poolster when” is our assessment of fit.

### SDK platforms and broad generators

| Tool | Documented focus and customization | Choose Poolster when |
| --- | --- | --- |
| **Poolster** | Local CLI and Rust library; native SDKs and API artifacts; typed generation plugins; authored source, runtime policies and editable delivery | You want this entire workflow in an extensible source toolchain you own. |
| **Speakeasy** | SDKs, Terraform, MCP and Postman generation; annotated contracts and workflow configuration; published generator sources | You prefer Poolster's typed provider/consumer composition and package-scoped customization as the core abstraction. [Official generator](https://github.com/speakeasy-api/openapi-generation), [workflow concepts](https://www.speakeasy.com/docs/sdks/core-concepts). |
| **Fern** | Multi-language SDKs and documentation; generator configuration and custom code; managed generation and a self-hosted option | You want local source builds and a plugin graph without configuring a managed generation service. [Generation](https://buildwithfern.com/learn/sdks/overview/how-it-works), [self-hosting](https://buildwithfern.com/learn/sdks/deep-dives/self-hosted). |
| **Stainless** | Configured resources/models, SDK generation, publishing and docs; configuration also includes Terraform and CLI targets | You want to compose or implement generator providers directly and own the generation sources. [Configuration](https://www.stainless.com/docs/reference/config/), [SDK configuration](https://www.stainless.com/docs/sdks/configure/). |
| **OpenAPI Generator** | Broad client/server/documentation generator catalog; custom templates, supporting files and custom generators | You want typed cross-plugin contracts and SDK delivery tooling integrated with API artifacts. It remains a strong option when you need a target outside Poolster's maintained set. [Project](https://github.com/OpenAPITools/openapi-generator), [customization](https://openapi-generator.tech/docs/customization/). |
| **Swagger Codegen** | Template-driven API clients, server stubs and documentation across languages; custom templates and generators | You want Poolster's native plugin composition, owned-output regeneration and delivery workflow. [Project](https://github.com/swagger-api/swagger-codegen), [generator customization](https://github.com/swagger-api/swagger-codegen/blob/master/docs/generators.md). |
| **APIMatic** | SDK generation, developer portals, publishing and preserved custom code | You want generation behavior implemented through local typed plugins and rebuildable source ownership. [SDK overview](https://docs.apimatic.io/v4/generate-sdks/apimatic-sdks/), [portals](https://docs.apimatic.io/v4/generate-developer-portals/generate-build/). |
| **AutoRest** | OpenAPI generation across languages through its generator ecosystem and configuration | You want Poolster's typed artifact composition and integrated SDK/artifact delivery workflow. [Project](https://github.com/Azure/autorest), [configuration](https://github.com/Azure/autorest/blob/main/docs/user/literate-file-formats/configuration.md). |
| **Microsoft Kiota** | Multi-language clients using request adapters, authentication, serialization and middleware abstractions | You want native packages plus non-SDK artifacts and generator composition; Kiota is worth evaluating if a shared adapter ecosystem is your priority. [Official documentation](https://learn.microsoft.com/en-us/openapi/kiota/). |
| **NSwag** | C#/TypeScript clients and ASP.NET OpenAPI generation, CLI and build integration | You need more languages and artifacts from the same contract. NSwag is worth evaluating for an ASP.NET-centered workflow. [Project and features](https://github.com/RicoSuter/NSwag). |

### TypeScript and frontend tools

| Tool | Documented focus and customization | Choose Poolster when |
| --- | --- | --- |
| **Kubb** | Generation built around plugins, generators, resolvers and hooks | You want plugin composition together with maintained native SDKs beyond TypeScript and API delivery artifacts. [Plugin architecture](https://www.kubb.dev/docs/5.x/guide/concepts/plugins). |
| **Hey API** | TypeScript SDKs/types/schemas, multiple clients and an integration plugin ecosystem | Your API must also ship native SDKs in other languages and owned publishing workflows. [Getting started](https://heyapi.dev/docs/openapi/typescript/get-started). |
| **Orval** | TypeScript clients from OpenAPI, frontend integrations and custom HTTP mutators | You want the same generation toolchain for frontend code, native SDKs and Terraform/Postman artifacts. [Documentation](https://orval.dev/docs/), [custom clients](https://orval.dev/docs/guides/custom-client/). |
| **openapi-typescript / openapi-fetch** | Generated TypeScript schema types paired with a typed Fetch runtime, serializers and middleware | You need emitted native SDK packages and multiple artifact types. The lightweight types/runtime approach is a useful alternative for a TypeScript application. [Runtime API](https://openapi-ts.dev/openapi-fetch/api). |
| **OpenAPI Client Axios** | An Axios client initialized from an OpenAPI definition, with operation methods and ordinary Axios configuration | You want generated source packages, cross-language output and generation-time policies. [Official API reference](https://openapistack.co/docs/openapi-client-axios/api/). |

### Language-specific generators

| Tool | Documented focus | Choose Poolster when |
| --- | --- | --- |
| **oapi-codegen** | Go client, server and model generation with configuration and an ecosystem of integrations | You want a shared generation/delivery configuration across several SDK languages. [Project](https://github.com/oapi-codegen/oapi-codegen). |
| **ogen** | Go API code generation, including clients and servers | You want other SDK languages and artifacts alongside Go. Evaluate its generated codecs and server support separately for a Go-only service. [Getting started](https://ogen.dev/docs/intro/). |
| **openapi-python-client** | Modern Python client generation, configuration and template customization | You want Python within a multi-language plugin and delivery workflow. [Project](https://github.com/openapi-generators/openapi-python-client). |

### Related model-first toolchains

**TypeSpec** describes APIs and emits client code through language-specific
emitters. Choose it when a model-first authoring language is central to your
workflow; choose Poolster when an existing OpenAPI contract is the input to your
SDK and artifact pipeline. [Client emitters](https://typespec.io/docs/emitters/clients/introduction/),
[custom emitters](https://typespec.io/docs/extending-typespec/emitters-basics/).

**Smithy** is a model and code-generation ecosystem, not simply another OpenAPI
SDK command. Its generator architecture is worth evaluating if you want to design
around Smithy models and protocols. Poolster is a fit when OpenAPI is your existing
contract and you want its outputs composed through Poolster plugins.
[Code generation](https://smithy.io/2.0/guides/using-code-generation/index.html),
[generator concepts](https://smithy.io/2.0/guides/building-codegen/overview-and-concepts.html).

## Where another tool can be a better fit

A managed SDK platform can be a better fit when you want a service and support
team to operate generation and delivery. A language-specific generator can be a
better fit when its particular runtime, framework or server generator matches
your application. OpenAPI Generator has a broader target catalog than Poolster.

Poolster's advantage is the combination of ownership, typed composition and breadth
of deliverables. Validate your contract in the languages you ship before deciding:
run native compilation, exercise your middleware, and check the workflows against
your actual repository and registry setup. The public-contract matrix exposes
failures; it does not turn every target into a blanket compatibility guarantee.
