# Poolster: name and package structure proposal

Status: **selected plan, implemented in source on `poolster-sdk-layout`** ·
2026-10-08. Publication remains disabled. The GitHub repository is still
`rlvtapp/kaji`; its URL and source directory paths remain until a separate
repository move. This document retains the original naming tradeoffs and
release checklist.

## Selected name: Poolster

Use **Poolster** for the whole product in this worked scenario. The name describes
the stable center that coordinates source inputs, generation plugins, and output
packages. It is a normal Dutch word that is reasonably pronounceable in English.

Use one name consistently in the eventual public surface:

| Surface | Proposed name |
| --- | --- |
| Product and documentation | Poolster |
| Command | `poolster` |
| npm CLI package | `poolster` |
| npm Node SDK | `@relevate/poolster` |
| JavaScript config | `poolster.config.mjs` or `poolster.config.ts` |
| Rust CLI workspace crate | `poolster-cli` with a `poolster` binary; kept off crates.io |
| Rust embedding crate on crates.io | `poolster` |
| PyPI CLI wheel | `poolster` with a `poolster` console command |
| GitHub repository, when renamed | `rlvtapp/poolster` |

The CLI and SDK remain separate installs. `poolster` contains the command
launcher and its native executable. `@relevate/poolster` contains the embedding
API and NAPI runtime. Neither package depends on the other for its basic use.
The root import `@relevate/poolster` is the canonical SDK entry point; `/sdk`
can be retained as a harmless alias if migration experience calls for it. Do not
create a second npm package named `poolster-sdk`.

### Earlier naming alternative: Unii

The package architecture does not depend on the word Poolster. If **Unii** is
chosen instead, substitute these roots throughout the maps below:

| Surface | Poolster scenario | Unii variant |
| --- | --- | --- |
| Product and command | Poolster / `poolster` | Unii / `unii` |
| npm CLI and SDK | `poolster`, `@relevate/poolster` | `unii`, `@relevate/unii` |
| npm plugins and inputs | `@relevate/poolster-plugin-*`, `@relevate/poolster-input-*` | `@relevate/unii-plugin-*`, `@relevate/unii-input-*` |
| npm native packages | `@relevate/poolster-cli-*`, `@relevate/poolster-node-*` | `@relevate/unii-cli-*`, `@relevate/unii-node-*` |
| Rust CLI workspace crate, Rust SDK, and PyPI | `poolster-cli`, `poolster`, `poolster` | `unii-cli`, `unii`, `unii` |
| Config | `poolster.config.mjs` | `unii.config.mjs` |

Unii is shorter and easy to say across languages; its double `i` needs a little
more care when spoken or searched. It also shares Kubb's four-letter shape and
double final letter. In the same code-generation category, that could make the
branding feel derivative even though the spelling and sound differ. This is a
subjective product consideration, not a claim about legal similarity. A
read-only npm registry check on
2026-10-08 found [metadata for a fully unpublished `unii@0.0.1`](https://registry.npmjs.org/unii),
with **no active versions**. This is not evidence of a currently installable
package. Whether our account can claim the unscoped name must be verified
before publication. An unrelated [UNii security product](https://unii-security.com/en/)
also uses the word; this calls for normal brand clearance, not an automatic
rejection. Poolster was selected for the source migration; registry and legal
clearance still precede any public release.

## Public npm packages

The proposed package names below describe the final rename, not packages to
publish now. All plugin packages must be added to the project config explicitly;
installing one never activates it.

### CLI, SDK, and platform binaries

| Current | Proposed | Responsibility |
| --- | --- | --- |
| `kajicli` | `poolster` | Thin command launcher; binary name `poolster`. No SDK dependency for native or static-config use. |
| `@relevate/kaji` | `@relevate/poolster` | Node SDK, config types, JS plugin engine, NAPI binding loader, `createPoolster`/`generate`. No public executable. |
| `@relevate/kajicli-<platform>` | `@relevate/poolster-cli-<platform>` | Native Rust CLI and its OpenAPI compiler for one platform. Optional dependency of `poolster`. |
| `@relevate/kaji-<platform>` | `@relevate/poolster-node-<platform>` | NAPI addon and its OpenAPI compiler for one platform. Optional dependency of `@relevate/poolster`. |

`<platform>` initially means `darwin-arm64`, `darwin-x64`,
`linux-x64-gnu`, or `win32-x64-msvc`, matching the current build matrix.
Platform packages are implementation details. The CLI install does not pull in
the SDK addon; the SDK install does not pull in the CLI binary. Someone who
installs both may get two OpenAPI compiler copies under this initial layout.
Extracting a shared compiler package is worth considering only after measuring
that cost against the extra release and resolution complexity.

The former `@relevate/kaji` exposed `kaji-sdk`. The renamed `poolster` command
discovers `poolster.config.mjs`, `.cjs`, and `.js` for JavaScript recipes; the
Rust CLI reads `poolster.json` for static configuration. A TypeScript config
loader remains deferred until its runtime strategy and supported Node versions
have tests. A JavaScript config loads the separately installed SDK; the Node SDK
also works directly without installing the CLI.

### Language output plugins

Each language keeps its own installable package and exported factory:

| Current prefix | Proposed prefix | Exact suffixes |
| --- | --- | --- |
| `@relevate/kaji-plugin-` | `@relevate/poolster-plugin-` | `typescript`, `rust`, `go`, `python`, `php`, `java`, `csharp`, `elixir`, `ruby`, `swift` |

For example, `@relevate/poolster-plugin-typescript` exports
`pluginTypeScript()` and is selected in the config. Plugin packages declare a
compatible SDK peer dependency; they do not depend on `poolster` or trigger
generation as an install side effect.

### Input plugins

| Current prefix | Proposed prefix | Exact suffixes |
| --- | --- | --- |
| `@relevate/kaji-input-` | `@relevate/poolster-input-` | `graphql`, `asyncapi`, `arazzo`, `protobuf`, `capnproto` |

Input plugins remain a separate role from output plugins. The config selects
one input provider and then selects output plugins. A native input that only
publishes its format-specific contract can feed JS output consumers; it does
not automatically become the normalized HTTP contract expected by HTTP SDK
renderers. The package documentation must state this capability boundary.

### Auxiliary output plugins and convenience bundle

| Current prefix/name | Proposed prefix/name | Exact suffixes or role |
| --- | --- | --- |
| `@relevate/kaji-plugin-` | `@relevate/poolster-plugin-` | `zod`, `faker`, `msw`, `cypress`, `react-query`, `vue-query`, `swr` |
| `@relevate/kaji-plugins` | `@relevate/poolster-plugins` | Optional factory bundle for the 10 languages, 5 inputs, and 7 auxiliaries. |

Keep the bundle because it offers the requested “install all, select manually”
workflow. It exports factories only. Individual packages remain the documented
default for projects that want a clear dependency list. A bundled import still
requires each factory to appear in `plugins` or `input.plugin` in the config.

### Integrations and internal tooling

| Current | Proposed | Publication |
| --- | --- | --- |
| `@relevate/unplugin-kaji` | `@relevate/unplugin-poolster` | Public bundler adapter; peer or executable lookup follows the new CLI. |
| PyPI `kaji-cli` in `packages/python` | `poolster` | Platform wheels for Python users; console command and Python module `poolster`. No Node dependency. |
| GitHub Action in `packages/integrations/github` | Poolster action and inputs | Repository action, not an npm SDK dependency. |
| GitLab template in `packages/integrations/gitlab` | Poolster template and command | Template, not an npm SDK dependency. |
| `@relevate/kaji-spec-sync-action` | `@relevate/poolster-spec-sync-action` | Keep private unless there is a separate reason to publish. |
| `@relevate/kaji-sdk-publish-action` | `@relevate/poolster-sdk-publish-action` | Keep private unless there is a separate reason to publish. |
| `@kaji/runtime-contract`, `@kaji/postman-execute`, `@kaji/github-app-broker` | `@poolster/...` | Internal test or service packages; keep `private: true`. |

## Rust workspace names

The current crates all have `publish = false`. Rename them together for source
coherence when the product rename actually happens; a crates.io migration is a
separate decision. Paths can move independently from Cargo package names.
Reserve the crates.io name `poolster` for the embedding SDK. Keep the Rust CLI
as an internal workspace crate named `poolster-cli`, producing the `poolster`
executable bundled with the Go compiler in npm platform packages and PyPI
wheels. Do not publish the CLI crate to crates.io in this plan.

| Current | Proposed | Role |
| --- | --- | --- |
| `kaji` | `poolster` | Public-facing Rust embedding facade and profiles; future crates.io package. |
| `kaji-core` | `poolster-core` | Engine, contracts, file ownership. |
| `kaji-cli` | `poolster-cli` | Internal Rust command crate, with `[[bin]] name = "poolster"`. |
| `kaji-node` | `poolster-node` | Internal NAPI bridge. |
| `kaji-inputs` | `poolster-inputs` | Native input bundle. |
| `kaji-input-<format>` | `poolster-input-<format>` | The five native parsers listed above. |
| `kaji-plugin-<name>` | `poolster-plugin-<name>` | `rust`, `typescript`, `typescript-cli`, `rust-cli`, `go`, `python`, `php`, `symfony`, `terraform`, `postman`, `java`, `csharp`, `dotnet`, `elixir`, `ruby`, `swift`. |

Rust crate names and npm names need not have one-to-one publication. In
particular, `poolster-plugin-typescript-cli`, `-rust-cli`, `-symfony`,
`-terraform`, `-postman`, and `-dotnet` are Rust workspace crates here, not
promised npm packages. The Go OpenAPI compiler can remain an internal build
component; rename its module/import path only if the repository itself moves.

## API and dependency boundaries

```text
poolster (CLI) ───────► its CLI platform binary
      │
      └── for JS/TS config only: load installed @relevate/poolster

@relevate/poolster (SDK) ──► its Node platform addon
      ▲
      │ compatible peer
language / input / auxiliary packages and optional plugins bundle

Rust poolster command ──► poolster SDK facade
Rust poolster SDK facade ──► poolster-core + selected Rust plugins
```

1. The SDK owns `defineConfig`, JS/TS types and JSDoc, `definePlugin`,
   `defineInputPlugin`, contract handles, plugin ordering, embedding, and safe
   materialization. The CLI calls this SDK only for JS/TS configs.
2. Language packages own their factory, options, documentation, and tests.
   Installing a package does not mutate a global registry or silently add a
   language to a generation run.
3. Input packages own the parser selection and source-specific contract shape.
   A generator can consume only a contract it explicitly supports.
4. Third-party JS plugins can implement the JS plugin API with no native build.
   Rust plugins are linked at compile time for Rust consumers.
5. Built-in plugin versions should be tested as one compatibility set with the
   SDK. Third-party plugin versions can evolve independently against a declared
   plugin API compatibility range. Platform binary versions must match their
   JavaScript launcher exactly.

**Native modularity caveat.** Today the individual npm packages are lightweight
selectors for Rust implementations compiled into the single Kaji NAPI addon.
Installing only the TypeScript factory does **not** make the SDK binary contain
only TypeScript. A future design for true per-language native packages needs a
versioned boundary where each addon accepts a normalized input contract and
returns owned files, so the JS orchestrator can combine outputs safely. Loading
arbitrary Rust trait objects from separately installed dynamic libraries is not
the proposal. Until that renderer boundary exists, document the packages as
independent *selection surfaces*, not independent native payloads. This also
applies to the Rust auxiliary plugins.

## Proposed user flows

CLI with native/static configuration:

```sh
npm install --save-dev poolster
npx poolster generate --config poolster.json
```

JS config and explicit TypeScript plugin:

```sh
npm install --save-dev poolster @relevate/poolster @relevate/poolster-plugin-typescript
npx poolster generate --config poolster.config.mjs
```

```js
// poolster.config.mjs — proposed API after the rename
import { defineConfig } from '@relevate/poolster';
import { pluginTypeScript } from '@relevate/poolster-plugin-typescript';

export default defineConfig({
  input: './openapi.yaml',
  output: './generated',
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginTypeScript()],
});
```

Embedding without the CLI:

```js
import { createPoolster } from '@relevate/poolster';

const result = await createPoolster(config).generate({ write: false });
```

The snippets show the source interface on the implementation branch; these
packages have not been published. A `.ts` config needs a defined loader
strategy and tests on every supported Node version before being advertised.
Plain `.mjs` plus JSDoc and exported types work without that step.
The parallel installation names would be `npx poolster` for npm CLI users,
`pip install poolster` for Python CLI users, and `cargo add poolster` for Rust
embedding. Homebrew and a shell installer can be considered later. Neither is
part of the first distribution plan.

## Repository layout

```text
crates/{core,facade,cli,node,inputs,plugins}/...  # Cargo package names carry poolster-
crates/inputs/bundle/                          # poolster-inputs
packages/npm/cli/                              # poolster
packages/npm/sdk/                              # @relevate/poolster
packages/npm/plugins/{typescript,...}/         # individual output plugins
packages/npm/inputs/{graphql,...}/             # individual input plugins
packages/npm/plugins-all/                      # optional bundle
packages/npm/platform/{cli,node}/<platform>/   # generated native packages
packages/integrations/{unplugin,github,gitlab}/
packages/internal/{runtime-contract,...}/
packages/python/                              # PyPI launcher and wheels
openapi/                                      # Go OpenAPI compiler
```

This layout is implemented on `poolster-sdk-layout`. Source folder names stay
separate from published package identities. The native platform directories
contain ignored build output. Integrations are grouped under
`packages/integrations`; release tools, contract tooling and internal services
are grouped under `packages/internal`. The shared factory generator lives at
`packages/npm/generate-plugins.mjs` and owns both plugin and input factories.

## Release checklist after the source rename

1. Decide the brand and check npm, PyPI, crates.io, the BOIP/KVK name checker,
   repository names, and domains. A registry 404 is not a reservation or legal
   clearance. Existing uses of “Poolster” include an unrelated
   [older software title](https://www.jhc-software.com/snook_pool.shtml);
   use the [official KVK/BOIP checker](https://www.kvk.nl/en/starting/name-checker-tool-page/)
   before public branding. Read-only registry checks on 2026-10-08 returned
   404 for npm `poolster` and `@relevate/poolster`. The official
   [PyPI](https://pypi.org/pypi/poolster/json) and
   [crates.io](https://crates.io/api/v1/crates/poolster) endpoints also returned
   404 for `poolster` on 2026-10-08; those results can change.
   If Unii is preferred, verify that the fully unpublished npm name can be
   claimed by our account; the remaining package split is the same.
2. Review the coordinated npm, Cargo, PyPI, config, binary, example, docs, and
   CI naming changes on `poolster-sdk-layout`. Source directories remain
   stable. The [name map](poolster-name-map.json) records public old-to-new names.
3. Test CLI-only, SDK-only, and combined installs on every supported platform.
   Exercise JS + Rust input/output plugins together; run emitted SDK builds
   and cross-language runtime probes. Verify no plugin is activated by install.
4. Decide how the existing published `@relevate/kaji@0.4.x` CLI is migrated.
   The current worktree already proposes `@relevate/kaji` as an SDK at 0.5;
   that is a breaking npm change even without the Poolster rename. Write one
   explicit migration path rather than two successive public renames.
5. Publish native platform packages before their launchers only after the
   entire package graph and migration notes are approved. Keep release jobs
   gated until then.

## Selected package structure

The selected structure has **one product name and two top-level npm packages**:
an unscoped package for the command and `@relevate/<name>` for the SDK, followed
by individually selected input/output plugins and an optional all-plugins
bundle. Poolster is selected; the Unii substitutions above record the earlier
alternative. The most significant architecture
decision is whether true per-language native payloads must ship with the first
release under a new name;
the current packages offer independent selection but a shared native addon.
