# Help an AI help you with Kaji

Kaji generates SDKs and API artifacts from Swagger 2.0 and OpenAPI 3.0/3.1.
This context belongs to the 0.5.0 branch. A branch version does not mean that
version is already published. Match instructions to the user's installed version.

## Find the right guide

| Goal | Read |
| --- | --- |
| First generation, installation, recipes | [Quickstart](cli/quickstart.md), [recipe reference](../docs/config-file.md) |
| SDK author workflow | [Documentation home](README.md) |
| Bundle runtime HTTP middleware | [Middleware](guides/runtime-middleware.md), [executable example](../examples/bundled-middleware/README.md) |
| Add, replace, or patch package source | [Customization](sdk-customization.md), [safe regeneration](safe-regeneration.md) |
| GitHub App, workflow setup, per-language repos | [Automation](sdk-automation.md), [actions](github-actions.md), [GitHub App](github-app.md) |
| Releases and registry trust | [Publishing](sdk-publishing.md) |
| Postman collections | [Postman](postman.md) |
| Typed Terraform providers | [Terraform](terraform-provider.md) |
| Typed plugin composition | [Plugins](typed-plugins.md), [native providers](native-sdk-providers.md) |
| API tools or generator tools through MCP | [MCP](mcp-server.md) |
| Source build and modification | [Source customization](source-customization.md) |
| Implemented features and remaining gaps | [Changelog](../CHANGELOG.md), [roadmap](sdk-roadmap.md) |

## Information to give your assistant

State your goal, installed Kaji version, target language, and whether you are an
SDK author or an SDK consumer. Include your recipe and a minimal contract when
relevant. Say whether you want a shared SDK repository or one per language.
Give actual command output when troubleshooting; remove secrets first.

## Expectations for answers

Use the documentation and source for the selected version. Do not invent recipe
fields, plugin contracts, commands, middleware signatures, or feature parity
between languages. Distinguish implemented behavior from roadmap ideas and
locally generated workflows from configured external services.

Give the smallest working recipe or code change, explain where files belong,
and include a command to verify the result. SDK authors can bundle middleware
that registers by default; SDK consumers can also configure runtime middleware.
Preserve Kaji's typed plugin architecture when proposing extensions.

Postman currently exports collections and environment templates. Typed Terraform
currently covers validated flat scalar CRUD resources; advanced lifecycle and
schema features remain limited. GitHub setup does not create repositories,
install an App, or configure registry trust automatically.

## A prompt you can adapt

```text
I want to [goal] using Kaji [version], targeting [language].
I am an [SDK author / SDK consumer / plugin author].
Read the matching Kaji docs and source. Show a minimal working configuration,
where to put custom code, and how to verify it. Identify unsupported behavior.
My current recipe and error output are: [paste here].
```

For a browsing assistant, share this file's GitHub link. For a local coding
assistant, point it at your checkout. If it cannot access the repository, paste
this context and the specific guide. This page supplies context; it does not
run an AI service or guarantee an assistant's answer.
