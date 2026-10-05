# Native Rust API CLI

From the repository root:

```sh
cargo run -p kaji-cli -- generate --config examples/rust-cli/kaji.json
cargo build --manifest-path examples/rust-cli/generated/notes/Cargo.toml
examples/rust-cli/generated/notes/target/debug/notes --help
```

Inspect the generated README for the exact resource commands and auth options.
The fixture exposes GET /notes/{noteId}; provide a server implementing that
contract before making requests. In a terminal, omitted required inputs and
base URL prompt interactively. For agents and scripts, provide all values and
use --json; pipes never prompt. Credential prompts are masked.

OpenAPI-derived references are generated under generated/notes/references/.
