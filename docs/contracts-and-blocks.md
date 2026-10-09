# Contracts and optional building blocks

A contract is a Rust type implementing `poolster_core::engine::Contract` with a
stable `NAME`. It can contain anything its producer and consumers agree on.
There is no required HTTP model, block collection or serialization format.
Third-party producers and consumers normally share their contract types through
an independently versioned Rust crate.

A provider can additionally expose independently consumable building blocks.
`poolster_core::blocks::Blocks<T>` implements `Contract` when `T` implements
`Block`. The block type chooses its own stable `CONTRACT_NAME`. `ModelBlock`
is an initial shared shape contract; protocol-specific blocks remain owned by
those protocols. Shared shapes must preserve nullability and field presence;
wire layouts and execution semantics must not be inferred from shape alone.

Each `BuildingBlock<T>` contains its typed value and metadata:

- `BlockId`: source identity and local identity, independent of generated names.
- `capabilities`: extensible string tags for discovery and filtering.
- `references`: related block IDs with their contract identities.
- `location`: an optional native source coordinate.

Tags do not authorize generation or replace typed contracts. A consumer first
requires the exact `Blocks<T>` contract, then can select tagged elements with
`with_capability()`. Unknown tags cannot silently convert an unsupported wire
protocol into a supported one. Reference resolution and protocol compatibility
remain consumer responsibilities; metadata is not an execution plan.

Providers can publish a whole contract, blocks, or both through the existing
provision/publish APIs. Consumers use the existing typed requirement/context APIs
for either. Intermediate plugins can consume a whole contract and publish blocks
without changing the input provider. Existing graph dependency ordering applies.
An element handler is ordinary iteration over typed blocks inside a plugin;
this API does not introduce a second automatic callback lifecycle.

A workflow-step consumer may generate documentation from exposed steps. A runner
still needs the workflow's dependency and control semantics. A message-model
consumer can generate types independently; a Kafka producer also needs routing
and broker bindings. A RPC-method consumer cannot reconstruct Protobuf or
Cap'n Proto wire semantics from method names and streaming flags.

Opaque custom contracts remain fully supported and need not expose any blocks.
No-capability output combinations retain the existing skipped-plugin reporting
policy. Missing required graph dependencies and invalid supported inputs remain
errors.

Node SDK exposure, recipe configuration of arbitrary third-party block
transformers, automatic reference indexing and broader shared block families
are follow-up integration work; the initial block API is Rust-native.

## Initial protocol projections

- `GraphqlOperations::input_model_blocks(source)` exposes input object shapes as
  `Blocks<ModelBlock>`, preserving field presence, nullability and defaults.
  Schema objects are not presented as selection-specific query results.
- `EventOperations::message_blocks(source)` exposes typed message blocks. An
  explicit input-crate transformer can publish them alongside the whole contract.
- `RpcContract::method_blocks(source)` exposes service/method metadata. Official
  Go generation still consumes the descriptor-set bytes as its wire authority.
- `WorkflowOperations::step_blocks(source)` exposes steps with contextual
  references. Running a workflow still requires its whole control contract.

These are projections, not synchronized mutable views. Editing a method block
alone does not rewrite Protobuf descriptors, and editing a workflow projection
does not automatically update its parent workflow. A transform that changes
execution semantics must publish a coherent authoritative contract explicitly.

## Handler plugins

`poolster_core::engine::hooks::<Language>()` builds an ordinary typed plugin.
Register `on_contract::<C>(provider, handler)` for a whole contract or
`on_block::<T>(provider, handler)` for each `BuildingBlock<T>`. Whole contracts
need not implement Clone or expose blocks. Handlers receive their immutable
input and a `HookContext` for files, workspace, package settings and declared
output publication. Declare output types with `.provides::<C>()` before
publishing; `.handle::<C>()` lets downstream plugins bind that output.

Rust convenience methods are `on_model` (rich `Schema` blocks), `on_endpoint`
(`Operation` blocks), `on_model_shape` (`ModelBlock` shapes), `on_message`,
`on_rpc_method` and `on_workflow_step`. These are typed `on_block` aliases,
not tag-driven guesses. Distinct model representations stay distinct.

```rust,ignore
let extension = hooks::<TypeScript>()
    .on_contract(Some(native_provider.handle()), |whole, cx| {
        // Consume a custom whole contract here.
        Ok(())
    })
    .on_endpoint(Some(endpoint_provider.handle()), |endpoint, cx| {
        // endpoint.value contains the complete HTTP operation.
        Ok(())
    });
```

Handlers run in registration order inside their plugin. The dependency graph
orders plugins, regardless of their position in the package builder. Multiple
handlers for the same contract and provider share one dependency. Conflicting
bindings for one contract type fail rather than selecting a provider silently.
Required absent contracts fail; `on_optional_contract` skips an absent automatic
provider while retaining explicit-provider and ambiguity checks. `on_block`
validates identities before dispatch, including duplicate IDs. Empty published
collections run zero element callbacks normally.

Use `.phase(PluginPhase::Post)` for handlers after language finalization.
Callbacks can emit package files or publish intermediate contracts; they cannot
mutate immutable input contracts in place. A transform explicitly publishes its
new contract. Existing plugins continue to implement `Plugin` directly.

This handler builder currently exposes idiomatic Rust names. JavaScript
`onContract`/`onBlock` factories and Node SDK serialization are not implemented.

## Revision, completeness and derivation rules

Entity IDs remain stable across transforms. `ContractReference` separately carries
contract name, stable instance identity and revision. Providers use
`publish_with_reference` to publish authoritative metadata; opaque contracts can
still use `publish`. Inputs and handlers can read a selected provider's reference.

`extract_blocks::<C,T>(Some(contract_handle), extractor)` is explicitly opt-in.
It derives a collection from exactly that provider and stamps the collection and
items with its reference. A transformer must publish its new whole contract with
an updated revision and wire extraction to that new handle. Nothing implicitly
rewrites arbitrary contracts. Multiple providers of the same contract type require
explicit selection; no execution-order or last-revision default exists.

`on_derived_block(whole_handle, blocks_handle, handler)` requires both views,
checks parent instance/revision before dispatch, and rejects mismatches even for
empty collections. Ordinary `on_block` is suitable for consumers requiring only
blocks; bind its provider explicitly when selecting a particular transform chain.
Consuming the original provider intentionally remains valid.

Collections report `Complete`, `Partial { diagnostics }` or
`Unavailable { diagnostics }`. Standard block handlers require complete data.
`on_partial_block` explicitly accepts partial collections but rejects unavailable
ones. `on_nonempty_block` separately requires nonempty complete collections.
A complete empty collection is valid; availability is not inferred from length.

Stable-name/different-Rust-type collisions fail before dependency binding,
including optional requirements. This diagnoses incompatible contract crate
copies rather than presenting them as absent capabilities.

The opt-in `contract_codec::JsonCodec` carries a versioned envelope with contract
instance/revision and serialized payload, including collection completeness and
block provenance. Unsupported envelope versions and mismatched names are errors.
Custom contracts can provide another `ContractCodec` or stay opaque. This is a
codec foundation, not a completed Node plugin bridge.

Deterministic symbol planning is available through the explicit reservation →
resolution → emission dependency chain described in [symbol planning](symbol-planning.md).
Existing built-in immediate symbol APIs have not been silently converted; new
planner consumers must reserve all their names before emitting files.

## First-party output consumers

HTTP output plugin entry points now expose `.input`, `.input_models` and
`.input_endpoints` through the shared `engine::HttpInput` bridge. Selected views
rebind API, security and derived semantics together; explicit blocks must match
the selected whole revision and be complete. Existing calls retain their legacy
context fallback. Protocol-native generators keep their own whole contracts.
See [output coverage and boundaries](output-contract-migration.md).
