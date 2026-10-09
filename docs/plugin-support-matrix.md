# Poolster plugin support matrix

Audited **9 October 2026**, against the unreleased alpha.2 implementation in this checkout.
Manifests still use `0.5.0-alpha.1`; the additions below are unreleased alpha.2
work. This matrix describes implemented generation, not parser availability.
For remaining work, start with the [native pipeline backlog](reference/inputs/native-pipelines.md#remaining-work).

GraphQL React Query, Vue Query, SWR, Zod, Faker, MSW and Cypress integrations now
have separate generated compilation and runtime checks. These checks establish
the documented GraphQL subset, rather than every existing HTTP plugin option.
See the [integration guide](reference/outputs/graphql-integrations.md) for API and limitations.

GraphQL clients are implemented for all ten SDK languages: TypeScript, Rust, Go,
Python, PHP, Java, C#, Ruby, Swift and Elixir. The deprecated `dotnet` target aliases
C#. All use the existing language plugin packages through Rust, CLI and npm entry points.
TypeScript and Rust have separate input/output scalar mappings; other languages
retain custom scalar JSON values without mapping codecs. All styles use fixed operation documents.
See the [language guides](reference/README.md) and [verification](verification/verification.md#graphql-client-completion-checks)
for tested features and per-language limits. These additions are not yet published.

✅ = implemented for the stated contract; — = no bundled generation support.
A check does not imply support for every feature of a specification. Target-specific
limitations still apply. GraphQL is a separate contract even when its transport is HTTP.

## Output plugins and accepted inputs

| Output plugin | Generated output | OpenAPI / HTTP `Api` | GraphQL operations | Protobuf RPC | AsyncAPI events | Arazzo workflows | Cap’n Proto |
| --- | --- | :---: | :---: | :---: | :---: | :---: | :---: |
| TypeScript | HTTP SDK/models; GraphQL client; Kafka message/client package; workflow runner | ✅ | ✅ | — | ✅ | ✅ | — |
| Zod | HTTP and selected GraphQL validators | ✅ | ✅ | — | — | — | — |
| Faker | HTTP and selected GraphQL fixtures | ✅ | ✅ | — | — | — | — |
| MSW | HTTP and GraphQL mock handlers | ✅ | ✅ | — | — | — | — |
| Cypress | HTTP and GraphQL request/interception helpers | ✅ | ✅ | — | — | — | — |
| React Query | TypeScript query/mutation hooks | ✅ | ✅ | — | — | — | — |
| Vue Query | TypeScript query/mutation hooks | ✅ | ✅ | — | — | — | — |
| SWR | TypeScript query/mutation hooks | ✅ | ✅ | — | — | — | — |
| Go | HTTP SDK; GraphQL client; official Protobuf messages and gRPC clients/server interfaces | ✅ | ✅ | ✅ | — | — | — |
| Rust | HTTP SDK; selection-specific GraphQL query/mutation client | ✅ | ✅ | — | — | — | — |
| Python | HTTP SDK; synchronous GraphQL client | ✅ | ✅ | — | — | — | — |
| Java | Java HTTP SDK; GraphQL client | ✅ | ✅ | — | — | — | — |
| C# | C# HTTP SDK; GraphQL client | ✅ | ✅ | — | — | — | — |
| .NET | Deprecated C# target alias | ✅ | ✅ | — | — | — | — |
| PHP | PHP HTTP SDK; GraphQL client | ✅ | ✅ | — | — | — | — |
| Symfony | Symfony/PHP package | ✅ | — | — | — | — | — |
| Ruby | Ruby HTTP SDK; GraphQL client with RBS | ✅ | ✅ | — | — | — | — |
| Elixir | Elixir HTTP SDK; GraphQL client | ✅ | ✅ | — | — | — | — |
| Swift | Swift HTTP SDK; GraphQL client | ✅ | ✅ | — | — | — | — |
| Rust CLI | Rust command-line client | ✅ | — | — | — | — | — |
| TypeScript CLI | TypeScript command-line client | ✅ | — | — | — | — | — |
| Postman | Collections, environments and examples | ✅ | — | — | — | — | — |
| Terraform | Terraform provider scaffolding | ✅ | — | — | — | — | — |

The TypeScript output package contains several generators: `ts::sdk()` /
`ts::types()` for HTTP, `ts::graphql()` for `GraphqlOperations`, `ts::asyncapi()`
for Kafka `EventOperations`, and `ts::workflow()` for `WorkflowOperations`.
These are generators within one plugin package, not separate language packages.
Go similarly provides its HTTP SDK and `go::grpc()` for `RpcContract`. Rust provides
`rust::graphql()` for GraphQL and its existing HTTP generators in one language package.

GraphQL companions consume the selected `GraphqlClient` contract, including
operation shapes and actual symbols; HTTP companions retain their HTTP contracts.
GraphQL hooks currently use single-file output and fixed query/mutation selections;
subscriptions and infinite-pagination helpers are unsupported. Zod/Faker runtime
scalar mappings support primitive wire types. Cypress mutation helpers require
explicit enablement. Input parsing alone does not establish companion support.
A Kafka package contains message types **and** producer/consumer code. Generation
writes files without contacting a broker; a broker is needed for runtime use and
integration tests. There is no standalone broker-independent AsyncAPI types-only
output yet, although input message blocks are available to custom plugins.

See [output migration coverage](verification/output-contract-migration.md) for typed HTTP
selection, block consumption, finalization and compatibility boundaries.

## Input providers

| Input format | Parsing / inspection | Usable bundled output |
| --- | :---: | --- |
| OpenAPI | ✅ | HTTP output plugins above |
| GraphQL SDL + operation documents | ✅ | All ten SDK languages through Rust API, CLI and npm entry points |
| Protobuf proto2/proto3 | ✅ | Go messages and gRPC clients/server interfaces |
| AsyncAPI 2.6 / 3.0 / 3.1 | ✅ | TypeScript Kafka for the supported 3.0/3.1 subset |
| Arazzo 1.0.0 / 1.0.1 / 1.1.0 | ✅ | TypeScript sequential runners with resolved local OpenAPI sources |
| Cap’n Proto | ✅ | — |

The npm input wrappers expose parsing/inspection and GraphQL client generation
through the existing language packages. Other native package
pipelines are exposed through Rust / CLI; general typed Node hooks remain open. CLI recipes currently
select one input source; Rust plugins can compose multiple typed contracts.

## GraphQL checks

- [x] Validate schema and operation documents together.
- [x] Selection-specific results, aliases, fragments and concrete abstract alternatives.
- [x] Preserve nullability, optional presence, lists and input defaults.
- [x] Generate variables and query/mutation functions with TypeScript Fetch and Rust Reqwest transports.
- [x] Represent GraphQL errors and partial results explicitly.
- [x] Compile generated TypeScript/Rust packages and execute against a local GraphQL server.
- [x] Separate TypeScript subscription capability with an injected async-iterable transport.
- [ ] Bundle a WebSocket or SSE subscription transport.
- [x] Separate input/output TypeScript scalar mappings.
- [x] Separate input/output Rust scalar mappings for validated self-contained wire types.
- [ ] Runtime scalar codecs.
- [ ] Introspection JSON and schema imports.
- [ ] Incremental delivery (`@defer` / `@stream`) and custom executable directives.
- [x] GraphQL support in Zod, Faker, MSW, Cypress and query-hook outputs (current checkout; unreleased).
- [x] GraphQL generation through the npm configuration engine (current checkout; unreleased).

Unsupported output packages succeed with warnings and a structured skipped-plugin
report. An all-skipped run leaves existing files untouched. Invalid input and
unsupported features in an otherwise supported pipeline still fail explicitly.

## Verification and release checks

Current alpha.2 workspace check: **816 passed, 0 failed, 143 ignored**;
formatting and workspace Clippy with warnings denied passed. Ignored tests are not passes.
Selected external GraphQL, gRPC, Kafka and workflow integration tests were also
run explicitly and passed; commands and boundaries are in [verification](verification/verification.md).

The pre-output-migration frozen-build **205 specs × 10 HTTP SDK targets** sweep
is complete: **2,050 effective passes**, including deterministic regeneration,
after audited infrastructure retries. PHP used native syntax lint and emitted
deprecation warnings in 140 contracts. These results do not establish endpoint
runtime behavior or corpus coverage of the later output migration. The migrated
workspace passed separately; final migrated-binary corpus coverage and clean
installation checks of release artifacts remain open. Original infrastructure
failures and successful retries remain in the audit trail. See
[current verification](verification/verification.md#current-alpha2-verification-9-october-2026).

- [x] Existing OpenAPI snapshots and workspace tests pass.
- [x] Provider substitution, typed downstream hooks and regeneration have focused tests.
- [x] GraphQL → TypeScript and Rust generation, clean packaged consumers and local-server execution.
- [x] Protobuf → Go official messages, gRPC clients/server interfaces and all streaming modes.
- [x] AsyncAPI → TypeScript message models and Kafka producer/consumer for the documented subset.
- [x] Arazzo → source-resolved sequential TypeScript runners and local integration tests.
- [x] All six inputs expose whole contracts and optional standard block collections.
- [x] Revision/provenance checks, completeness checks, typed hooks and opt-in deterministic symbol planning.
- [x] Complete the frozen pre-output-migration OpenAPI corpus: 2,050 effective passes.
- [ ] Run the corpus against final migrated binaries.
- [x] GraphQL generation through all ten existing npm SDK language plugin packages.
- [ ] Broaden native protocol features and expose other pipelines/general typed hooks through npm.
- [ ] Cap’n Proto → Rust messages/capability RPC using the official toolchain.
- [ ] Schema-level JSONPath overlays.
- [ ] Audited Forge migration and a tested compatibility matrix.
- [ ] Confirm Cap’n Web scope, then implement and test output.
- [ ] Alpha.2 versioning, final packaging, publication and install verification.

See [native pipelines](reference/inputs/native-pipelines.md) for exact feature limits and concrete
implementation/verification requirements for each follow-up.

## Generation planning (unreleased)

The [CLI planner](cli/plan.md) renders terminal lines or exports JSON/HTML without
loading protocol inputs. [Rust planning](rust/planning.md) exposes the resolved
typed package graph and handler declarations. [Node planning](javascript/planning.md)
exposes JavaScript dependencies and callback names; native nodes are opaque
configuration boundaries. Planning does not predict runtime revisions, block
completeness, emitted files or timings. Execution tracing and a full Node native
graph bridge remain follow-up work.

Go and Python GraphQL boundaries: [Go](reference/outputs/graphql-go.md) · [Python](reference/outputs/graphql-python.md).

## GraphQL language boundaries

All SDK languages provide raw operations, flat client APIs and grouped APIs with
explicit custom group mappings. Grouping follows language conventions: Elixir uses
modules, while object-oriented outputs use client methods/accessors. No schema-field
heuristic invents resource groups.

| Language | Custom scalar fallback | Abstract result variants | Runtime model validation |
| --- | --- | --- | --- |
| TypeScript | Configurable input/output types | Selected concrete alternatives | Static types; optional Zod validators |
| Rust | JSON or configured wire types | Selected concrete alternatives | Serde decoding and presence wrappers |
| Go | json.RawMessage | Unsupported; rejects | JSON decoding; required primitives can default to zero |
| Python | Any | Selected typed dictionary alternatives | Static TypedDict annotations |
| PHP | JSON values | Requires selected typename | Generated model decoding/validation |
| Java | JsonNode | Requires selected typename | Generated record decoding/validation |
| C# / dotnet | JsonElement | Unsupported; rejects | Required presence; nonnull values are not revalidated |
| Ruby | Untyped JSON | Selected variants | Generated model validation and RBS signatures |
| Swift | GraphqlJSON | Unsupported; rejects | Codable plus presence/null checks |
| Elixir | JSON terms | Requires selected typename | Generated struct/scalar decoding/validation |

Subscriptions require a distinct transport capability. Only TypeScript currently
accepts an injected subscription transport; other language generators reject subscriptions.
No language in this table has a bundled WebSocket/SSE subscription transport, dynamic
field selection, incremental delivery or runtime custom scalar codecs.
Symfony remains a separate HTTP integration target; adapting the PHP GraphQL transport
to Symfony HttpClient is possible, but generated Symfony GraphQL DI bindings are not implemented.

## GraphQL source organization

All ten SDK client outputs now separate model, operation, client/facade and runtime
code using native module/package conventions. Per-entity files, bounded facade/export
parts and stable filename checks prevent large operation sets from accumulating
in a monolith. The source grouping budget is **128 KiB**; oversized indivisible
declarations are retained with explicit layout diagnostics. This is a grouping
budget rather than a hard limit on all valid GraphQL selections.

Raw, flat and grouped call paths are preserved. Java model imports move from nested
`Client` records to `<package>.models` types. Generated-source customizations using
old paths require migration. See each language guide for its actual file tree.
