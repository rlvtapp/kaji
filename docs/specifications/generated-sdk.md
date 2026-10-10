# Generated SDK specification

**Version 1 · 10 October 2026 · normative target, with implementation gaps.**

This defines what Poolster's generated SDKs should provide. It covers the ten SDK
languages, the Symfony integration and the deprecated .NET alias. It applies to
HTTP, GraphQL and future native protocol generators without converting those
protocols into one HTTP model.

**MUST** is a release requirement for a capability advertised as conforming.
**SHOULD** is the default; an exception needs a documented reason and verification.
This document is not a claim that every current generator conforms. The
[support matrix](../plugin-support-matrix.md) records implemented capabilities;
[verification records](../verification/verification.md) record actual evidence.

## 1. Required package capabilities

| Requirement | Required behavior |
| --- | --- |
| Installable package | Native manifest, declared dependencies, public entry point and documented minimum runtime/toolchain. A clean consumer must install and compile it. |
| Models | Preserve wire names, references, enums, nullability, omission, defaults, recursion and supported abstract alternatives. Never silently replace an unsupported shape with an empty object. |
| Operations | Typed variables/parameters and results where the language permits; documented value shapes and signatures otherwise. Preserve the contract's semantics. |
| Transport | Inject the native HTTP/RPC/broker driver where supported; configure endpoint/authentication and expose cancellation or cleanup appropriate to that driver. |
| Results | Distinguish transport/protocol/decoding errors from application errors. Preserve partial results and response metadata when the protocol supplies them. |
| API styles | Raw, flat and idiomatic surfaces over the same operation implementation. Unsupported style/capability combinations need explicit diagnostics. |
| Source quality | Semantic file splitting, deterministic symbols/paths, conventional formatting, useful documentation and no accidental generated monoliths. |
| Regeneration | Use Poolster ownership/customization machinery; preserve unrelated files and edited user-owned extension points. Report conflicting edits. |
| Verification | Compile a clean package and execute representative operations, malformed inputs, error paths, provider substitution and regeneration. |

No SDK is required to support every input protocol. A supported pipeline must
publish its precise capability boundary. Parsing or exposing blocks alone does
not count as usable generation.

## 2. API styles

Style controls the public calling surface, not selections, serialization, error
policy, transport policy or return semantics. Changing style MUST NOT change the
request sent or hide errors. Each style uses one shared execution implementation.

| Style | Meaning | Example shape |
| --- | --- | --- |
| `raw` | Standalone functions, static methods or operation modules. Supply the execution context/transport explicitly. No product facade is required. | `readUser(context, variables)` |
| `flat` | One bound client/facade with direct operation methods. Functional languages use one public module with an explicit client value. | `client.readUser(variables)` |
| `idiomatic` | Language-native groups for operations with a bound client, or grouped modules for functional languages. | `client.users.read(variables)` |

These are conceptual examples, not interchangeable signatures. Target-native
casing, async/error handling and parameter shapes follow the language rules below.
A raw execution context may be a configured transport or the SDK's lightweight
client; raw does not mean “return unparsed HTTP bytes.”

The default style MUST be documented per generator. Explicit style selection through
Rust, CLI recipes and npm MUST resolve to the same surface where those entrypoints
support that generator; unknown values must fail rather than fall back.

Flat and idiomatic generation SHOULD retain public raw operations. They MUST NOT
emit independent request/decoder copies for each facade. Group methods forward
to the same raw operation or shared executor.

### Group selection

- HTTP uses explicit group mappings first, then declared resource/tag metadata.
  Any path-based fallback must be documented and deterministic.
- GraphQL uses explicit operation-to-group mappings first. Its default groups
  are `query`, `mutation` and, when enabled, `subscription`. A schema field does
  not establish a `users` resource group.
- RPC groups follow services/capabilities; events follow explicit channel/binding
  identities; workflows follow workflow IDs. They keep their native semantics.
- Custom groups map **group → method → operation identity**, not field names.
  Unknown operations, invalid identifiers and normalized collisions must fail.
- `idiomatic`, `grouped` and `namespaced` may be configuration aliases where
  already supported. They must resolve to the same documented grouped behavior.
- Group configuration with a style that cannot use it must fail, not disappear.

Example GraphQL mapping: `users.read → ReadUser`, `users.rename → RenameUser`.
The mutation method returns the `RenameUser` operation result, including its errors;
putting it in `users` does not make it a query or change retry behavior.

### Arguments and returns

Required arguments MUST remain required. Optional/defaulted arguments MUST permit
omission independently of explicit null. For a parameterless operation, bound
calls SHOULD require no dummy variables object; unavoidable raw empty values must
be documented. Return types must come from the protocol contract:

| Contract | Required distinctions |
| --- | --- |
| HTTP | Declared statuses, content types, headers, bodies, transport errors and supported binary/streaming responses. Body-returning conveniences must preserve access to an envelope/error form. |
| GraphQL | Selection-specific data, data presence, `errors`, `extensions`, transport/protocol failures and partial results. “Require clean data” must reject errors or absent/null data explicitly. |
| RPC | Service/method identities, request/response messages, status, metadata, deadlines and all supported streaming directions. |
| Events | Message payload/headers, channel identity, broker bindings and supported delivery/acknowledgment behavior. |
| Workflows | Resolved source/operation identity, step inputs/outputs, execution order and supported failure policy. |

## 3. Files and source boundaries

The physical layout follows the target language. The conceptual responsibilities
MUST remain distinguishable even when native tooling requires one source directory.

| Responsibility | Splitting rule |
| --- | --- |
| Models | One public model/enum per file where practical. Small related declarations may share a file; recursive references must remain valid. |
| Variables and results | Separate operation-specific types; preserve selection identity. Split nested declarations using stable identities rather than traversal counters. |
| Operations | One named operation per file where practical. Keep its native document/wire definition beside its execution wrapper. |
| Client | Thin constructor/configuration and forwarding surface. Split large facade method sets at declaration boundaries. |
| Groups | Separate group facade/module files. Split large method collections without changing public call paths. |
| Runtime | Separate transport, errors/envelopes, codecs/presence, and supported streaming helpers. Generated entity counts must not inflate common runtime code. |
| Public exports | Thin entry points/barrels/loaders. Keep private helpers private; partition large export collections. |
| Framework integration | Separate adapters and DI/configuration from portable models/operations. Reuse the portable generator. |

### Per-file artifact rules

Directory output SHOULD be the default. Each artifact has one clear role and a
stable entity identity. “One type per file” means one public named model, enum or
alias; an anonymous nested field shape does not automatically need a new file.
Hoisting must preserve its selection/parent identity and recursive references.

| File role | What belongs in it | What must stay elsewhere |
| --- | --- | --- |
| Named model | One public model and its own serialization/validation helpers or native partial declarations. | Unrelated models, operations and client configuration. |
| Enum / named alias | One enum or alias with exact wire values and required conversion helpers. | Client methods and unrelated declarations. |
| Variables / request | One operation's variables or request parameters, with required/optional/default semantics. | Selected result types and transport execution. |
| Result / response | One operation's selected result or declared response shape, with related nested shapes where practical. | Input parameters and shared transport code. |
| Operation | One raw callable and its exact document/wire descriptor; import its models and shared executor. | A copy of the HTTP client, shared codecs or other operations. |
| Client facade | Constructor/configuration and forwarding methods, split into bounded parts when needed. | Embedded copies of every operation document and decoder. |
| Group facade | One group, its client/context reference and forwarding methods. | Guessed protocol semantics or duplicate execution bodies. |
| Runtime helper | One cohesive concern: transport, envelope/errors, presence, scalar codecs, SSE or incremental framing. | Entity-specific models and growing collections of operations. |
| Validator / fixture | One model or operation's validator/fixture when supplied by a companion. | Invented support for unknown scalar domains. |
| Hook / mock / CLI helper | One operation's integration plus imports of the selected SDK symbols. Shared adapters live separately. | Independent renaming or a duplicate SDK implementation. |
| Export / loader | Imports/re-exports and any minimal initialization required by the language. | Business logic, request execution and large model declarations. |
| Manifest / tool config | Native dependency, export, formatter and build settings owned by the package assembler. | Multiple plugins independently overwriting the same manifest. |
| User extension | Clearly marked customization code with create-once/preservation behavior. | Code that regeneration silently overwrites. |

For example, a TypeScript GraphQL package may expose:

```text
src/
  index.ts
  graphql/
    models/User.ts
    models/UserRole.ts
    models/ReadUserVariables.ts
    models/ReadUserResult.ts
    operations/readUser.ts
    client/Client.ts
    groups/Users.ts
    runtime/transport.ts
    runtime/errors.ts
    runtime/presence.ts
    runtime/scalarCodecs.ts
```

This is a target layout illustration, not a claim about today's exact filenames.
A GraphQL schema `User` model is emitted only if the generator actually needs it;
`ReadUserResult` must describe its selection rather than import an over-broad
schema model. Go maps these roles to separate same-package `.go` files; Java maps
public types to class-matching files; other languages follow their profile below.
Do not add empty directories or unused helpers merely to resemble the example.

### Small source-quality rules

- Filenames/extensions and namespace/module paths must agree with target tooling.
  Export identifiers and filenames may have different native casing, but their
  relationship must be deterministic and collision checked.
- Every artifact needs a known owner, stable relative path, exported symbols and
  dependencies. Companion plugins must use that metadata rather than guess paths.
  This is a target requirement; the conformance audit must identify missing metadata.
- Imports must be minimal, ordered by target conventions and rewritten together
  with configured paths. Model files must not depend on operation/client facades;
  shared presence/codec helpers are permitted.
- Use UTF-8, deterministic line endings, a final newline and no trailing whitespace.
  Do not include timestamps, machine-specific paths or secrets in generated banners.
- Preserve useful descriptions/deprecation notes. Escape documentation and source
  literals correctly; input text must not accidentally terminate comments or strings.
- Export only intended public symbols. TypeScript should use named/type-only exports
  where appropriate; native loaders must not depend on filesystem traversal order.
- A generated-file banner should identify Poolster and ownership/customization
  rules without a large repeated header. License notices remain configurable.
- Changing one model/operation should primarily change its own files and necessary
  dependents. Reordering input must not renumber unrelated files or facade parts.

### Configurable layout target

SDK authors SHOULD be able to configure output root, per-role directories,
file/symbol naming, explicit grouping and public export depth through a validated
layout resolver. All affected imports, manifests and companion references must
follow the resolved layout. A filename resolver must not change the wire identity.

Single-file output may be an explicit target-supported option for small packages;
it must not be the default workaround for missing splitting. Incompatible grouping,
namespace or size settings must produce diagnostics. These layout controls are
specification requirements for future adoption, not newly implemented config keys.

Kubb is a useful reference: its TypeScript generator offers directory output with
one file per operation/schema, grouping and configurable barrel exports. Poolster
uses the same small-artifact principle with native language layouts. See
[Kubb's output options](https://www.kubb.dev/plugins/plugin-ts/reference/options).

**Size rules:** the source grouping budget is **128 KiB per file**, measured on
final formatted source. Split collections before exceeding that budget. A facade
SHOULD contain at most **64 forwarding methods per implementation part**; native
partial declarations, traits, mixins or extensions can preserve one public client.
The method count is a target requirement, not a claim about every current plugin.

Do not split string literals, function bodies or atomic declarations arbitrarily.
An indivisible declaration or operation document that exceeds the budget MUST be
retained intact and reported in `.poolster/source-layout-diagnostics.json` with
its path, byte count, budget and reason. A target toolchain limit requires an
explicit error instead. Never truncate valid data to satisfy a size target.

Line length SHOULD target 100 characters, with a soft ceiling of 120. Wire
documents, URLs and inseparable literals may exceed it. Long executable statements
must be wrapped where the target formatter allows. Model constructors, codecs,
control flow and facade methods MUST NOT be deliberately emitted as dense
single-line blocks. A small loader file is not an excuse for one giant runtime.

## 4. Language profiles

The directory names below describe responsibilities, not a promise that every
protocol uses identical paths. Protocol guides specify concrete public import paths.

| Target | Package / namespace rules | Required source layout | Formatting baseline |
| --- | --- | --- | --- |
| TypeScript / JavaScript | npm name separate from exported symbols; ESM public exports; declaration types usable from TS and runtime usable from plain JS. | `models/`, `operations/`, `client/`, `groups/`, `runtime/`; thin root and supported subpath barrels. | Pinned Prettier configuration, readable JSDoc; type-check emitted consumer imports. |
| Rust | Cargo crate name separate from Rust module identifiers; snake_case modules/functions, PascalCase types; deliberate public re-exports. | `src/lib.rs`, protocol module, model/operation/client/group parts, separate runtime helpers. | `rustfmt`; compile and Clippy against declared minimum supported tooling. |
| Go | Valid module path and package identifier; exported PascalCase symbols, private lowerCamelCase symbols; exact JSON/protobuf tags. | Native same-package files: model, operation, group, client, transport and codec files. Do not create directories that accidentally change the Go package boundary. | `gofmt`; generated package compile and `go vet`. |
| Python | Distribution name separate from snake_case import package; PascalCase classes and snake_case modules/functions. | Model and operation packages, thin `__init__.py`, client/group modules or mixins, separate runtime. Include `py.typed` when advertising static typing. | Pinned Ruff formatter; annotations/docstrings and consumer type checks for claimed typing. |
| PHP | Composer name separate from namespace; PascalCase classes, camelCase methods; wire keys unchanged. Public classes must be autoloadable. | `src/Models/`, `Operations/`, `Methods/`, `Groups/`, client and runtime helpers; explicit Composer autoload for functions/shared declarations. | PSR-12 baseline via a pinned formatter; syntax lint every emitted PHP file. |
| Java | Maven/Gradle coordinates separate from lowercase package names; PascalCase public classes, camelCase methods. | Package directory tree with `models`, `operations`, `groups`, client and runtime subpackages; public class filenames match Java rules. | Pinned google-java-format; compile with the advertised Java release. |
| C# | NuGet/package identity separate from PascalCase namespaces/types; PascalCase methods with `Async` for Task-returning APIs. | `Models/`, `Operations/`, `Groups/`, `Client/`, `Runtime/`; partial declarations for large public surfaces. | Pinned .NET SDK formatter and generated `.editorconfig`; build with declared target frameworks. |
| Ruby | Gem name separate from PascalCase module hierarchy; snake_case files/methods. | `lib/<gem>/models`, operations, groups, client/runtime; thin loader; corresponding `sig/` RBS parts. | Pinned RuboCop layout rules; syntax checks and RBS checks for advertised signatures. |
| Swift | SwiftPM product/module identity separate from source type names; PascalCase types, lowerCamelCase members. | `Sources/<Module>/` with model, operation, group, client and runtime files; extensions partition client methods. | Pinned swift-format configuration; SwiftPM build and warnings checks. |
| Elixir | Mix application name separate from PascalCase module hierarchy; snake_case files/functions. | `lib/<app>/models`, operations, client/group modules and runtime helpers; explicit module-based public facades. | `mix format` with pinned Elixir tooling; compilation with warnings as errors. |
| Symfony | Follow PHP profile; bundle namespace separated from portable SDK namespace. Bundle/extension discovery and config alias must agree. | Portable PHP layout plus `Symfony/` transport, bundle and `DependencyInjection/` configuration/extension. | PHP profile; compile a real Symfony container and resolve an autowired consumer. |
| `.NET` alias | Same generated C# package, namespace, API and capabilities. | Same as C#; not a separate language implementation. | Same as C#. |

A formatter is a build-time generation/verification tool, not a runtime dependency.
Its version/configuration must be pinned for deterministic output. If formatting
requires an external tool, generation must expose that requirement or use an
explicit unformatted mode with a diagnostic. It must not silently claim formatted
output when the formatter did not run.

### Native API shape examples

For a fixed GraphQL query named `ReadUser`, the intended conventions are:

| Target | Raw | Flat | Idiomatic default / custom group |
| --- | --- | --- | --- |
| TS / JS | `readUser(context, variables)` | `client.readUser(variables)` | `client.query.readUser(variables)` / `client.users.read(variables)` |
| Rust | `read_user(context, &variables).await` | `client.read_user(&variables).await` | `client.query().read_user(&variables).await` / `client.users().read(&variables).await` |
| Go | `ReadUser(ctx, transport, variables)` | `client.ReadUser(ctx, variables)` | `client.Query().ReadUser(ctx, variables)` / `client.Users().Read(ctx, variables)` |
| Python | `read_user(context, variables)` | `client.read_user(variables)` | `client.query.read_user(variables)` / `client.users.read(variables)` |
| PHP / Symfony | `readUser($client, $variables)` | `$client->readUser($variables)` | `$client->query()->readUser($variables)` / `$client->users()->read($variables)` |
| Java | `ReadUser.execute(context, variables)` | `client.readUser(variables)` | `client.query().readUser(variables)` / `client.users().read(variables)` |
| C# | `ReadUser.ExecuteAsync(context, variables, token)` | `client.ReadUserAsync(variables, token)` | `client.Query.ReadUserAsync(variables, token)` / `client.Users.ReadAsync(variables, token)` |
| Ruby | `Sdk.read_user(context, variables)` | `client.read_user(variables)` | `client.query.read_user(variables)` / `client.users.read(variables)` |
| Swift | `readUser(context, variables)` | `client.readUser(variables)` | `client.query.readUser(variables)` / `client.users.read(variables)` |
| Elixir | `Sdk.Operations.read_user(client, variables)` | `Sdk.read_user(client, variables)` | `Sdk.Query.read_user(client, variables)` / `Sdk.Users.read(client, variables)` |

**These are target shapes, not copy/paste examples of current signatures.** Labels,
argument order, context types and static containers vary by generator; async Swift
calls also require `try await`. Existing signatures must be migrated deliberately,
not changed merely to match this illustration. Language guides contain runnable
examples. Async and cancellation support must match the advertised native driver;
Python's synchronous baseline does not imply an async client.

## 5. Naming, namespaces and imports

Public names MUST derive from stable contract/entity identities and explicit user
configuration. Wire names remain independent from generated names. Preserve aliases,
original field keys, operation names and enum values on the wire.

Name planning MUST reserve all relevant requests before resolving collisions. It
must be deterministic under reordered inputs and plugin scheduling, and account
for reserved words, generated runtime names, case-insensitive filesystems, target
identifier rules and path limits. A deterministic suffix must use stable identity,
not “whichever plugin ran first.” Ambiguous public-name changes require a diagnostic.

Named models SHOULD be shared inside a generated package when their complete
semantics agree. GraphQL selected result objects are not interchangeable just
because they refer to the same schema type. Cross-contract deduplication is opt-in
and requires semantic equivalence; do not merge incompatible wire types.
Anonymous models need stable parent/selection identities and documented hoisting.

Imports MUST compile in a clean consumer. Avoid blanket global exports, duplicate
symbols, dependency cycles caused by generated loaders and imports of parser-library
types. Namespaces/modules must not expose generator implementation details.
Moving public imports or changing names is a compatibility change even if wire
requests still work; include a migration note and a tested compatibility policy.

## 6. Protocol capability requirements

Capabilities must be declared and tested separately; a generic “GraphQL supported”
check does not imply every row below.

| Capability | Conformance requirement |
| --- | --- |
| HTTP SDK | Respect operation schemas, auth, media types and documented request/response controls. No implicit unsafe mutation retries. |
| GraphQL baseline | Validate supplied operations, generate selection-specific variables/results, preserve exact documents and distinguish partial/errors/absent data. |
| GraphQL subscriptions | State transport/protocol dialect, authentication, native iterator/stream API, framing limits, cancellation/cleanup and error behavior. |
| GraphQL incremental | Separate final selected types from incomplete snapshots; identify dialect, validate patches and never decode an incomplete snapshot as a final required model. |
| Scalar codecs | Separate input encoding from output decoding; recurse selected fields/lists/named inputs; preserve absence/null; document supported domain types. |
| Abstract variants | Preserve selected fields/discriminators; reject ambiguity the target cannot represent. Do not guess a variant. |
| RPC/event/workflow outputs | Preserve protocol identity, streaming/bindings/source resolution; list unsupported features explicitly and execute against local protocol implementations. |
| Framework helpers | Consume the selected SDK contract and actual generated symbols. Advertise their own narrower capability boundary, including style/transport restrictions. |

An input/output pair without a matching capability follows Poolster's established
policy: succeed with a warning and a structured skipped-plugin report; an all-skipped
run leaves existing output untouched. A malformed input or unsupported feature in
an otherwise supported pipeline must fail explicitly. These are distinct outcomes.

## 7. Acceptance checklist per generator

Each language/protocol/style combination advertised as conforming MUST have an
auditable record containing the generator revision, fixture/toolchain versions,
capability boundary and check outcomes.

- [ ] Generate from a pinned representative native contract and operation documents.
- [ ] Install/build a clean package through its native manifest and public imports.
- [ ] Verify raw, flat, idiomatic and explicit custom-group calls send equivalent
  requests and produce equivalent results/errors.
- [ ] Exercise required, optional, nullable, recursive, enum and supported abstract
  values; check real decoded values, not only compilation.
- [ ] Exercise auth, partial/application errors, malformed responses, transport
  failures, cancellation and stream cleanup for advertised capabilities.
- [ ] Verify provider substitution, transformed-contract selection and required
  block completeness/provenance when the generator consumes blocks.
- [ ] Reorder declarations/operations, regenerate and compare symbols, files and
  manifests; verify no unrelated file deletion or ownership loss.
- [ ] Generate a large fixture; check source byte budgets, facade splitting,
  atomic-declaration diagnostics and bounded filenames on formatted output.
- [ ] Run the target formatter/checker with pinned configuration; lint/type-check
  where applicable and compile documented examples.
- [ ] Record passes, failures, ignored/skipped tests and infrastructure retries
  separately. Unexecuted tests are not passes.

## 8. Current adoption and follow-up work

The GraphQL layout work already separates entity/operation/client/runtime files in
all ten SDK languages. Symfony reuses PHP output and adds separate framework files.
This is a useful foundation, not full conformance to every rule above.

Concrete adoption work:

1. **Formatting first:** remove compact one-line PHP model/method/runtime generation,
   including Symfony's reused SDK. Audit all other targets against their formatter
   profile and verify final emitted source, not Rust template formatting.
2. **Enforce per-file roles and final-source budgets:** confirm every generator measures post-format
   bytes and partitions facades/exports consistently; keep atomic diagnostics.
3. **Publish naming and style conformance:** audit stable name reservation, grouped
   mappings, raw availability and clean public imports per target and protocol.
4. **Add a conformance ledger:** link each acceptance requirement to executed tests
   and mark `implemented`, `gap`, or `unsupported`. Reuse existing evidence rather
   than rewriting historical verification results.
5. **Gate releases:** advertise only the capability/style/quality combinations that
   pass their acceptance checklist; record justified language-specific exceptions.

Current protocol limits remain in the [matrix](../plugin-support-matrix.md),
[GraphQL capability guide](../reference/outputs/graphql-capabilities.md) and
[native pipeline backlog](../reference/inputs/native-pipelines.md#remaining-work).
