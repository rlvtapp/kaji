# Poolster plugin support matrix

Release candidate scope: **0.5.0-rc.1**. This matrix describes usable generation
in this branch, rather than parser availability or future plans.

✅ = implemented for the stated contract; — = no bundled generation support.
A check does not imply support for every feature of a specification. Target-specific
limitations still apply. GraphQL is a separate contract even when its transport is HTTP.

## Output plugins and accepted inputs

| Output plugin | Generated output | OpenAPI / HTTP `Api` | GraphQL operations | Protobuf RPC | AsyncAPI events | Arazzo workflows | Cap’n Proto |
| --- | --- | :---: | :---: | :---: | :---: | :---: | :---: |
| TypeScript | HTTP SDK/models; GraphQL selection types, variables, operations and Fetch transport | ✅ | ✅ | — | — | — | — |
| Zod | TypeScript validators | ✅ | — | — | — | — | — |
| Faker | TypeScript fixtures | ✅ | — | — | — | — | — |
| MSW | HTTP mock handlers | ✅ | — | — | — | — | — |
| Cypress | HTTP test helpers | ✅ | — | — | — | — | — |
| React Query | TypeScript query hooks | ✅ | — | — | — | — | — |
| Vue Query | TypeScript query hooks | ✅ | — | — | — | — | — |
| SWR | TypeScript query hooks | ✅ | — | — | — | — | — |
| Go | Go SDK | ✅ | — | — | — | — | — |
| Rust | Rust SDK | ✅ | — | — | — | — | — |
| Python | Python SDK | ✅ | — | — | — | — | — |
| Java | Java SDK | ✅ | — | — | — | — | — |
| C# | C# SDK | ✅ | — | — | — | — | — |
| .NET | .NET SDK | ✅ | — | — | — | — | — |
| PHP | PHP SDK | ✅ | — | — | — | — | — |
| Symfony | Symfony/PHP package | ✅ | — | — | — | — | — |
| Ruby | Ruby SDK | ✅ | — | — | — | — | — |
| Elixir | Elixir SDK | ✅ | — | — | — | — | — |
| Swift | Swift SDK | ✅ | — | — | — | — | — |
| Rust CLI | Rust command-line client | ✅ | — | — | — | — | — |
| TypeScript CLI | TypeScript command-line client | ✅ | — | — | — | — | — |
| Postman | Collections, environments and examples | ✅ | — | — | — | — | — |
| Terraform | Terraform provider scaffolding | ✅ | — | — | — | — | — |

The existing output families consume HTTP capabilities, directly or through a
previous plugin's generated-model/operation contract. The TypeScript plugin package supports both HTTP and GraphQL. Its `ts::sdk()` /
`ts::types()` generators handle HTTP, while `ts::graphql()` consumes
`GraphqlOperations` and publishes `GraphqlClient`. It does not
make Zod, mocks or query-hook plugins GraphQL-capable automatically. Shared
package assembly, release metadata, ownership and customization are infrastructure,
not additional protocol generators.

## Input providers

| Input format | Parsing / inspection | Usable bundled output |
| --- | :---: | --- |
| OpenAPI | ✅ | HTTP output plugins above |
| GraphQL SDL + operation documents | ✅ | TypeScript through Rust API / CLI |
| Protobuf | ✅ | — |
| AsyncAPI | ✅ | — |
| Arazzo | ✅ | — |
| Cap’n Proto | ✅ | — |

The npm input wrappers expose parsing/inspection. The new GraphQL package pipeline
is not yet exposed through the npm configuration engine. CLI recipes currently
select one input source; Rust plugins can compose multiple typed contracts.

## GraphQL checks

- [x] Validate schema and operation documents together.
- [x] Selection-specific results, aliases, fragments and concrete abstract alternatives.
- [x] Preserve nullability, optional presence, lists and input defaults.
- [x] Generate variables and query/mutation functions with Fetch POST transport.
- [x] Represent GraphQL errors and partial results explicitly.
- [x] Compile generated TypeScript and execute against a local GraphQL server.
- [x] Separate subscription capability with an injected async-iterable transport.
- [ ] Bundle a WebSocket or SSE subscription transport.
- [ ] Custom scalar mappings/codecs (currently `unknown`).
- [ ] Introspection JSON and schema imports.
- [ ] Incremental delivery (`@defer` / `@stream`) and custom executable directives.
- [ ] GraphQL support in Zod, Faker, MSW, Cypress and query-hook outputs.
- [ ] GraphQL generation through the npm configuration engine.

Unsupported output packages succeed with warnings and a structured skipped-plugin
report. An all-skipped run leaves existing files untouched. Invalid input and
unsupported features in an otherwise supported pipeline still fail explicitly.

## Verification and release checks

Recorded implementation verification: 688 Rust tests passed, 0 failed and 130 ignored;
workspace formatting and Clippy passed. Six native GraphQL tests passed with ignored
tests explicitly enabled, including generated compilation and local-server runtime
checks. Ignored tests are not counted as passes. See [verification](verification.md)
for commands and environment requirements. These results precede final RC packaging;
registry publication is a separate check.

- [x] Existing OpenAPI golden snapshots and Go tests pass.
- [x] Provider substitution, downstream typed hooks and regeneration covered.
- [x] Malformed inputs and unsupported GraphQL features covered.
- [ ] Final RC versions, package manifests and publication artifacts verified.
- [ ] RC uploaded and registry versions/installations verified.

## Next pipeline checks

- [ ] Protobuf → Go messages and gRPC clients/servers, including streaming.
- [ ] AsyncAPI → TypeScript message models and Kafka producer/consumer.
- [ ] Arazzo → source-resolved, integration-tested workflow runners.
- [ ] Cap’n Proto → Rust messages/capability RPC using the official toolchain.
- [ ] Schema-level JSONPath overlays.
- [ ] Audited Forge configuration migration and tested compatibility matrix.
- [ ] Confirm Cap’n Web scope, then implement and test its output pipeline.

See [native pipelines](native-pipelines.md) for exact feature limits and concrete
implementation/verification requirements for each follow-up.
