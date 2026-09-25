# Contributing to Kaji

Thanks for contributing. Kaji values small, reviewable changes with clear
behavioural tests.

1. Open an issue before a broad API or generator-design change.
2. Keep generated output deterministic and add a focused regression test for
   every observable change.
3. Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   and `cargo test --workspace` before opening a pull request.
4. Do not add generated SDK build artifacts, local `target` directories, or
   copied compatibility corpora to the repository.

New language targets belong under `crates/plugins/<language>`. Keep shared
OpenAPI semantics in `kaji-core`; do not make the core depend on a particular
SDK runtime.
