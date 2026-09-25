# OpenAPI compiler

Kaji owns its OpenAPI compiler under `openapi/`. It is Go source checked into
this repository, built with the Go toolchain, and does not call Relevate Docs,
an external sidecar service, or Node.

## Run

```sh
cd openapi
go run . --out ../.kaji/openapi ../openapi.yaml
```

The final argument can be an OpenAPI 3.0/3.1 JSON or YAML file. `--out` is the
artifact directory consumed by `kaji::generate_openapi`.

```sh
go test ./...
go build -o ../bin/kaji-openapi .
../bin/kaji-openapi --out ../.kaji/openapi ../openapi.json
```

## Artifacts

The compiler emits stable JSON artifacts rather than language-specific SDK
code:

```text
.kaji/openapi/
  operations.json
  operations-order.json
  operations/
  schemas.json
  security-schemes.json
```

Kaji's Rust generators load these files into a target-neutral AST. That keeps
OpenAPI complexity in one proven parser while every SDK generator remains pure
Rust.

The compiler test suite lives next to its source in `openapi/*_test.go`.
