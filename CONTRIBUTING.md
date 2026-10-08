# Contributing to Poolster

Thanks for contributing. Poolster values small, reviewable changes with clear
behavioural tests.

Start with the [contributor guide](docs/contributing.md) for workspace structure,
toolchains, checks, plugin boundaries, and release preparation.
Follow the [code organization guidelines](docs/code-organization.md) when
adding modules, generated-language templates, or test fixtures.

1. Open an issue before a broad API or generator-design change.
2. Keep generated output deterministic and add a focused regression test for
   every observable change.
3. Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   and `cargo test --workspace` before opening a pull request.
4. Do not add generated SDK build artifacts, local `target` directories, or
   copied compatibility corpora to the repository.

New language targets belong under `crates/plugins/<language>`. Keep shared
OpenAPI semantics in `poolster-core`; do not make the core depend on a particular
SDK runtime.
