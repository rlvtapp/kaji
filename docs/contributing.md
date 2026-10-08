# Contributing

[Docs home](README.md) · [Plugin authoring](typed-plugins.md) · [Maintainer checks](verification.md#maintainer-checks)

This guide is for developing Poolster itself. To use it on an API, start with the
[CLI](cli.md) or [Rust getting-started guide](getting-started.md).

## Workspace

| Path | Purpose |
| --- | --- |
| `openapi/` | Bundled Go OpenAPI compiler and its tests. |
| `crates/core` | Language-neutral AST, artifact adapter, typed plugin engine, SDK semantics, and mock primitives. |
| `crates/facade` | First-party composition facade and mock package plugin. |
| `crates/plugins/*` | Language implementations and language-owned configuration. |
| `crates/cli` | Native command-line application. |
| `packages/npm/cli` | Thin Node launcher and platform package build tooling. |
| `packages/npm/sdk` | Node SDK with NAPI bindings and JavaScript plugins. |
| `docs/` | User guides, configuration, architecture, and verification. |

## Local checks

Use Rust/Cargo (see the workspace `rust-version`), the Go version declared in
`openapi/go.mod`, and Node for launcher tests. Generated SDK checks also need
the target language's toolchain.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd openapi
go test ./...
```

From the repository root, run optional broader checks:

```sh
cargo test -p poolster --test sdk_to_mock_contract -- --ignored
bash scripts/test-large-graph.sh
node --test packages/npm/cli/test/*.test.cjs
```

Check generated Fetch SDK consumers with a locally installed TypeScript compiler
(Node is required; this test does not download dependencies):

```sh
POOLSTER_TSC_JS=/path/to/typescript/lib/tsc.js \
  cargo test -p poolster-plugin-typescript generated_fetch_consumer_compiles_with_strict_typescript -- --ignored
```

This compiles both raw operations and a full namespaced client with strict
TypeScript settings. It is ignored by the default Rust suite because it needs
the external compiler.

The full Graph check downloads a large public specification and leaves its
temporary workspace for inspection. It does not call Microsoft Graph. See
[large specs](large-specs.md) for resource use.

## Change guidelines

Keep language-specific code and options in the corresponding plugin crate.
Use the normalized AST and typed contracts rather than adding language branches
to core. Declare file ownership and dependencies; do not overwrite another
plugin's output silently.

Add a focused regression test for fixes. When generated output intentionally
changes, review the all-target golden diff and update it alongside behavioral
tests. A snapshot alone is not proof that the generated code compiles.

User-facing configuration changes need corresponding examples and reference
updates. Poolster is pre-1.0; avoid retaining unused compatibility layers.

See [code organization](code-organization.md), [architecture](architecture.md),
[plugin authoring](typed-plugins.md), and [verification](verification.md) before
expanding the generation pipeline.
Native npm builds and release ordering are documented in [the CLI guide](cli.md#npm-layout-and-release-preparation).
Do not publish packages as part of an ordinary build or test.
