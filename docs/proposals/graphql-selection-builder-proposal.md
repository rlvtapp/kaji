# Typed GraphQL selections: proposal

Status: **planned; not implemented or supported by the current generated SDKs**.

The current GraphQL pipeline generates clients from validated operation documents.
Each operation has a fixed selection and a selection-specific result type. This
proposal adds an optional TypeScript mode where callers choose fields using a typed
second parameter. Finishing the existing operation-based clients takes priority.

## Proposed caller API

```ts
const response = await client.readUser(
  { id: "42" },
  {
    fields: {
      id: true,
      name: true,
      posts: {
        args: { limit: 5 },
        fields: { title: true },
      },
    },
  },
);
```

The first parameter supplies root-field arguments. The second supplies selections
and nested arguments. Autocomplete offers schema-defined fields and arguments;
invalid names, argument types and selection shapes produce TypeScript errors.
The inferred data type contains only the selected fields, preserving list and
schema nullability. GraphQL success, partial data and errors retain the existing
explicit response representation.

The same signature should work with explicit resource grouping:

```ts
await client.user.read(
  { id: "42" },
  { fields: { id: true, name: true } },
);

await client.user.rename(
  { id: "42", name: "Ada" },
  { fields: { id: true, name: true } },
);
```

Queries and mutations use the same selection mechanism. Grouping changes method
placement, not operation semantics. Names such as `readUser` and `user.read` are
illustrative: the mapping from schema root fields to public methods must be
explicitly designed and checked for collisions before implementation.

## Configuration and contract integration

Selection-builder options belong under the GraphQL exporter configuration in
`contracts.graphql`. The exact option names remain undecided. Fixed-operation
generation remains available and backward compatible; dynamic selections are a
separate, opt-in mode, not a reinterpretation of existing operation variables.

Use Poolster-owned schema/selection contracts and exposed blocks, extending them
where needed. Output interfaces must not expose parser-library types. Reuse the
typed graph, deterministic naming, ownership, customization and package assembly
machinery. Preserve native documents for details not represented by contracts.

Grouping initially comes from explicit exporter configuration. A GraphQL directive
for grouping metadata is a separate deferred proposal; no directive support is
required for this feature.

## Required implementation work

- Generate schema-driven selection and argument types, including recursive
  objects, lists, enums, input objects and custom scalar mappings.
- Infer selected results without losing nullability or omitted-field semantics.
- Build and validate operation documents at runtime; bind argument values as
  variables rather than interpolating them into GraphQL source.
- Define aliases and repeated fields with different arguments, plus interface and
  union selections through fragments and `__typename` discrimination.
- Define conditional selections, defaults, empty-selection rejection, operation
  naming and transport cancellation behavior.
- Preserve existing HTTP transport and explicit partial/error handling. Subscription
  support remains a separately declared transport capability.
- Reject unsupported features clearly; do not silently omit requested selections.

Schema types describe what callers may select; existing operation result types
alone cannot provide this API. Runtime selection validation is also necessary for
JavaScript callers and values that bypass TypeScript checking.

## Acceptance checks

- Compile consumer examples proving exact inferred results and rejection of
  nonexistent fields, invalid arguments and invalid nested selections.
- Execute queries and mutations against a pinned local GraphQL server, checking
  that only requested fields are sent and returned.
- Cover aliases, abstract types, scalar mappings, nested/list/null results,
  partial errors, cancellation and malformed runtime selection objects.
- Compile and execute clean installed generated packages; verify provider
  substitution and deterministic regeneration.
- Keep existing fixed-operation and OpenAPI checks passing, and update the support
  matrix only after each capability is tested.

TypeScript is the initial target. Rust requires a separate API design, likely
using typed builders or generated selection structures; no equivalent Rust API
is promised by this proposal.
