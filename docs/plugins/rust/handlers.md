# Handle contracts and building blocks

← [Rust plugins](README.md)

Use a whole-contract handler when the producer's complete value is what you need.

```rust
use poolster::prelude::*;
use poolster_core::native::GraphqlOperations;
use poolster_plugin_typescript::TypeScript;

let report = hooks::<TypeScript>()
    .on_contract::<GraphqlOperations>(Some(input.handle()), |contract, cx| {
        cx.files.emit(GeneratedFile::new(
            "operations.txt",
            contract.operations.iter().map(|op| op.name.as_str())
                .collect::<Vec<_>>().join("\n"),
        )?)?;
        Ok(())
    });
```

Add `report` to the same package as its input provider. The dependency graph runs
that provider first.

## Handle exposed blocks

```rust
let report = hooks::<TypeScript>()
    .on_endpoint(Some(endpoints.handle()), |block, cx| {
        // block.value is the typed HTTP operation.
        // block.metadata.id and block.metadata.parent carry identity and provenance.
        Ok(())
    });
```

The producer must publish the matching block collection. Whole-only contracts
work normally without blocks.

| Handler | Receives |
| --- | --- |
| `on_contract::<C>` | A whole contract |
| `on_block::<T>` | Each exposed block of type T |
| `on_model` / `on_endpoint` | Rich HTTP models / operations |
| `on_model_shape` | Shared model shapes |
| `on_message` / `on_rpc_method` / `on_workflow_step` | Protocol-specific blocks |

One plugin can register several handlers. Complete collections are required by
default; partial or nonempty policies must be chosen explicitly.

**Next:** [Custom contracts](contracts.md) · [Block rules](../../internals/blocks.md)
