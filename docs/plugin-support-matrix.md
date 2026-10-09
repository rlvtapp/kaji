# Poolster plugin support matrix

Audited **9 October 2026**, against the current **`alpha-2` working tree**.
Manifests still use `0.5.0-alpha.1`; the additions below are unreleased alpha.2
work. This matrix describes implemented generation, not parser availability.
For remaining work, start with the [native pipeline backlog](native-pipelines.md#remaining-work).

✅ = implemented for the stated contract; — = no bundled generation support.
A check does not imply support for every feature of a specification. Target-specific
limitations still apply. GraphQL is a separate contract even when its transport is HTTP.

## Output plugins and accepted inputs

| Output plugin | Generated output | OpenAPI / HTTP `Api` | GraphQL operations | Protobuf RPC | AsyncAPI events | Arazzo workflows | Cap’n Proto |
| --- | --- | :---: | :---: | :---: | :---: | :---: | :---: |
| TypeScript | HTTP SDK/models; GraphQL client; Kafka message/client package; workflow runner | ✅ | ✅ | — | ✅ | ✅ | — |
| Zod | TypeScript validators | ✅ | — | — | — | — | — |
| Faker | TypeScript fixtures | ✅ | — | — | — | — | — |
| MSW | HTTP mock handlers | ✅ | — | — | — | — | — |
| Cypress | HTTP test helpers | ✅ | — | — | — | — | — |
| React Query | TypeScript query hooks | ✅ | — | — | — | — | — |
| Vue Query | TypeScript query hooks | ✅ | — | — | — | — | — |
| SWR | TypeScript query hooks | ✅ | — | — | — | — | — |
| Go | HTTP SDK; official Protobuf messages and gRPC clients/server interfaces | ✅ | — | ✅ | — | — | — |
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

The TypeScript output package contains several generators: `ts::sdk()` /
`ts::types()` for HTTP, `ts::graphql()` for `GraphqlOperations`, `ts::asyncapi()`
for Kafka `EventOperations`, and `ts::workflow()` for `WorkflowOperations`.
These are generators within one plugin package, not separate language packages.
Go similarly provides its HTTP SDK and `go::grpc()` for `RpcContract`.

Zod, mocks and query-hook plugins still consume HTTP-generated contracts; adding a
native TypeScript generator does not automatically make those helpers support it.
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
| GraphQL SDL + operation documents | ✅ | TypeScript through Rust API / CLI |
| Protobuf proto2/proto3 | ✅ | Go messages and gRPC clients/server interfaces |
| AsyncAPI 2.6 / 3.0 / 3.1 | ✅ | TypeScript Kafka for the supported 3.0/3.1 subset |
| Arazzo 1.0.0 / 1.0.1 / 1.1.0 | ✅ | TypeScript sequential runners with resolved local OpenAPI sources |
| Cap’n Proto | ✅ | — |

The npm input wrappers expose parsing/inspection. The native package pipelines
are exposed through Rust / CLI, not yet through the npm configuration engine. CLI recipes currently
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

Post-migration alpha.2 workspace check: **788 passed, 0 failed, 138 ignored**;
formatting and workspace Clippy with warnings denied passed. Ignored tests are not passes.
Selected external GraphQL, gRPC, Kafka and workflow integration tests were also
run explicitly and passed; commands and boundaries are in [verification](verification.md).

The pre-output-migration frozen-build **205 specs × 10 HTTP SDK targets** sweep
is running. The post-migration full workspace check passed separately.
It includes deterministic regeneration. Results are pending; the earlier
[2,050-pass record](guru-compatibility.md) combines runs across fixes and is not
proof of this alpha.2 working tree. The supplemental-reference regeneration harness has been fixed and tested;
DigitalOcean passes the normal TypeScript/Go runner with unchanged output hashes.
Original harness failures remain in the audit trail; other language retries continue.

- [x] Existing OpenAPI snapshots and workspace tests pass.
- [x] Provider substitution, typed downstream hooks and regeneration have focused tests.
- [x] GraphQL → TypeScript generation and local-server execution.
- [x] Protobuf → Go official messages, gRPC clients/server interfaces and all streaming modes.
- [x] AsyncAPI → TypeScript message models and Kafka producer/consumer for the documented subset.
- [x] Arazzo → source-resolved sequential TypeScript runners and local integration tests.
- [x] All six inputs expose whole contracts and optional standard block collections.
- [x] Revision/provenance checks, completeness checks, typed hooks and opt-in deterministic symbol planning.
- [ ] Finish and record the fresh alpha.2 frozen-build OpenAPI corpus sweep.
- [ ] Broaden native protocol features and expose generation through the npm engine.
- [ ] Cap’n Proto → Rust messages/capability RPC using the official toolchain.
- [ ] Schema-level JSONPath overlays.
- [ ] Audited Forge migration and a tested compatibility matrix.
- [ ] Confirm Cap’n Web scope, then implement and test output.
- [ ] Alpha.2 versioning, final packaging, publication and install verification.

See [native pipelines](native-pipelines.md) for exact feature limits and concrete
implementation/verification requirements for each follow-up.
