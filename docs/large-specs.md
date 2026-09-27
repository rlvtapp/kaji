# Large specifications and Go SDK layout

Kaji always splits Go output: one model per file, one operation (with
request/errors/pagers) per file, and service files capped at 50 methods.
The files share one Go package, so splitting does not introduce import cycles
or change the public client API. There is no layout toggle.

`client.go` owns shared HTTP and retry infrastructure. Model, operation, and
service filenames are readable, bounded, and hash-suffixed to avoid filesystem
length limits and normalized-name collisions. Each file imports only the
standard-library packages it uses. Large single type declarations cannot be
divided between Go source files.

```rust
use kaji::{go, prelude::*};

let release = ProfileSet::new("sdk")
    .package(go::package("go")
        .name("graph")
        .with(go::sdk().namespaced().jobs(4)));
// kaji::generate(&api, release)?.write_to(output)?;
```

Equivalent CLI selection:

```sh
npx @relevate/kaji generate graph.yaml --output ./generated --language go \
  --name "Microsoft Graph" --jobs 4
```

Use a fresh output directory when changing output paths or removing operations:
the existing writer intentionally preserves unrelated/custom files and does not
prune obsolete generated files. Leftover declarations can conflict with the new
output. This is not yet a manifest-based clean rebuild.

## Parallelism and memory

Split-model and operation rendering uses scoped, bounded worker threads. The
default is up to eight available workers; `.jobs(1)` is serial, `.jobs(0)` is
automatic, and explicit counts are capped at 64. CLI `--jobs` accepts positive
integers. Small batches stay serial to avoid thread startup overhead.

Workers own their file buffers; no mutable plugin workspace is shared between
threads. Trees merge in deterministic order with normal collision checks. The
same input must produce byte-identical output regardless of worker count.
Package finalization and service rendering remain ordered. This is currently
Go rendering parallelism, not parallel execution of arbitrary community plugins
or all language packages.

Composition moves generated file buffers instead of cloning them between output
prefixes. The engine borrows the API unless a package overrides its version.
The AST and final output tree are still held in memory; generation is not a
constant-memory streaming pipeline. Splitting files also does not make Go's
compiler compile each source file independently: it still compiles the package.

## Microsoft Graph regression test

Run the full opt-in check with Rust, Go, curl, and Bash installed:

```sh
bash scripts/test-large-graph.sh
```

It downloads Microsoft's Graph v1.0 document, compiles it using Kaji's bundled Go
compiler source, generates the complete Go SDK with one and four workers,
compares every output file, and runs a generated-SDK test. The test uses an
in-memory HTTP transport (no Graph account, credentials, or live API calls) and
checks path escaping, bearer auth, OData query parameters, resource access, and
typed inherited response fields. The temporary workspace is retained and printed
for inspection. Allow several hundred MB of disk plus compiler cache space.

The September 27, 2026 input used during development had SHA-256
`77c1a39c94ab0a72c0a2e07ff74f221903141e8913e748fddcc189b98da42feb`:

- 17,870 operations and 5,187 component schemas.
- 23,465 generated files, including 404 bounded service files.
- Largest Go file about 106 KiB (a single large enum).
- Optimized local SDK generation plus file writing about 4–6 seconds from
  previously compiled artifacts; this excludes OpenAPI parsing and Go compilation
  and is not a cross-machine performance guarantee.

The test intentionally downloads the current document, so counts may change.
The compiler previously hit a 1 GB goroutine stack overflow while synthesizing
recursive examples; path-local guards now stop that traversal. Tests also cover
recursive field summaries, branching budgets, long paths, and filename collisions.

Compatible named object `allOf` inheritance becomes ordinary typed Go structs.
Ambiguous/non-object intersections and union types can still use
`json.RawMessage`; this does not claim full discriminator-aware union support.
Wire-distinct enum values and path/query parameters retain distinct identifiers
even if their normalized Go names collide.
