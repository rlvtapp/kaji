# Poolster implementation map

Status: proposal only. Keep developing under Kaji until a rename and release
are explicitly chosen. This map supplements the [package naming proposal](poolster-package-structure.md)
with the concrete work required by today's repository.

## Keep the existing boundaries

| Current path | Proposed Cargo package | Responsibility |
| --- | --- | --- |
| `crates/kaji-core/` | `poolster-core` | Shared AST, contracts, plugin engine, file ownership. |
| `crates/kaji/` | `poolster` | Rust embedding API and profiles; future crates.io package. |
| `crates/kaji-cli/` | `poolster-cli` | Internal Rust command and `poolster` binary for npm/PyPI bundles. |
| `crates/kaji-node/` | `poolster-node` | Internal NAPI addon; not a crates.io package. |
| `crates/inputs/*/`, `crates/kaji-inputs/` | `poolster-input-*`, `poolster-inputs` | Native parsers and optional bundle. |
| `crates/plugins/*/` | `poolster-plugin-*` | Native language and output plugins. |

Cargo package names can change without moving these directories. Keep paths
stable for the first naming change, then decide whether directory moves help.
The Rust SDK should re-export the core API without requiring every language
plugin by default. Rust consumers add the plugin crates they select. The CLI
and NAPI addon can link the built-in plugin set they expose. This requires a
coordinated API migration through the CLI, NAPI addon, examples, and tests
because `kaji` currently re-exports all 16 output plugin crates.

The other source and distribution paths keep distinct jobs:

| Current path | Job |
| --- | --- |
| `openapi/` | Go OpenAPI compiler executable used by the CLI and Node SDK. |
| `packages/cli/` | npm CLI launcher and prebuilt platform packages. |
| `packages/cli/sdk/` | Node SDK, JS plugin engine, and NAPI platform packages. |
| `packages/node-plugins/` | Individually installable JS plugin factories and optional bundle. |
| `packages/python/` | PyPI CLI launcher and platform wheels. |
| `packages/unplugin-kaji/`, `packages/github-action/`, `packages/gitlab-ci/` | Build-tool and CI integrations. |

## Public installs by ecosystem

| Ecosystem | CLI | Embedding |
| --- | --- | --- |
| npm | `poolster` | `@relevate/poolster` and explicitly selected `@relevate/poolster-plugin-*` packages |
| PyPI | `poolster` platform wheel | No Python embedding package proposed |
| crates.io | No CLI package | `poolster` and explicitly selected `poolster-plugin-*` / `poolster-input-*` crates |

The npm CLI, Node SDK, and PyPI CLI already have separate packaging paths.
`packages/cli/sdk/` is an independent Node package nested under the CLI
directory; move it to a sibling such as `packages/node-sdk/` only in a separate
layout change. Folder names do not determine published package names.

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

This is a target API sketch, not a command that works with today's Kaji names.

## Rust distribution boundary

All 26 Rust workspace crates currently have `publish = false`, and their local
dependencies use paths without registry versions. The SDK facade currently
re-exports all 16 output plugins, so a future crates.io `poolster` package
would pull in the full plugin graph. Make plugin selection explicit before
publishing it. Publishable path dependencies need registry versions and an
ordered release. Keep `poolster-node` unpublished because it ships via npm.

The Rust CLI remains a workspace implementation, but it is not a public Rust
install. The npm and PyPI packages already bundle the prebuilt Rust executable
and adjacent Go `kaji-openapi` compiler. Keep that two-binary runtime layout;
there is no need to embed Go in the executable for these distribution paths.
Do not advertise `cargo install poolster`: that name is reserved for the SDK.
Homebrew, a shell installer, and GitHub Release downloads can be considered
later, after the npm/PyPI CLI distribution is established.

## Suggested sequence

1. **Untangle source layout first.** Apply the [code organization guidelines](../code-organization.md)
   to the CLI and one plugin; move one plugin's templates and probes out of
   `src/`. Do not combine this with package renames.
2. **Slim the Rust SDK facade.** Make plugin selection explicit for Rust users,
   retain clear migration examples, and test Rust embedding with only one
   language and one input plugin installed.
3. **Rename manifests and public commands together.** Update Cargo, npm, and
   PyPI names, JS config discovery, environment variables, examples, docs, and
   release metadata from one checked name map. Keep the current directories
   initially.
4. **Prepare only the Rust SDK for crates.io.** Add versioned publishable
   dependencies, test `cargo package`, and test a fresh consumer using
   `poolster` with one explicitly selected plugin. Keep the internal CLI crate
   unpublished; add an SDK-only crates.io job only after that graph is ready.
5. **Release each ecosystem deliberately.** Publish Rust dependencies before
   the SDK; publish npm platform packages before launchers; publish PyPI wheels
   from the existing platform matrix. Verify exact versions and installation
   behavior before enabling any public publish job.

The current release workflow publishes npm packages and PyPI wheels but has no
crates.io job. Nothing in this map enables one or publishes a package.
