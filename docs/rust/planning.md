# Inspect the configured Rust graph

← [Rust SDK](README.md)

After constructing a `ProfileSet`, inspect it before generation:

```rust
let plan = profiles.plan(true); // true: native input mode; false: legacy HTTP mode
println!("{}", plan.to_text());
let json = plan.to_json()?;
```

`Packages::plan(native)` exposes the same API at the core package level. The
result is `GenerationPlan`, with public serializable fields. Use those fields or
the versioned JSON to build your own viewer. `to_html()` is an optional standalone
renderer; callers do not need it to consume the plan.

## What the plan exposes

Each package has local plugin IDs, phases, resolved order, requirements,
publications and provider connections. Typed hook plugins expose their handler
labels; other plugins can implement `Plugin::plan_handlers` to describe their
entry points. A default `generate` label does not reveal internal control flow.

Resolution uses generation's provider selection, type identity, phase and cycle
checks. Invalid graphs have `status: "invalid"` and a diagnostic. Incompatible
native packages have `status: "skipped"`. No provider loading or generator callback
runs. `status: "ready"` means declarations resolve, not that input documents or
generated packages have passed validation.

Block collection contracts appear as typed edges. Their actual completeness,
provenance, values and revision agreement are known after publication and are not
predicted by this API. Contract entity IDs and runtime revisions are distinct
from the plan's local plugin IDs.

Finalizers and ownership are lifecycle boundaries rather than predicted emitted
files. Read [the CLI envelope](../cli/plan.md) for field interpretation and limits.
