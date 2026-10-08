# Poolster implementation map

Status: proposal only. Keep developing under Kaji until a rename and release
are explicitly chosen. This map supplements the [package naming proposal](poolster-package-structure.md)
with the concrete work required by today's repository.

## Keep the existing boundaries

| Current path | Proposed Cargo package | Responsibility |
| --- | --- | --- |
| `crates/kaji-core/` | `poolster-core` | Shared AST, contracts, plugin engine, file ownership. |
| `crates/kaji/` | `poolster-sdk` | Rust embedding API and profiles. |
| `crates/kaji-cli/` | `poolster` | Rust command and `poolster` binary. |
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
| crates.io | `poolster` binary crate, when its compiler is self-contained | `poolster-sdk` and explicitly selected `poolster-plugin-*` / `poolster-input-*` crates |

The npm CLI, Node SDK, and PyPI CLI already have separate packaging paths.
`packages/cli/sdk/` is an independent Node package nested under the CLI
directory; move it to a sibling such as `packages/node-sdk/` only in a separate
layout change. Folder names do not determine published package names.

The proposed Rust embedding experience makes plugin selection visible in the
dependency list and in code:

```sh
cargo add poolster-sdk poolster-plugin-typescript
```

```rust
use poolster_sdk::{ProfileSet, generate};
use poolster_plugin_typescript as typescript;

let profiles = ProfileSet::new("generated")
    .package(typescript::package("typescript").with(typescript::sdk()));
let files = generate(&api, profiles)?;
```

This is a target API sketch, not a command that works with today's Kaji names.

## The crates.io blocker

All 26 Rust workspace crates currently have `publish = false`, and their local
dependencies use paths without registry versions. The `kaji-cli` binary also
locates a separate `kaji-openapi` executable beside itself. npm platform
packages and PyPI wheels bundle that Go executable; a plain `cargo install`
would install only the Rust binary and leave ordinary OpenAPI generation
unable to find its compiler.

Do not advertise `cargo install poolster` as a working distribution until both
conditions are solved:

1. Give the Cargo CLI a reliable OpenAPI compiler strategy. Prefer a Rust-native
   compiler path or another self-contained, tested solution. Requiring users to
   install Go or fetch an unchecked binary at first run would make this install
   path worse than the existing npm/PyPI packages.
2. Decide which Rust crates to publish. With the current dependency graph, the
   CLI reaches the core, SDK facade, input bundle, five input parsers, and all
   16 output plugins. Publishable path dependencies need registry versions and
   an ordered release. Keep `poolster-node` unpublished because it ships via npm.

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
4. **Make crates.io installation complete.** Solve the Go compiler sidecar,
   add versioned publishable dependencies, and test `cargo package`,
   `cargo install --path crates/kaji-cli`, and a fresh consumer using
   `poolster-sdk`. Only then add a crates.io publish job.
5. **Release each ecosystem deliberately.** Publish Rust dependencies before
   the SDK and CLI; publish npm platform packages before launchers; publish
   PyPI wheels from the existing platform matrix. Verify exact versions and
   installation behavior before enabling any public publish job.

The current release workflow publishes npm packages and PyPI wheels but has no
crates.io job. Nothing in this map enables one or publishes a package.
