# Language plugin implementation specification

**Version 1 · 10 October 2026 · internal architecture target.**

This defines how we organize and maintain Poolster's language output plugins.
The [generated SDK specification](generated-sdk.md) defines the code they emit.
Neither document asserts that every existing module already follows the target.

## 1. Boundaries and ownership

| Layer | Owns | Must not own |
| --- | --- | --- |
| Input provider | Parsing, native-document retention, validation, protocol contracts and opt-in block extraction. | Language syntax or output package layout. |
| Core contracts | Poolster-owned wire semantics, stable identities, revisions, presence/nullability and versioned codecs where supported. | Parser-library types or target-specific source syntax. |
| Core engine | Dependency planning, provider selection, typed hooks, completeness/provenance checks, ownership, customization and release machinery. | One generator's naming or business logic. |
| Language backend | Target names/imports, type rendering, source emission, package assembly and formatting policy. | Pretending every native contract is an HTTP API. |
| Protocol generator | Map one supported protocol contract to that language's models, operations and runtime surface. | Reparse native documents or duplicate unrelated protocol generators. |
| Companion / adapter | Consume declared generated-symbol contracts; emit hooks, validators, mocks or framework integration. | Guess SDK paths/names or duplicate its operation implementation. |

A language package may contain HTTP, GraphQL and other generators. These remain
one package with multiple capabilities, not separate language plugins that compete
for the same namespace and manifest. Their native contracts remain distinct.
Symfony is a PHP integration backend: reuse PHP models/operations and add only
framework-specific package, transport and DI artifacts.

## 2. Cohesive public contract modules

HTTP types currently live in `ast`, `adapter` and related root exports; GraphQL
and other native types live in `native`. That separation reflects real semantics,
but the public surface should present discoverable protocol families together:

```text
poolster_core::contracts
  common       shared models where semantics agree, identity and provenance
  http         AdaptedApi, Api, Schema, Operation, security/media types
  graphql      GraphqlOperations, selections, variables, incremental contracts
  rpc          services/methods/messages and streaming contracts
  events       message/channel/broker binding contracts
  workflows    source-resolved workflow contracts
  capabilities capability-RPC contracts where these differ from ordinary RPC
```

This import facade is now available through `poolster_core::contracts`. It uses
**re-exports of the original types**, not replacement wrapper structs or copied
definitions. Definitions remain at their existing paths. `capabilities` currently
exports owned Cap’n Proto block metadata; complete native documents remain input-owned. Keep existing paths compatible during migration;
`TypeId`, contract `NAME`, serialized payloads and revision behavior must remain
unchanged. Document intentional breaking changes separately.

Shared models belong in `common` only when presence, nullability, defaults,
references and wire representation genuinely agree. HTTP status/media/security,
GraphQL selection/error/incremental semantics, RPC streaming, broker bindings and
workflow source resolution remain protocol-specific. A neat folder tree is not a
reason to flatten their contracts.

Native parser documents may be retained by the input package for unrepresented
details. Output interfaces must still use Poolster-owned types. If generation
needs an unrepresented semantic, extend/version the contract or declare it
unsupported; do not reach into an Apollo/Prost/parser object from the renderer.

Custom third-party contracts remain supported without registration in a closed
first-party enum. A whole contract can be opaque, with no blocks. Re-exporting
first-party types does not change optional decomposition or typed dispatch.

## 3. One predictable language crate layout

Use the following structure when a language has multiple protocols. Omit unused
parts; do not create empty placeholder modules or mechanically split tiny helpers.

```text
src/
  lib.rs                    public exports and module declarations
  package.rs                Language, Settings, PackageExt and package factory
  contracts.rs              public generated-artifact contracts for companions
  shared/
    names.rs                target casing, reserved words, collision requests
    types.rs                target syntax for genuinely shared model semantics
    imports.rs              resolved imports/exports and dependency rules
    files.rs                artifact paths and source layout policy
    formatting.rs           pinned formatter policy and source diagnostics
    manifest.rs             one package manifest assembly path
  http/
    mod.rs                  HTTP factory/settings and declared requirements
    models.rs               HTTP model lowering and rendering
    operations.rs           HTTP request/response generation
    client.rs               raw/flat/idiomatic surface wrappers
    runtime/                transport, errors, auth, streaming, codecs
    features/               pagination, retry, OAuth, webhooks, etc.
  graphql/
    mod.rs                  GraphQL factories/settings and requirements
    models.rs               inputs, selections and abstract variants
    operations.rs           fixed-document operation wrappers
    client.rs               raw/flat/idiomatic surface wrappers
    scalars.rs              direction-specific scalar mapping/codec plans
    runtime/                HTTP execution, envelopes, SSE and incremental templates
    incremental/            GraphQL incremental plans and emission, if supported
  rpc/                      only if this language supports native RPC output
  events/                   only if this language supports event output
  workflows/                only if this language supports workflow output
  integrations/             framework-specific adapters, not copied SDKs
  templates/                source templates organized by protocol/role
  tests/                    focused unit tests close to implementation
```

Keep a protocol's implementation together: GraphQL models, operations, client
styles, scalar helpers, SSE subscriptions and incremental delivery belong under
`graphql/`, including their runtime templates. Incremental execution may use its
own typed capability contract without becoming an unrelated language backend.
The public crate root can re-export compatible factories and artifact contracts.
General target naming/imports/package policy stay outside the protocol directory;
protocol-specific companion code belongs under its owning integration's GraphQL
module.

Native integration probes, pinned fixtures and consumer packages belong under
crate-level `tests/`. Keep generated runtime code in templates or structured
renderers, not in test files. Source templates need their own formatting checks;
`cargo fmt` checks the generator's Rust, not the PHP/Java/Swift strings it emits.

`lib.rs` should explain the crate and expose its API. It must not contain thousands
of lines of HTTP rendering plus an unrelated GraphQL implementation. Existing
public factories/re-exports remain compatible when implementation files move.

New authored Rust implementation files SHOULD stay below **400 lines**, matching
the repository's source-size audit. Existing oversized files must not grow without
an explicit audit exception; reduce them at cohesive boundaries. This limit differs
from the generated-source **128 KiB** budget. Do not hide oversized source in one
long string or divide modules using meaningless numeric filenames.

## 4. Shared backend, protocol-specific plans

Reuse target mechanics: identifier validation, name reservation/resolution, literal
escaping, imports, file ownership, package assembly, formatting and style wrappers.
Reuse model syntax only when its semantics match. For example, an optional HTTP
field and a conditionally selected GraphQL field may share a presence wrapper but
still require different lowering and validation.

Each protocol generator SHOULD follow this staged flow:

```text
selected contract / validated blocks
  → protocol semantic plan
  → target name and artifact reservations
  → deterministic symbol/path resolution
  → target model/operation/runtime emission
  → generated-symbol contract publication
  → companions and package finalization
  → customization, formatting and final-source diagnostics
  → owned output tree
```

This is a responsibility model over the existing dependency graph, not a second
scheduler. Preserve existing hook order and release/customization behavior when
introducing stages; any ordering change needs a focused regression.

The semantic plan preserves native identity and wire meaning. Target source plans
may describe symbols, imports and files; they must not become a universal HTTP
operation model. Share abstractions after at least two working implementations
show matching needs, rather than designing every protocol into one generic plan.

## 5. Plugin API and generated-symbol contracts

A generator declares its exact requirements, provisions and supported capability
combinations. Explicit provider handles select input revisions; do not choose the
first contract with the right name or read undeclared engine state.

A generated-symbol contract should give companions enough information to avoid
reconstructing output from operation IDs:

| Metadata | Requirement |
| --- | --- |
| Source identity | Stable contract/entity identity and authoritative revision where relevant. |
| Artifact identity | Stable role, relative path and ownership, independent of process-local IDs. |
| Public symbols | Actual resolved names, namespace/module and public import path. |
| Operation surface | Variable/result symbols, raw callable, selected style and grouped mappings. |
| Runtime ABI | Required execution context/transport interface and supported capabilities. |
| Dependencies | Required artifacts and public/private export information. |

These contracts are language-owned, publicly discoverable and independently
versioned when their ABI changes. Do not publish parser-library types. A type-only
companion must not require a particular HTTP transport unless it actually uses it.
Companions use selected providers, so substitution must preserve the declared ABI
or fail clearly before files are emitted.

Per-contract and per-block handlers remain engine hooks. Inputs without blocks
still work through whole-contract handlers. Consumers of derived blocks must
check required completeness and whole/block revision agreement. Contract updates
must not leave a handler quietly consuming old projections.

## 6. Package assembly and source quality

Only one assembler owns each package manifest and public entry point. Generators
and companions contribute typed requests/artifacts; finalization merges compatible
requirements and rejects conflicting ones. Two plugins cannot silently overwrite
a file, namespace or dependency declaration.

Naming uses reserve → deterministic resolve → emit. Shared name services must not
make output depend on plugin execution order. Finalizers must preserve ownership
and user extension files, validate resolved imports, apply documented source
customizations and run the target formatting/size policy on final source.

Raw, flat and idiomatic APIs are wrappers over shared execution. They must not
be separate renderers that slowly diverge in request serialization, validation or
error handling. Transport/cancellation and protocol capability choices remain
independent of facade style. Configuration must reach the same generator through
Rust composition, CLI recipes and npm wherever that entrypoint supports it.

## 7. Maintenance and acceptance rules

A language plugin change must preserve these invariants:

- [ ] Public import paths/factories stay compatible or have an explicit migration.
- [ ] Contract re-exports keep type identity, diagnostic names and serialization.
- [ ] No parser dependency enters output-plugin interfaces.
- [ ] Protocol semantics remain distinct and testable.
- [ ] Shared helpers have clear owners; no copied casing/escaping/import rules.
- [ ] Provider substitution and hook/provenance behavior still work.
- [ ] Manifest and file ownership remain deterministic under plugin reordering.
- [ ] All supported API styles use the same execution and result policy.
- [ ] Small and large fixtures cover formatting, paths, size diagnostics and
  regeneration; clean generated consumers compile and execute.
- [ ] Existing HTTP behavior and native protocol checks pass; ignored tests and
  external-tool setup failures are recorded separately from successful checks.

## 8. Migration sequence

1. Audit each language's current modules and dependency/public surfaces; record
   findings in a conformance ledger rather than claiming uniform architecture.
2. Add cohesive core contract re-exports with identity/serialization tests. Keep
   HTTP/GraphQL definitions and old imports compatible.
3. Move large HTTP renderers out of `lib.rs`; group protocol modules without
   changing generated output. Verify existing snapshots byte-for-byte.
4. Extract shared names/imports/files/manifests from real duplicate implementations;
   retain protocol-specific lowering and runtime semantics.
5. Extend generated-symbol contracts so companions follow resolved artifacts.
6. Apply the generated SDK source-quality specification and targeted/native tests.
7. Run the full regression suite and record which language/protocol/style combinations
   conform, which have justified exceptions and which remain unsupported.

This cleanup is separate from adding a new protocol capability. A nicer crate tree
must not be reported as GraphQL/RPC/event feature completion.
