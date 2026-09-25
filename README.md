# Kaji

Kaji is a Rust SDK-generation toolkit for producing polished, idiomatic SDKs
and contract mocks. It embeds its OpenAPI 3.0/3.1 compiler as Go source under
`openapi/`, keeps generation independent of JavaScript, and lets one API
contract produce packages for Rust, TypeScript, Go, Python, PHP, Java, .NET,
and Elixir.

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

## Start here

| Read | When you need it |
| --- | --- |
| [Getting started](docs/getting-started.md) | Compile an OpenAPI document and generate packages. |
| [OpenAPI compiler](docs/openapi-compiler.md) | The embedded Go compiler, artifacts, and standalone command. |
| [Configuration reference](docs/configuration.md) | Every target, package, TypeScript, and mock-server option. |
| [Generated SDKs](docs/generated-sdks.md) | Raw versus full SDK output, client shapes, and language requirements. |
| [Contract mocking](docs/mocking.md) | Run the Docker mock and add `x-kaji-mock` / pagination behavior. |
| [SDK verification](docs/verification.md) | Snapshot and live-contract CI coverage. |

## Workspace

| Path | Purpose |
| --- | --- |
| `openapi/` | Embedded Go OpenAPI compiler and its conformance tests |
| `crates/kaji-core` | AST, artifact adapter, generation engine, SDK and mock primitives |
| `crates/kaji` | First-party profile builder and standalone mock-server package |
| `crates/plugins/*` | Native SDK package generators by language |
| `docs/` | Architecture, contract-mocking, and SDK-verification notes |

## Quick start

First compile the source document into Kaji artifacts:

```sh
cd openapi
go run . --out ../.kaji/openapi ../openapi.yaml
```

Then generate the requested packages from Rust:

```rust
use anyhow::Result;
use kaji::{ProfileSet, generate_openapi};

fn main() -> Result<()> {
    let artifacts = generate_openapi(
        std::path::Path::new(".kaji/openapi"),
        "Example API",
        "1.0.0",
        ProfileSet::new("artifacts")
            .rust()
            .typescript_fetch()
            .python()
            .mock_server(),
    )?;
    artifacts.write_to("generated")?;
    Ok(())
}
```

The Rust call then writes isolated packages below `generated/artifacts`,
including `generated/artifacts/mock-server`.

## Development

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
# Runs the generated Go/Python SDKs against a local contract mock.
cargo test -p kaji --test sdk_to_mock_contract -- --ignored
```

Generated output is protected by an approved all-target snapshot, and the
first live SDK contract suite exercises the generated Go and Python clients.
See [docs/verification.md](docs/verification.md) for the coverage model.

## Inspiration

Kaji's plugin-oriented workflow was initially inspired by Kubb. Kaji does not
include Kubb configuration compatibility, source code, test corpus, or runtime
dependencies.

## License

Kaji is licensed under the [MIT License](LICENSE).
