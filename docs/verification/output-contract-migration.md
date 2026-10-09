# Output contracts and blocks

Updated 9 October 2026 for the unreleased `alpha-2` working tree.

First-party HTTP output plugin entry points can select an authoritative
`AdaptedApi` contract and optional complete `Blocks<Schema>` and
`Blocks<Operation>` collections. They use the shared `engine::HttpInput` bridge.
Existing `sdk()` calls and OpenAPI recipes remain valid through the legacy context
fallback. Standalone renderer functions taking an `Api` remain ordinary renderers;
they do not become plugin graph nodes.

## What migrated

| Output family | Typed HTTP selection |
| --- | --- |
| TypeScript | SDK/types; independent models, transport, operations and client; Zod, Faker, MSW, Cypress, React/Vue Query, SWR and operation tests |
| TypeScript CLI | Selected API and model/endpoint blocks |
| Go | SDK, independent provider, roundtrips, OAuth, webhooks and operation tests; selected API retained through finalization |
| Rust | SDK, independent provider, roundtrips, OAuth, webhooks and operation tests; selected API retained through finalization |
| Java, C#, Swift | SDKs and HTTP-dependent OAuth/webhook/operation test plugins |
| .NET | Reexports the C# implementation; no second generator |
| Rust CLI | Selected API and model/endpoint blocks |
| Python | SDK, webhook output, roundtrip and operation tests |
| Ruby | SDK, roundtrip and operation tests; OAuth/webhooks already consume actual generated model symbols |
| PHP, Elixir | SDK, OAuth, webhook output and operation tests |
| Symfony | Integration package generator |
| Postman | Examples, collection and environment |
| Terraform | SDK scaffolding, entity planning and typed provider; release scaffolding uses explicit package settings |
| HTTP mock | Server package and fixtures |
| API reference | Selected HTTP contract and blocks |
| Release metadata | Protocol-neutral behavior retained; explicit HTTP input can supply the selected version |

Protocol-native generators keep their own contracts: GraphQL uses
`GraphqlOperations`, Kafka uses `EventOperations`, workflows use
`WorkflowOperations`, and Go gRPC uses `RpcContract` descriptor bytes. They do not
need HTTP blocks to participate. Custom generators may consume opaque whole
contracts without decomposing them.

## Select a contract or projection

HTTP-dependent plugins expose these builders:

```rust,ignore
let output = python::sdk()
    .input(http_provider.handle())
    .input_models(model_extractor.handle())
    .input_endpoints(endpoint_extractor.handle());
```

`.input()` selects the whole `AdaptedApi`; model and endpoint selections are
optional. A selected block collection replaces its corresponding renderer view,
while the whole contract supplies package/API context and reusable security.
No automatic block binding chooses an earlier provider's projection.

The bridge declares graph requirements, resolves the selected input, validates
block identities/completeness and checks the parent contract instance/revision.
It then derives semantics from the selected view, applies package version and
HTTP idempotency settings, and renders with matching API/security/semantics.
Explicitly selected blocks require authoritative whole-contract provenance.
Opaque whole contracts work without blocks or provenance.

Ambiguous automatic providers and missing explicit providers fail at graph
planning. Partial/unavailable collections and mismatched parents fail before the
consumer emits files. Consumers intentionally selecting an original contract
remain valid; transformations do not silently replace every provider.

Python/Ruby `models_from(&sdk)` test helpers inherit that SDK's HTTP selection.
Other consumers that accept only an output handle still require their HTTP
selection explicitly when they also need HTTP source data; an output symbol
handle does not silently choose a source revision.

Defaults remain HTTP generators. Explicit selection permits typed HTTP graph
execution; it does not make an HTTP plugin accept GraphQL, Kafka or RPC inputs.
The existing incompatible-output skipped-package policy remains in place.

## Verification and remaining boundaries

Focused tests compare legacy and selected-contract bytes, reverse provider
registration order, reject stale/incomplete blocks, exercise selected downstream
symbols, and regenerate packages. External TypeScript checks compile selected
SDK/helper/CLI output. Fresh native GraphQL, Kafka and workflow suites were also
run explicitly. The full workspace passes **788 tests**, with **0 failures and 138 ignored**;
formatting and workspace Clippy pass. [Recorded evidence](../verification-results/output-contracts-2026-10-09.json)
includes source provenance and the three unchanged source-size baseline violations.
See [verification](verification.md).

The earlier frozen 205×10 corpus uses pre-migration binaries. Its completion
cannot be presented as a corpus validation of this migration. A post-migration
frozen run is a separate verification step.

This migration does **not** finish global deterministic naming. Existing native
symbol policies and published generated symbols remain in use; shared
reserve → resolve → emit planning is still opt-in. It also does not expose typed
hooks through Node, add multi-input recipes, or make every language support every
protocol. See [remaining native work](../reference/inputs/native-pipelines.md#remaining-work) and
[symbol planning](../internals/symbol-planning.md).
