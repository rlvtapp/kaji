# Kaji

Kaji is a Rust-native OpenAPI toolkit for generating polished, idiomatic SDKs
and contract mocks. It keeps the generator core independent of JavaScript and
lets one OpenAPI model produce release-ready packages for Rust, TypeScript,
Go, Python, PHP, Java, .NET, and Elixir.

> Kaji is pre-1.0. The public workspace is ready for collaboration; package
> publication and the stable configuration format are intentionally still in
> progress.

## What it does

- Generates typed SDK packages with flat or resource-namespaced clients.
- Supports Rust/Reqwest, TypeScript/Fetch, TypeScript/Axios, Go, Python, PHP,
  Java, .NET, and Elixir targets.
- Carries OpenAPI auth, declared errors, retries, pagination, streaming, file
  media, and runtime hooks into supported SDKs.
- Generates a language-neutral `httpmock` package from the same contract, so
  every generated SDK can test against one local service.
- Exposes a small Rust plugin API and a target-neutral AST for new generators.

## Workspace

| Path | Purpose |
| --- | --- |
| `crates/kaji-core` | AST, OpenAPI sidecar adapter, generation engine, SDK and mock primitives |
| `crates/kaji` | First-party profile builder and standalone mock-server package |
| `crates/plugins/*` | Native SDK package generators by language |
| `docs/` | Architecture and contract-mocking notes |

## Quick start

```rust
use kaji::{generate, ProfileSet};
use kaji_core::Api;

let api = Api::default(); // Build or adapt this from an OpenAPI contract.
let artifacts = generate(
    &api,
    ProfileSet::new("artifacts")
        .rust()
        .typescript_fetch()
        .python()
        .mock_server(),
)?;
```

`artifacts` contains isolated packages per SDK target plus
`artifacts/mock-server`. Materialize the returned `GeneratedTree` in your
build pipeline, then run the mock package with Docker Compose.

## Contract mocks

OpenAPI responses provide deterministic happy paths. Add `x-kaji-mock` to an
operation for named conditional cases—such as rate limits, validation failures,
or delayed responses. The generated fixtures are static YAML that
[`httpmock`](https://httpmock.rs/) can run in Docker, independently of the
language used by a generated SDK. See [docs/mocking.md](docs/mocking.md).

## Development

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Inspiration

Kaji's plugin-oriented workflow was initially inspired by Kubb. Kaji does not
include Kubb configuration compatibility, source code, test corpus, or runtime
dependencies.

## License

Kaji is licensed under the [MIT License](LICENSE).
