# Poolster plugin support matrix

Audited **9 October 2026**, against the unreleased alpha.2 implementation in this checkout.
Manifests still use `0.5.0-alpha.1`; the additions below are unreleased alpha.2
work. This matrix describes implemented generation, not parser availability.
For remaining work, start with the [native pipeline backlog](native-pipelines.md#remaining-work).

GraphQL React Query, Vue Query, SWR, Zod, Faker, MSW and Cypress integrations now
have separate generated compilation and runtime checks. These checks establish
the documented GraphQL subset, rather than every existing HTTP plugin option.
See the [integration guide](graphql-integrations.md) for API and limitations.

GraphQL client work is verified for TypeScript and Rust, including separate
input/output scalar mappings. Unmapped Rust custom scalars retain JSON values. Both are wired through the CLI and existing npm language plugin packages.
Packaged clients compile and execute against local GraphQL servers. These changes
are not yet published; see the [TypeScript guide](graphql-typescript.md),
[Rust guide](graphql-rust.md) and [verification](verification.md#graphql-client-completion-checks).

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
| Go | HTTP SDK; official Protobuf messages and gRPC clients/server interfaces | ✅ | — | ✅ | — | — | — |
| Rust | HTTP SDK; selection-specific GraphQL query/mutation client | ✅ | ✅ | — | — | — | — |
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

See [output migration coverage](output-contract-migration.md) for typed HTTP
selection, block consumption, finalization and compatibility boundaries.

## Input providers

| Input format | Parsing / inspection | Usable bundled output |
| --- | :---: | --- |
| OpenAPI | ✅ | HTTP output plugins above |
| GraphQL SDL + operation documents | ✅ | TypeScript and Rust through Rust API, CLI and npm entry points |
| Protobuf proto2/proto3 | ✅ | Go messages and gRPC clients/server interfaces |
| AsyncAPI 2.6 / 3.0 / 3.1 | ✅ | TypeScript Kafka for the supported 3.0/3.1 subset |
| Arazzo 1.0.0 / 1.0.1 / 1.1.0 | ✅ | TypeScript sequential runners with resolved local OpenAPI sources |
| Cap’n Proto | ✅ | — |

The npm input wrappers expose parsing/inspection and GraphQL client generation
through the existing TypeScript/Rust language packages. Other native package
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
- [ ] GraphQL support in Zod, Faker, MSW, Cypress and query-hook outputs.
- [ ] GraphQL generation through the npm configuration engine.

Unsupported output packages succeed with warnings and a structured skipped-plugin
report. An all-skipped run leaves existing files untouched. Invalid input and
unsupported features in an otherwise supported pipeline still fail explicitly.

## Verification and release checks

Current alpha.2 workspace check: **816 passed, 0 failed, 143 ignored**;
formatting and workspace Clippy with warnings denied passed. Ignored tests are not passes.
Selected external GraphQL, gRPC, Kafka and workflow integration tests were also
run explicitly and passed; commands and boundaries are in [verification](verification.md).

The pre-output-migration frozen-build **205 specs × 10 HTTP SDK targets** sweep
is complete: **2,050 effective passes**, including deterministic regeneration,
after audited infrastructure retries. PHP used native syntax lint and emitted
deprecation warnings in 140 contracts. These results do not establish endpoint
runtime behavior or corpus coverage of the later output migration. The migrated
workspace passed separately; final migrated-binary corpus coverage and clean
installation checks of release artifacts remain open. Original infrastructure
failures and successful retries remain in the audit trail. See
[current verification](verification.md#current-alpha2-verification-9-october-2026).

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
- [x] GraphQL generation through existing TypeScript/Rust npm plugin packages.
- [ ] Broaden native protocol features and expose other pipelines/general typed hooks through npm.
- [ ] Cap’n Proto → Rust messages/capability RPC using the official toolchain.
- [ ] Schema-level JSONPath overlays.
- [ ] Audited Forge migration and a tested compatibility matrix.
- [ ] Confirm Cap’n Web scope, then implement and test output.
- [ ] Alpha.2 versioning, final packaging, publication and install verification.

See [native pipelines](native-pipelines.md) for exact feature limits and concrete
implementation/verification requirements for each follow-up.
