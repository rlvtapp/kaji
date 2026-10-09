# Define a custom contract

← [Rust plugins](README.md)

Producer and consumer share a Rust type, usually from a small contract crate.

```rust
use poolster_core::engine::Contract;

pub struct Inventory {
    pub names: Vec<String>,
}
impl Contract for Inventory {
    const NAME: &'static str = "example.inventory.v1";
}
```

## Publish and consume

| Producer | Consumer |
| --- | --- |
| Declare `Provision::of::<Inventory>()` | Declare `Requirement::on(handle)` |
| Call `cx.publish(Inventory { names })` | Read `cx.inputs.get::<Inventory>()?` |
| Expose a typed handle using `Meta` | Bind the handle to select the provider |

Store one `Meta` per plugin instance. A missing declaration, missing publication
or duplicate publication is an error.

## Transform a contract

Read the original and publish a new value. Contracts are immutable after
publication. Bind consumers to the transformed provider explicitly; there is
no last-writer-wins rule.

If you expose derived blocks, extract them from the selected transformed
contract and carry its instance/revision. A derived handler checks that the whole
contract and blocks belong to the same revision.

Custom contracts can remain opaque. Blocks and serialization are optional.
Never force RPC, events or workflows into an HTTP endpoint contract solely to
reuse a generator.

**Next:** [Interfaces](interfaces.md) · [Identity and revisions](../../internals/contracts.md)
