# Explicit symbol planning

`poolster_core::symbols` provides an opt-in reserve → resolve → emit lifecycle.
A request identifies a source entity with `BlockId`, an output target, a relative
module, and a preferred identifier. Target renderers apply their own casing and
keyword policy before reservation; core does not pretend every language has the
same naming rules.

```rust
use poolster_core::{blocks::BlockId, symbols::{SymbolRequest, SymbolRequests}};
let request = SymbolRequest {
    entity: BlockId { source: "example:orders".into(), local: "#/messages/Created".into() },
    target: "typescript".into(), module: "models".into(), preferred: "OrderCreated".into(),
};
let mut names = SymbolRequests::new();
names.reserve_name("typescript", "models", "Array")?;
let key = names.reserve(request)?;
names.resolve()?;
let actual = names.symbol(&key)?;
// Emit `actual.name` in `actual.key.module`; publish it to downstream consumers.
```

Reserve every competing name before resolution. Resolution sorts stable entity
IDs rather than using plugin registration order. It first protects naturally
preferred names, then allocates `_2`, `_3`, and later suffixes without stealing a
name such as another entity's explicit `Order_2`. Runtime/imported names can be
reserved in the same namespace. Identifier comparison is case-sensitive by default;
`SymbolRules` can select case-insensitive comparison per target before reservations.
Module separators are canonicalized, unsafe paths reject, and case-only module
aliases reject to keep generated output portable.

Resolution freezes the allocator. Late reservations, reserved names, and rule
changes fail; emission lookups before resolution or for unreserved entities fail.
Repeated identical reservations share a symbol. Conflicting preferred names for
the same entity/target/module fail rather than silently redefining its identity.
Stable source identities should be document URIs or other persistent IDs, not
plugin labels or generated symbol names. Distinct roles for one source entity can
use stable child IDs, such as `#/operations/Read/variables` and `/result`.

The existing typed plugin graph can enforce the lifecycle:

1. `reserve_symbol_requests(requests)` publishes an immutable `SymbolPlan`.
2. `resolve_symbol_requests(Some(reservations.handle()))` consumes that plan and
   publishes `ResolvedSymbols`.
3. An emitter requires the resolver's handle and uses actual symbols from that
   table. Its generated-output contract can publish those symbols for later hooks.

Registration order does not control this chain. Core's integration test registers
the emitter before the resolver and reservation provider; typed requirements still
run reserve → resolve → emit. Custom collectors can read whole contracts or optional
blocks and publish a `SymbolPlan` directly. An opaque input does not have to expose
blocks to participate.

This is a real opt-in graph pattern, **not a new global engine phase**. Existing
TypeScript/OpenAPI generators and `Workspace::declare` retain their established
naming behavior. Migrating those generators requires collecting all competing
requests before they emit files; immediate declarations cannot be silently renamed
after other plugins have already generated imports. Published symbols remain the
source of truth for downstream consumers during that migration.
