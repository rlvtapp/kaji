# Building blocks

← [Internals](README.md)

A producer may expose pieces of its contract for focused consumers.

| Block | Example use |
| --- | --- |
| Model | Generate types or validation |
| HTTP endpoint | Generate request functions |
| GraphQL operation | Generate selected result types and calls |
| Event message | Generate message fixtures or models |
| RPC method | Inspect method/streaming metadata |
| Workflow step | Generate a workflow report |

These are examples of useful pieces, not a promise that every input publishes
every block kind. Use the [input reference](../reference/inputs/input-plugins.md) for actual projections.

## Each block carries

- A stable entity ID and its typed value.
- A parent contract reference, including instance and revision.
- Optional capabilities, related references and source location.

Tags help discover or filter blocks. They do not replace type compatibility or
supply missing wire semantics.

## Collections carry completeness

| State | Default block handler |
| --- | --- |
| Complete | Runs; an empty collection invokes zero item handlers |
| Partial with diagnostics | Requires an explicit partial-data policy |
| Unavailable with diagnostics | Rejected when required |

An extractor is opt-in and bound to one whole-contract provider. Transforming a
contract does not automatically rewrite every existing block collection. Bind a
new extractor to the new contract, then consume that matching pair.

A whole-only input remains usable through `on_contract`.

**Next:** [Rust handlers](../plugins/rust/handlers.md) ·
[Complete block API reference](contracts-and-blocks.md)
