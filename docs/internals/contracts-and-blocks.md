# Contract and block API reference

For the concepts, start with [contracts](contracts.md) and
[building blocks](blocks.md). For code, use [Rust handlers](../plugins/rust/handlers.md).

## Core types

| Type | Responsibility |
| --- | --- |
| `Contract` | Producer/consumer Rust type with stable diagnostic `NAME` |
| `Handle<C>` | Select one provider instance inside a package |
| `Blocks<T>` | Typed block collection with completeness and parent provenance |
| `BuildingBlock<T>` | Value plus `BlockMetadata` |
| `BlockId` | Stable source/entity identity |
| `ContractReference` | Contract name, instance and revision |
| `CollectionState` | Complete, partial or unavailable, with diagnostics |

Contracts can be opaque and expose no blocks. Third parties normally share their
contract types through an independently versioned crate. Parser types belong
inside input providers, not output-plugin interfaces.

## Publication and extraction

- `Provision::of::<C>()` declares publication; `cx.publish(value)` publishes once.
- `publish_with_reference` adds an authoritative instance/revision reference.
- `Requirement::on(handle)` binds a consumer; ambiguous automatic selection fails.
- `extract_blocks::<C, T>(handle, extractor)` derives blocks from the selected whole.
- `on_derived_block` checks matching whole/block provenance, including empty collections.

A transform publishes a new immutable value. Re-extract from its explicit handle;
there is no automatic decomposition of arbitrary custom contracts and no
last-revision selection default. Consumers may intentionally select the original.

## Completeness and compatibility

Standard handlers require complete collections. Partial data requires an explicit
policy; unavailable required collections fail. Complete empty collections are
valid and dispatch zero item callbacks. Nonempty is a separate requirement.

Blocks keep stable IDs across revisions. Parent instance/revision fields are
separate, so unchanged entities can be diffed. Tags support discovery but do not
replace exact type compatibility or wire semantics.

Same-name/different-Rust-type contracts fail before binding, including optional
requirements. This diagnoses incompatible shared contract crate copies.

## Serialization and names

`contract_codec::JsonCodec` is opt-in. Versioned envelopes carry identity,
revision, payload and block provenance/completeness. Unsupported versions or
mismatched names fail. Custom contracts may use another codec or stay opaque.
This foundation is not a completed Node bridge.

[Symbol planning](symbol-planning.md) provides reservation → deterministic
resolution → emission. Existing immediate naming APIs have not all migrated.

## First-party inputs and outputs

Inputs publish protocol contracts and optional standard projections. The
[provider reference](../reference/inputs/input-plugins.md) lists actual availability. HTTP plugins
select whole/models/endpoints through `engine::HttpInput`; views must be complete
and match the selected revision. Protocol-native generators retain their own
whole contracts.

[Output migration reference](../verification/output-contract-migration.md) ·
[Core blocks](../../crates/core/src/blocks.rs) · [Typed hooks](../../crates/core/src/engine/hooks.rs)
