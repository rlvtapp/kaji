# Rust embedded example

This standalone application compiles `openapi.yaml`, composes TypeScript and Go
packages through the Rust API, and writes them to `generated/`.

```sh
cd openapi
go run . --out ../examples/rust-embedded/.poolster/openapi ../examples/rust-embedded/openapi.yaml
cd ../examples/rust-embedded
cargo run
```

The example uses path dependencies because the crates are currently consumed
from this checkout. It lives outside the workspace to resemble a consumer
application. Read the [library quickstart](../../docs/rust/quickstart.md)
before adapting it to your own build process.
