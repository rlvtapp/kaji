# Why Relevate built Poolster

We built Poolster for Relevate Email. An API change needed to reach our SDKs,
frontend hooks, validators, mocks and documentation without maintaining every
artifact by hand.

That started with OpenAPI. Poolster now also has native GraphQL, Protobuf,
AsyncAPI, Arazzo and Cap’n Proto inputs. Each protocol keeps its own semantics;
GraphQL operations and event messages do not become pretend HTTP endpoints.

## One source, several useful outputs

Choose the artifacts your project needs and generate them together:

| Source | Implemented pipelines in the current checkout |
| --- | --- |
| OpenAPI | HTTP SDKs in multiple languages, query hooks, validators, fixtures, mocks and other tooling |
| GraphQL schema + operations | TypeScript and Rust clients; TypeScript query hooks, validators, fixtures and testing helpers |
| Protobuf | Go messages and gRPC clients/server interfaces, including streaming |
| AsyncAPI | TypeScript Kafka message models and producer/consumer support |
| Arazzo | TypeScript sequential workflow runners with resolved local OpenAPI sources |
| Cap’n Proto | Parsing and inspection; bundled generation remains planned |

These are specific supported pipelines, not universal support for every protocol
feature or output plugin. Some additions are unreleased. The
[support matrix](../plugin-support-matrix.md) records the exact boundaries and
verification status.

GraphQL clients use supplied operation documents to generate selection-specific
results. AsyncAPI describes messages and broker interactions. Those differences
matter to the generated package, so they remain visible throughout generation.

## Extend the pipeline, not just a template

Input plugins publish typed contracts. Output plugins consume compatible
contracts and produce owned files. Contracts can also expose building blocks,
such as models, endpoints or operations, for focused handlers. Decomposition is
optional: a custom plugin can consume a whole contract directly.

Declared dependencies connect plugins. Revisions, provenance and completeness
checks help consumers detect inconsistent inputs. Package assembly,
customization and ownership checks keep those outputs in one regeneration
workflow.

Use the [JavaScript SDK](../javascript/README.md) from Node, the
[Rust SDK](../rust/README.md) inside an application, or the
[CLI](../cli/README.md) in a repository. JavaScript and Rust plugin APIs have
different capabilities; their tutorials explain what each exposes:
[JavaScript plugins](../plugins/javascript/README.md) ·
[Rust plugins](../plugins/rust/README.md).

## Keep generation reviewable

A committed recipe makes generation repeatable. Check for drift in CI, prepare
SDK changes as pull requests, and test generated packages with their native
language tools. Customize generated sources through the supported regeneration
workflow, or eject the generator source when you want to maintain your own renderer.

- [GitHub Actions and generated PRs](../cli/automation.md)
- [Customization, regeneration and ejection](../cli/customization.md)
- [Verification results and remaining checks](../verification/verification.md)

Repeatable generation and a passing build do not prove every endpoint or
protocol feature works. We keep runtime checks, ignored tests and remaining
work visible so teams can decide whether a pipeline fits their needs.

## Built for our own use, released for yours

Relevate Email is why we maintain Poolster. Publishing it as open source lets
other teams use it, inspect the implementation and contribute their own plugins.

Poolster is MIT licensed and free to use, including for commercial work. We will
not add a paid tier, feature-gate the generator, sell a commercial license or
create an enterprise-only edition.

If something is missing, open an issue, propose a design or contribute a plugin.
Start with the [documentation overview](../README.md), the
[working examples](../../examples/README.md), or the
[contracts and lifecycle guides](../internals/README.md).
