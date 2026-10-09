# Plugin hook reference

For a walkthrough, choose [JavaScript plugin authoring](../plugins/javascript/README.md)
or [Rust plugin authoring](../plugins/rust/README.md).

## JavaScript hooks

| API | Boundary |
| --- | --- |
| `transformApi(api)` | Normalize/change the HTTP API before native rendering |
| `generate(ctx)` | Run once after native rendering |
| `schema(schema, ctx)` | Visit each normalized HTTP schema |
| `operation(operation, ctx)` | Visit each normalized HTTP operation |
| `phase: 'post'` | Run after JS Generate plugins |
| `provides`, `requireContract`, `ctx.publish` | Exchange process-local JS contracts |

[Ordering and examples](../plugins/javascript/contracts.md) ·
[Exact signatures](../../packages/npm/sdk/index.d.ts)

## Rust handlers

| API | Input |
| --- | --- |
| `on_contract::<C>` | Selected whole contract |
| `on_optional_contract::<C>` | Optional whole contract |
| `on_block::<T>` | Complete typed block collection |
| `on_partial_block::<T>` | Explicitly accepted partial collection |
| `on_nonempty_block::<T>` | Nonempty complete collection |
| `on_derived_block` | Blocks and whole contract with matching provenance |
| `on_model`, `on_endpoint`, `on_model_shape` | Standard model/HTTP projections |
| `on_message`, `on_rpc_method`, `on_workflow_step` | Protocol projections |

[Handler example](../plugins/rust/handlers.md) · [Contract/block rules](contracts-and-blocks.md)

## Rust plugin and language interfaces

`Plugin<L>` declares `requires`/`provides` and implements `generate`.
`phase()` chooses Generate or Post. Post uses the same method, not a `post()` callback.

`Language` defines settings/workspace and `finalize`, `bundle_middleware`,
`finalize_files`. Finalization runs once before Post.

[Full interface reference](typed-plugins.md) · [Lifecycle diagrams](lifecycle.md)

The JavaScript and Rust interfaces are separate. General JS `onContract`/`onBlock`
and automatic native contract serialization are not implemented.
