# Public-contract compatibility results

**Alpha.2 update (9 October 2026):** the new frozen-build 205×10 sweep is complete:
2,050 effective passes including regeneration, after audited infrastructure retries.
The frozen binaries predate the latest output migration. The record below is the
historical October 8 reconciled result, not confirmation of the current working
tree. See [current verification](verification.md#current-alpha2-verification-9-october-2026).

The October 8, 2026 follow-up records successful generation and native checks for
all 205 pinned contracts in all ten SDK languages: **2,050 passing cases**.
No contracts were excluded. These results reconcile full runs with targeted
confirmations across the fixes; they are not a single frozen-build matrix run.

The [follow-up ledger](verification-results/guru-fixed-2026-10-08.json) records
contract checksums, executable hashes where available, phase exit codes/times,
output measurements and the report used for each confirmation. The
[original baseline](verification-results/guru-2026-10-08.json), at `d4e884f`,
retains 1,462 passes, 119 generation failures and 469 native failures.
Inputs and selection are documented in [large-spec testing](large-specs.md).

## Results

| Language | Generation + native passed | Unresolved cases |
| --- | ---: | ---: |
| Go | 205 | 0 |
| Python | 205 | 0 |
| TypeScript | 205 | 0 |
| Java | 205 | 0 |
| C# | 205 | 0 |
| Rust | 205 | 0 |
| Swift | 205 | 0 |
| Ruby | 205 | 0 |
| PHP | 205 | 0 |
| Elixir | 205 | 0 |

Go, Python, Java, C#, Ruby and PHP have full frozen-executable sweeps. Elixir
uses a frozen prefix and tail covering the same 205 inputs. TypeScript has a
204-pass frozen sweep plus a successful Snyk retry and ten latest representative
confirmations. Rust's frozen sweep includes a fresh Zuora retry after a runner
interruption. Swift combines an iterative 150-case prefix with a frozen 55-case
tail, frozen confirmations of every failure and latest representative checks.
Successful Swift prefix cases do not have per-case executable hashes; a new
single-build full Swift sweep would provide stronger reproducibility.

Runner interruptions are incomplete checks, not generated-code failures. Their
fresh successful retries are identified in the ledger. The final idempotency
argument fixes also have focused native regressions; most corpus contracts do
not exercise auto-idempotency annotations.

## What changed

Identifier allocation now preserves original wire keys through keywords,
normalized-name collisions, case-insensitive paths, runtime names and references.
Fixes cover aliases, recursive models, inherited duplicate fields, native codecs,
nullable values and enum backing values. Java uses typed holders for operations
and models that would exceed JVM argument limits. Valid multipart schemas now
compile through ordered builders in Java, C# and Swift.

Shared TypeScript response descriptors avoid repeated schema expansion. Stripe's
package fell from 147,315,512 to 3,580,589 bytes, about 97.6% smaller. Its latest
native check took 5.888 seconds with the default Node heap; the earlier 8 GB retry
took about 135.9 seconds after a default-heap exhaustion. These timings include
cache/toolchain effects and are measurements, not performance guarantees.

Ruby, Swift, Go response registries, CLI command trees and auxiliary TypeScript
outputs now split at declaration boundaries. Chunk byte targets are soft:
a single atomic model or operation can exceed them. Swift's large-query helper
fix reduced the Gsmtasks Debug build from about 405 to 16.66 seconds while keeping
public operation signatures. See the [follow-up layout measurements](verification-results/layout-fixed-2026-10-08.json)
and [remaining generator backlog](generator-backlog.md).

Stripe and all five Azure fixtures pass in every language. The Azure fixtures
cover Web Apps, Compute, Virtual WAN, Storage and Key Vault; they do not represent
the entire Azure API surface.

## Check boundaries and reproduction

Rust, Go, TypeScript, Java, C# and Swift run native compilation. Python runs
bytecode compilation plus package import; Ruby runs syntax checks plus package
load; PHP lints every source file; Elixir compiles with warnings denied. These
checks do not call production APIs, prove every operation's runtime behavior,
or test registry publication. [Runtime verification](verification.md) records
separate behavior tests for wire values, retry stability, middleware, decoding,
framework caching/cancellation and split imports.

The follow-up uses Go 1.25, Python 3.12, TypeScript 5.9, Java 17, .NET 8, the
installed Rust toolchain and Swift 6.4. Ruby/PHP/Elixir use the native corpus
containers; their baseline toolchain metadata is retained in the original ledger.
Dependency caches are reused; Rust/Swift use two workers. Peak memory was not
measured. Full publication and live GitHub synchronization remain separate gated
exercises.

With the matching native toolchain installed, reproduce a target with:

```sh
(cd openapi && go build -o ../target/debug/poolster-openapi .)
cargo build --locked -p poolster-cli
POOLSTER_BINARY="$PWD/target/debug/poolster" python3 scripts/guru-corpus.py \
  --language typescript --output /tmp/poolster-guru-typescript-new
```

Use a fresh output directory. `--contracts stripe,azure-compute` selects focused
cases; `--keep-generated` preserves sources for diagnosis. Reports separate source
and metadata bytes, list the largest source files and emit configurable size
warnings. The manual `APIs.guru large contract corpus` workflow provisions
supported toolchains and uploads reports and diagnostics for one language or all ten.
