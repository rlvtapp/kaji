# Generator completion backlog

Audited on 2026-10-08. Checked items are delivered; unchecked items retain
remaining work. This backlog covers SDKs, framework helpers and generated artifacts. The
[compatibility results](guru-compatibility.md) remain the source of truth for
native compilation; the [feature catalog](features.md) describes current support.

## 1. Get every SDK language green

- [x] **GREEN-1:** Add small regressions for the corpus failure causes: normalized
  symbol collisions, reserved words, case-insensitive paths, local-variable
  shadowing and bounded filenames, while preserving wire keys.
- [x] **GREEN-2:** Fix referenced aliases, recursive models, inherited duplicate
  fields and missing model imports/decoders. Exercise actual decoded values and
  round trips, beyond syntax checks.
- [x] **GREEN-3:** Correct TypeScript form/media encoding types and excessive
  nested type expansion. Resolve supported multipart failures; retain explicit
  diagnostics for genuinely unsupported forms.
- [x] **GREEN-4:** Rerun all 205 pinned contracts across ten languages. Resolved the
  baseline of 1,462 passes, 119 generation failures and 469 native failures.
  The reconciled follow-up covers 2,050 passing cases; reports identify frozen builds
  and retries, including Swift’s iterative prefix and frozen confirmations.
  Track compiler failures separately from unsupported-feature rejections. Require
  supported cases to pass and document any remaining exclusions individually.

## 2. Make output scale predictably

File count alone is insufficient: a single operation/model can be enormous,
while many small files can still repeat the same types. Stripe's original
TypeScript package measured 147,315,512 bytes. The follow-up
measures 3,580,589 bytes and compiles in 5.888 seconds with the default Node heap.
These are package sizes, not single-file sizes. Splitting and reducing duplicated
declarations are separate tasks.

The following inventory includes the delivered fixes. A shared layout API and
byte policy across every emitter remain open. Atomic declarations may exceed budgets.

| Output | Current layout | Work to schedule |
| --- | --- | --- |
| TypeScript SDK | Per-model and per-operation modules, aggregate facades/exports | Shared schema descriptors delivered; bound remaining facades and barrels |
| Go | Per-model/per-operation files; resource groups of 50; bounded response registries | Add byte-aware grouping and oversized declaration diagnostics |
| Python | Per-model modules; operation/resource groups of 100; grouped exports | Bound aggregate client/import registries and large individual models |
| Rust | Per-model modules grouped in directories of 100; operation/resource groups of 25 | Byte-aware groups and bounded root exports |
| Java | Per-model classes; grouped operations and resource groups of 100 | Typed holders handle JVM argument limits; broader byte budgets remain |
| C# | Per-model files; operation/resource partial groups of 100 | Byte-aware partials and bounded facades |
| PHP | Per-model classes; operation groups of 100, resource groups of 25 | Byte-aware traits and aggregate class limits |
| Elixir | Per-model modules; grouped operations, errors and resources | Bound delegates/registries and verify split decoder references |
| Swift | Per-model files; bounded operation/resource extensions; large-query helpers | Broader byte policy and large individual declarations |
| Ruby | Bounded model/operation/resource/factory/descriptor modules; stable loaders | Large atomic models and common layout settings |
| React/Vue Query, SWR | Bounded operation chunks with stable public aggregate | Additional layout modes and operation selection |
| Zod, Faker, MSW, Cypress | Bounded modules with max_file_bytes and stable entrypoints | Broader behavioral audits and large atomic declarations |
| Direct TypeScript models API | Aggregate models.ts | Offer split layout without confusing it with the SDK model provider |
| TypeScript CLI | Bounded command modules; separate runtime | Command/flag collision and broader behavior audit |
| Rust CLI | Bounded command metadata; runtime and extension modules | Broader runtime/layout controls |
| Terraform | Per-resource/data-source Go files; shared provider/runtime | Bound large schemas and provider registries using Go declarations |
| Postman | One collection JSON plus environment JSON | Optional valid collections by tag/resource; retain a default single collection |
| MCP tool artifact | One tools.json manifest | Measure manifest size; filtering/grouping without implying a generated server |
| ReDoc | HTML/config entrypoint | Budget bundled contract assets separately from small entrypoints |
| Mock artifacts | Server configuration/fixtures | Budget fixture payloads and group routes without breaking server format |
| Symfony / dotnet | PHP integration / C# compatibility alias | Reuse the underlying language policy; test integration files separately |

- [ ] **SIZE-1:** Extend corpus reports with largest source file, source bytes,
  metadata bytes, file counts, top oversized paths, generation time and native
  compile time. Measure Stripe, GitHub, Graph, Mailchimp and Azure, including
  auxiliary generators and CLIs. Record peak memory where the runner supports it.
- [ ] **SIZE-2:** Design typed layout settings for single-file, per-operation,
  per-resource and bounded chunks. Add configurable warning budgets for bytes
  and declaration counts. Select defaults from measurements; do not present an
  arbitrary byte threshold as a compiler limit.
- [ ] **SIZE-3:** Ruby/Swift/CLI/auxiliary splitting and Go registries are delivered.
  Still apply byte-aware grouping to existing chunked emitters. Split at declaration
  boundaries; factor large schemas rather than slicing source text.
- [ ] **SIZE-4:** Preserve stable public imports, typed provider handles, custom
  transport bindings, bundled middleware, authored overlays and ownership cleanup.
  Bound filenames and check case collisions. Generate identical output across
  worker counts and repeat runs; remove stale chunks safely when groups shrink.
- [ ] **SIZE-5:** Add large-output regression checks for budgets, compilation,
  import graphs and representative memory/time growth. Keep a small fixture in
  normal CI and the pinned large corpus in the gated workflow.

The original generation-only Stripe probe measured Ruby models.rb at 1,994,092 bytes,
Go client.go at 508,751 bytes and the largest TypeScript operation module at
375,848 bytes. See the [dated layout measurements](verification-results/layout-2026-10-08.json)
for revision, source checksum, package totals and largest paths. This is a
three-target historical sample. [Follow-up measurements](verification-results/layout-fixed-2026-10-08.json)
record compile times and bounded output. Corpus source/metadata totals, largest paths
and warning budgets are delivered; peak memory and full auxiliary audit remain SIZE-1.

## 3. Complete framework consumers

React/Vue output now includes native typed hooks, reusable option/key factories,
cache scopes, cancellation and bounded modules. Infinite/suspense variants and
operation selection remain open; these are independent from file layout.

- [x] **QUERY-1:** Generate reusable queryOptions/mutationOptions and stable key
  factories for React/Vue, with native typed options, callbacks and overrides.
- [x] **QUERY-2:** Forward query-function cancellation signals into the selected
  HTTP driver. Build keys from operation inputs and explicit cache scope rather
  than whole client/config objects; test tenant separation and credential-header
  exclusion. Operation inputs remain visible in keys, as documented.
- [ ] **QUERY-3:** Add infinite-query helpers using resolved pagination metadata,
  suspense variants where supported, prefetch/SSR examples and framework-specific
  signatures. Define page parameters, termination and cancellation explicitly.
- [ ] **QUERY-4:** Support operation selection, query/mutation classification
  overrides, names and grouped output. Preserve custom operation providers and
  relocated imports; compile against supported React/Vue versions.
- [ ] **QUERY-5:** Delivered cache reuse/invalidation, callback contexts, errors
  and framework/caller abort runtime tests. Page traversal remains open and
  depends on infinite-query helpers. Compile tests alone do not establish behavior.
- [ ] **SWR-1:** Delivered explicit cache scopes, native configuration overrides
  and cancellation forwarding. Still add
  pagination/mutation helpers where appropriate to SWR's API; document differences.

## 4. Harden all other generated systems

These are audit/extension tasks. Each starts with existing implementation tests
and capability documentation so already-supported behavior is not reimplemented.

- [ ] **VALIDATION-1:** Audit Zod constraints, aliases, recursion, unions,
  nullability and request/response schemas against the model provider's wire
  representation, including lossless integers. Test valid and invalid values.
- [ ] **FIXTURES-1:** Audit Faker seeds, constraints, recursion bounds and optional
  values. Make unsupported sample construction visible instead of producing
  apparently valid fixtures.
- [ ] **MOCKS-1:** Expand MSW scenarios for declared status codes, media types,
  pagination and failures. Compile and execute handlers against generated clients.
- [ ] **CYPRESS-1:** Turn useful scaffolding into configurable executable smoke
  suites with explicit assertions, deterministic fixtures and safe local defaults.
- [ ] **POSTMAN-1:** Test collection splitting, stable identifiers, regeneration,
  environment/custom script preservation and secret-free exports. Broaden Newman
  coverage for auth, media encoding, examples, pagination and response assertions.
- [ ] **TERRAFORM-1:** Reconcile every planned lifecycle capability with the
  [provider guide](terraform-provider.md) and current tests: import identifiers,
  nested state, optional/computed/sensitive/write-only attributes, replacement,
  partial updates, polling, timeouts and drift. Record supported cases explicitly.
- [ ] **TERRAFORM-2:** Add acceptance scenarios against disposable local services
  for create/read/update/delete, refresh, import, plan stability and failures.
  Validate documentation and provider packaging; keep publication gated.
- [ ] **CLI-1:** Test normalized command/flag collisions, large command trees,
  encoding, auth profiles, noninteractive behavior and customer extension
  preservation in both CLI targets.
- [ ] **MCP-1:** Verify tool schema/serialization, bounded discovery and error
  behavior for the existing runtime server; keep generated manifest documentation
  distinct from server behavior. Add selection and size reporting.
- [ ] **DOCS-1:** Audit ReDoc/offline assets, mock-server examples and generated
  API references. Give every generator an author guide, supported-subset table,
  executable example and customization/splitting instructions.
- [ ] **DELIVERY-1:** Validate generated per-language checks, source bundles,
  synchronization and trusted-publishing workflow files in disposable/gated CI.
  Live GitHub App delivery and registry publication remain separate authorized
  exercises; this backlog does not enable publishing.
- [ ] **MIGRATION-1:** Exercise vendor migration recipes on larger real projects;
  retain explicit manual-review diagnostics for unsupported annotations/settings.

## Completion rules and ordering

Work in this order: GREEN regressions; SIZE reporting and TypeScript expansion;
Ruby/Swift/helper splitting; QUERY APIs and runtime tests; remaining artifact
acceptance coverage. Splitting work may run alongside native compiler fixes.

A task is complete only when its public options and limitations are documented,
its output works with independent/custom plugin providers, and meaningful native
or behavioral tests pass. A successful generation, small file count or matching
feature name is insufficient. Update this checklist with evidence links as tasks
land; do not advertise planned work as released capability.
