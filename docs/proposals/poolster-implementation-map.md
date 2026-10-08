# Poolster implementation map

Status: package rename and repository layout implemented on `poolster-sdk-layout`; publication disabled.
This map supplements the [package naming plan](poolster-package-structure.md)
with the boundaries and remaining release work in this repository.

## Keep the existing boundaries

| Current path | Proposed Cargo package | Responsibility |
| --- | --- | --- |
| `crates/core/` | `poolster-core` | Shared AST, contracts, plugin engine, file ownership. |
| `crates/facade/` | `poolster` | Rust embedding API and profiles; future crates.io package. |
| `crates/cli/` | `poolster-cli` | Internal Rust command and `poolster` binary for npm/PyPI bundles. |
| `crates/node/` | `poolster-node` | Internal NAPI addon; not a crates.io package. |
| `crates/inputs/*/`, `crates/inputs/bundle/` | `poolster-input-*`, `poolster-inputs` | Native parsers and optional bundle. |
| `crates/plugins/*/` | `poolster-plugin-*` | Native language and output plugins. |

The folders now follow the repository layout in the package naming plan.
Cargo package names remain independent of their source directory names.
The Rust SDK facade starts without language plugins by default. Rust consumers
add the plugin crates they select. The CLI and NAPI addon link the built-in
plugin set they expose. Rust examples and tests exercise this explicit
dependency boundary.

The other source and distribution paths keep distinct jobs:

| Current path | Job |
| --- | --- |
| `openapi/` | Go OpenAPI compiler executable used by the CLI and Node SDK. |
| `packages/npm/cli/` | npm CLI launcher and prebuilt platform packages. |
| `packages/npm/sdk/` | Node SDK, JS plugin engine, and NAPI platform packages. |
| `packages/npm/plugins/` | Individually installable JS plugin factories and optional bundle. |
| `packages/python/` | PyPI CLI launcher and platform wheels. |
| `packages/integrations/unplugin/`, `packages/integrations/github/`, `packages/integrations/gitlab/` | Build-tool and CI integrations. |

## Public installs by ecosystem

| Ecosystem | CLI | Embedding |
| --- | --- | --- |
| npm | `poolster` | `@relevate/poolster` and explicitly selected `@relevate/poolster-plugin-*` packages |
| PyPI | `poolster` platform wheel | No Python embedding package proposed |
| crates.io | No CLI package | `poolster` and explicitly selected `poolster-plugin-*` / `poolster-input-*` crates |

The npm CLI and Node SDK are sibling packages under `packages/npm/`; PyPI
keeps its launcher under `packages/python/`. Generated native packages belong
under `packages/npm/platform/{cli,node}/`. Input factories, output factories
and the optional bundle occupy distinct folders under `packages/npm/`.
Folder names do not determine published package names.

The proposed Rust embedding experience makes plugin selection visible in the
dependency list and in code:

```sh
cargo add poolster poolster-plugin-typescript
```

```rust
use poolster::{ProfileSet, generate};
use poolster_plugin_typescript as typescript;

let profiles = ProfileSet::new("generated")
    .package(typescript::package("typescript").with(typescript::sdk()));
let files = generate(&api, profiles)?;
```

This is a compact API sketch. Packages remain unpublished under the new names.

## Rust distribution boundary

The SDK, core, input and output plugin crates have versioned local dependencies
and an ordered crates.io release job. The SDK facade has a slim default feature
set. Keep `poolster-node` unpublished because it ships via npm.

The Rust CLI remains a workspace implementation, but it is not a public Rust
install. The npm and PyPI packages already bundle the prebuilt Rust executable
and adjacent Go `poolster-openapi` compiler. Keep that two-binary runtime layout;
there is no need to embed Go in the executable for these distribution paths.
Do not advertise `cargo install poolster`: that name is reserved for the SDK.
Homebrew, a shell installer, and GitHub Release downloads can be considered
later, after the npm/PyPI CLI distribution is established.

## Suggested sequence

1. **Continue source layout cleanup.** Apply the [code organization guidelines](../code-organization.md)
   to the CLI and plugins; move templates and probes out of `src/` as those
   modules are split.
2. **Verify the slim Rust SDK facade.** Test Rust embedding with only one
   language and one input plugin installed.
3. **Review the implemented rename and layout.** Cargo, npm, and
   PyPI names, JS config discovery, environment variables, examples, docs, and
   release metadata follow the package naming plan and repository layout.
4. **Validate the prepared Rust SDK publication.** Versioned dependencies and
   an SDK-only crates.io job are implemented. Check packaging and a fresh
   consumer using `poolster` with one explicitly selected plugin before release.
   The internal CLI and Node bridge remain unpublished.
5. **Release each ecosystem deliberately.** Publish Rust dependencies before
   the SDK; publish npm platform packages before launchers; publish PyPI wheels
   from the existing platform matrix. Verify exact versions and installation
   behavior before enabling any public publish job.

The release workflow defines npm, PyPI and crates.io publication jobs for
verified release tags. The crates.io job also requires `CRATES_IO_TOKEN`.
Preparing these jobs does not publish a package.
