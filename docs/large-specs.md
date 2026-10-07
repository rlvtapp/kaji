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

Regenerate through the ownership-aware writer when changing paths or removing
operations. It preflights conflicts, removes unchanged stale owned files and
preserves unrelated/customer files. An edited owned file blocks regeneration
until resolved; `--check` detects drift without writing. See
[safe regeneration](safe-regeneration.md).

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

It downloads a pinned Microsoft Graph v1.0 document, compiles it using Kaji's bundled Go
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

The test now pins upstream revision `fd42f0e5bcd96b0c5edd5e62e2956a1dc1c5d17a`
and SHA-256 `533f6d86985584327109ba2c52c0e51454ce1ba2153c11f4d841e898b88c5b1b`.
A changed checksum fails before compilation. `KAJI_GRAPH_SPEC` can supply those
exact bytes from a local cache; `KAJI_BINARY` can use an already built generator.
The historical counts above describe the earlier recorded input. A new pin needs
intentional review and a new native run.
The compiler previously hit a 1 GB goroutine stack overflow while synthesizing
recursive examples; path-local guards now stop that traversal. Tests also cover
recursive field summaries, branching budgets, long paths, and filename collisions.

Compatible named object `allOf` inheritance becomes ordinary typed Go structs.
Ambiguous/non-object intersections and union types can still use
`json.RawMessage`; this does not claim full discriminator-aware union support.
Wire-distinct enum values and path/query parameters retain distinct identifiers
even if their normalized Go names collide.

The read-only `Pinned large contract regression` workflow runs this same pinned
check on explicit dispatch. It does not call Microsoft Graph or publish packages.
This verifies full Go compilation/behavior and multi-language generation; it
does not yet compile every target against multiple public specifications.

## Full official contracts

`scripts/fixtures/large-contracts.json` pins revisions and SHA-256 digests for
Stripe, GitHub, OpenAI, Twilio, DigitalOcean and Linode. DigitalOcean is an archive
because its root document depends on local files. The fetcher verifies the archive
before extraction and rejects traversal, symlinks and oversized inputs.

```sh
KAJI_PUBLIC_CONTRACT_MANIFEST="$PWD/scripts/fixtures/large-contracts.json" \
KAJI_PUBLIC_CONTRACTS=openai \
KAJI_PUBLIC_LANGUAGES=go \
KAJI_PUBLIC_CONTRACT_ROOT=/tmp/kaji-public-check \
bash scripts/test-public-contracts.sh generate

KAJI_PUBLIC_CONTRACT_MANIFEST="$PWD/scripts/fixtures/large-contracts.json" \
KAJI_PUBLIC_CONTRACTS=openai \
KAJI_PUBLIC_CONTRACT_ROOT=/tmp/kaji-public-check \
bash scripts/test-public-contracts.sh check go
```

The manual `public-contracts.yml` workflow selects a contract and generates and
compiles it separately in all ten language lanes. It only reads upstream sources
and uses local compilation; it does not call production APIs or publish packages.
Each lane uploads compiler diagnostics even when generation fails. A workflow
matrix is coverage infrastructure, not evidence that every contract compiles in
every language. Local native Go compilation passed for full OpenAI, GitHub, Stripe,
Twilio, Linode and DigitalOcean. Full OpenAI native compilation also passed for
TypeScript, Python, Ruby, Rust, Java and C#. This does not prove every wire behavior
in those APIs. Other lanes are being hardened against these same contracts;
unsupported constructs fail explicitly rather than silently dropping operations.
