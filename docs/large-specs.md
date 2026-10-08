# Large specifications and Go SDK layout

[Compiler](openapi-compiler.md) · [Compatibility results](guru-compatibility.md) · [Verification](verification.md)

Poolster always splits Go output: one model per file, one operation (with
request/errors/pagers) per file, and service files capped at 50 methods.
The files share one Go package, so splitting does not introduce import cycles
or change the public client API. There is no layout toggle.

`client.go` owns shared HTTP and retry infrastructure. Model, operation, and
service filenames are readable, bounded, and hash-suffixed to avoid filesystem
length limits and normalized-name collisions. Each file imports only the
standard-library packages it uses. Large single type declarations cannot be
divided between Go source files.

```rust
use poolster::{go, prelude::*};

let release = ProfileSet::new("sdk")
    .package(go::package("go")
        .name("graph")
        .with(go::sdk().namespaced().jobs(4)));
// poolster::generate(&api, release)?.write_to(output)?;
```

Equivalent CLI selection:

```sh
npx poolster generate graph.yaml --output ./generated --language go \
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

It downloads a pinned Microsoft Graph v1.0 document, compiles it using Poolster's bundled Go
compiler source, generates the complete Go SDK with one and four workers,
compares every output file, and runs a generated-SDK test.

The test uses an
in-memory HTTP transport (no Graph account, credentials, or live API calls) and
checks path escaping, bearer auth, OData query parameters, resource access, and
typed inherited response fields.
The temporary workspace is retained and printed
for inspection.
Allow several hundred MB of disk plus compiler cache space.

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
A changed checksum fails before compilation.
`POOLSTER_GRAPH_SPEC` can supply those
exact bytes from a local cache; `POOLSTER_BINARY` can use an already built generator.
The historical counts above describe the earlier recorded input.
A new pin needs
intentional review and a new native run.

The compiler previously hit a 1 GB goroutine stack overflow while synthesizing
recursive examples; path-local guards now stop that traversal.
Tests also cover
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
KAJI_PUBLIC_CONTRACT_ROOT=/tmp/poolster-public-check \
bash scripts/test-public-contracts.sh generate

KAJI_PUBLIC_CONTRACT_MANIFEST="$PWD/scripts/fixtures/large-contracts.json" \
KAJI_PUBLIC_CONTRACTS=openai \
KAJI_PUBLIC_CONTRACT_ROOT=/tmp/poolster-public-check \
bash scripts/test-public-contracts.sh check go
```

The manual `public-contracts.yml` workflow selects a contract and generates and
compiles it separately in all ten language lanes.
It only reads upstream sources
and uses local compilation; it does not call production APIs or publish packages.
Each lane uploads compiler diagnostics even when generation fails.

A workflow
matrix is coverage infrastructure, not evidence that every contract compiles in
every language.
Local native Go compilation passed for full OpenAI, GitHub, Stripe,
Twilio, Linode and DigitalOcean.
Full OpenAI native compilation also passed for
TypeScript, Python, Ruby, Rust, Java and C#.
This does not prove every wire behavior
in those APIs.

Other lanes are being hardened against these same contracts;
unsupported constructs fail explicitly rather than silently dropping operations.

## APIs.guru corpus

`scripts/fixtures/guru-contracts.json` pins 205 specifications from 201
APIs.guru providers to one immutable repository revision, with byte sizes and
SHA-256 checksums.
The root documents total about 181 MB.
Every contract
exceeds 100 KB; the
original 32 exceed 1 MB.

Selection keeps the original fixtures and adds the
largest qualifying OpenAPI document from each additional provider, plus five
Azure Swagger 2 contracts: Web Apps, Compute, Virtual WAN, Storage and Key Vault.
It includes GitHub, Stripe, AWS EC2, Google Compute, Mailchimp, Zoom, DocuSign,
Jira, Plaid, Box and other large APIs.

```sh
python3 scripts/guru-corpus.py --language go --output /tmp/poolster-guru-check
# A smaller selection, or retain generated sources for debugging:
python3 scripts/guru-corpus.py --language python --contracts github,stripe \
  --output /tmp/poolster-guru-python --keep-generated
```

Build `target/debug/poolster` and `target/debug/poolster-openapi` first, or set
`POOLSTER_BINARY` and `POOLSTER_OPENAPI_BIN`. The output must be a fresh directory.
The runner verifies inputs, generates and checks each SDK separately, continues
after failures and returns a failing exit code if any case fails. Reports include
source digests, phase exit codes, timeouts, logs and generation metadata. They
also record source/metadata bytes, largest source paths and total file counts
before native compilers create build artifacts. `--file-warning-bytes 262144`
reports outputs above that optional budget without failing generation or treating
the threshold as a compiler limit.
Generated sources are temporary unless `--keep-generated` is supplied.

The manual `guru-contracts.yml` workflow selects one language or all ten and
uploads these diagnostics. It does not publish or call the upstream APIs. Native
checks use the existing public-contract runner: compilation for compiled targets,
Python import/bytecode checks, Ruby syntax/load checks and PHP syntax checks.
These checks are not complete runtime conformance tests.

The October 8, 2026 local Go verification passed all 205 contracts: the complete
200-provider run plus a separate five-service Azure run. Each pass includes SDK
generation and `go test ./...`. All original seven failures and the four new
failures discovered while expanding the corpus are resolved. This verifies Go
compilation, not every API operation's runtime semantics or all language targets.

The expanded corpus exposed model/runtime/service/enum symbol collisions,
repeated operation IDs, nested schema references, YAML block scalar compatibility,
and JSON keys that cannot appear in Go struct tags.
Poolster now allocates stable Go
symbols and operation identities, lifts nested reference targets, preserves
valid block content, and generates custom JSON encoding for those wire keys.

References inside vendor extension data remain literal and do not trigger file
fetches.
Focused tests cover these behaviors through independent typed plugins.

The catalog's DigitalOcean root omits discriminator mapping files. Its original
bytes remain unchanged; the manifest separately pins 14 companion files from
the official repository, including their complete local reference closure,
checksums and size limits. The fetcher verifies every file before assembling an
isolated source tree. No missing schemas are guessed or skipped.

The subsequent [cross-language baseline](guru-compatibility.md) attempts all
2,050 cases and retains generation and native failures in the other targets.
This is a compatibility assessment, not an all-language pass claim.

Selecting `all` in the workflow requests 2,050 generation/native-check cases
across ten language lanes. That is available coverage, not evidence that every
contract or runtime behavior passes in every language.
