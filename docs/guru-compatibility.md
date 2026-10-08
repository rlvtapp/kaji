# Public-contract compatibility baseline

This October 8, 2026 source-build baseline attempts all 205 pinned contracts in
all ten SDK languages: 2,050 generation/native-check cases. The tested generator
revision is `d4e884f`. The main CI passed for that revision, while these broader
checks found additional failures. An unqualified cross-language stability claim
is premature.

Inputs, checksums and selection are documented in [large-spec testing](large-specs.md).
The [machine-readable results](verification-results/guru-2026-10-08.json) retain
every passing contract, every failing contract, failure phase, exit code and a
diagnostic. Contracts are not removed because they fail.

## Results

| Language | Passed | Generation failed | Native failed | Resource failure | Timeout |
| --- | ---: | ---: | ---: | ---: | ---: |
| Go | 205 | 0 | 0 | 0 | 0 |
| Python | 165 | 0 | 40 | 0 | 0 |
| TypeScript | 164 | 11 | 30 | 0 | 0 |
| Java | 132 | 23 | 50 | 0 | 0 |
| C# | 117 | 14 | 74 | 0 | 0 |
| Rust | 158 | 0 | 47 | 0 | 0 |
| Swift | 72 | 62 | 71 | 0 | 0 |
| Ruby | 188 | 0 | 17 | 0 | 0 |
| PHP | 165 | 9 | 31 | 0 | 0 |
| Elixir | 96 | 0 | 109 | 0 | 0 |

Each row totals 205. A pass requires generation and a successful native check.
Generation failures include explicit unsupported-subset rejections and output
collisions; the diagnostic identifies which occurred. Native failures include
compiler errors, import/load errors and Elixir warnings denied by the check.
A timeout or resource failure establishes an incomplete check, rather than a
generated-code defect.

## Stripe and Azure

P = passed; G = generation failed; N = native failed; T = timed out; R = resource failure.

| Language | Stripe | Azure Web Apps | Compute | Virtual WAN | Storage | Key Vault |
| --- | --- | --- | --- | --- | --- | --- |
| Go | P | P | P | P | P | P |
| Python | P | P | P | P | P | N |
| TypeScript | N | P | P | P | P | P |
| Java | G | P | P | P | P | P |
| C# | G | P | P | P | P | P |
| Rust | P | P | P | P | P | N |
| Swift | G | P | P | P | P | N |
| Ruby | P | P | P | P | P | P |
| PHP | P | P | P | P | P | P |
| Elixir | N | N | N | N | N | N |

These Azure fixtures cover five services, not the entire Azure API surface.
Stripe initially exhausted Node's default 4 GB heap. An 8 GB retry completed
and exposed generated type/import errors, including a form encoding type that
excludes `deepObject` and a missing generated model import. The retry is counted
in the results above. Its generated TypeScript package is about 147 MB, which
also motivates reducing repeated inline type expansion.

## What needs fixing first

1. Allocate identifiers consistently across fields, parameters, models and
   resources. Preserve wire names while distinguishing `+1`/`-1`, reserving
   runtime/local names and keywords, and avoiding case-only path collisions.
   Examples include GitHub reaction fields, Python `field` shadowing, C# `query`
   locals, Swift `default` enum cases and case-only resource names.
2. Make references, aliases and composed models consistent across emitters.
   Missing model imports/decoders and repeated inherited fields prevent native
   compilation even when generation succeeds.
3. Align TypeScript form serialization types with emitted encoding metadata and
   reduce large repeated type expansions. Recheck Stripe with an explicit heap
   budget after the fixes.
4. Expand the declared multipart subset where intended and give endpoint/schema
   context for remaining rejections. Java/C#/Swift reject some existing inputs;
   those limitations must stay explicit.
5. Minimize these failures into regression fixtures and promote a representative
   set into normal CI. Keep the full pinned corpus as a larger compatibility gate.

## Check boundaries and reproduction

Rust, Go, TypeScript, Java, C# and Swift run native compilation. Python runs
bytecode compilation plus package import; Ruby runs syntax checks plus package
load; PHP lints every source file; Elixir compiles with warnings denied. These
checks do not call production APIs, prove every operation's runtime behavior,
or test registry publication. See [runtime verification](verification.md).

Toolchain versions and per-phase time limits are recorded in the JSON ledger.
Ruby/PHP/Elixir use containers with two CPUs and 2 GB memory. Elixir reuses
verified dependencies and rebuilds the generated application. Rust/Swift use
two build workers. Standard-library and dependency caches are reused. The
exploratory Python 3.9 run is excluded; the recorded Python run uses 3.12.

With the matching native toolchain installed, reproduce a target with:

```sh
(cd openapi && go build -o ../target/debug/kaji-openapi .)
cargo build --locked -p kaji-cli
KAJI_BINARY="$PWD/target/debug/kaji" python3 scripts/guru-corpus.py \
  --language typescript --output /tmp/kaji-guru-typescript-new
```

Use a fresh output directory. `--contracts stripe,azure-compute` selects focused
cases; `--keep-generated` preserves sources for diagnosis. Set
`NODE_OPTIONS=--max-old-space-size=8192` for the recorded Stripe retry. The
manual `APIs.guru large contract corpus` workflow provisions supported toolchains
and uploads reports and diagnostics for one language or all ten.
