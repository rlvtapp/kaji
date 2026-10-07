# OpenAPI compiler

Kaji owns its OpenAPI compiler under `openapi/`. It is Go source checked into
this repository, built with the Go toolchain, and does not call Relevate Docs,
an external sidecar service, or Node.

## Run

```sh
cd openapi
go run . --out ../.kaji/openapi ../openapi.yaml
```

The final argument can be a Swagger 2.0 or OpenAPI 3.0/3.1 JSON or YAML file. `--out` is the
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

The artifact directory is an internal, versioned-together compiler/generator
boundary, not a stable interchange format. Regenerate it with this repository's
compiler when updating Kaji. `schemas.json` and `security-schemes.json` are
required even when their catalogs are empty. Operation `security_requirements` preserve the full
declared alternatives; request bodies carry their representations under
`media_types`, including media-specific schema and example data. Rust generators
consume typed `request_body` and `responses`, rather than guessed type-name
strings or a first-scheme authentication summary.

The compiler test suite lives next to its source in `openapi/*_test.go`.

## Large and recursive documents

Schema example/field previews use path-local cycle detection, a maximum depth
of 32, and a 256-node traversal budget. This limits only synthesized documentation
previews: original schema definitions and references remain in the artifacts.
Independent uses of the same schema are not mistaken for a cycle. Truncated
examples use explicit placeholder text and are not validation fixtures.

Long operation filenames are bounded and receive a deterministic hash suffix;
paths that normalize to the same filename are disambiguated rather than
overwriting another operation. See [large-spec verification](large-specs.md).

OpenAPI 3.2 ordinary contracts, `QUERY` operations, nullable request bodies and
webhook-only descriptions are accepted. SDK methods preserve QUERY on the wire,
including its request body. `HEAD`, `OPTIONS` and `TRACE` are also retained.
Custom `additionalOperations`, `querystring` parameters, streaming `itemSchema`
and multipart `prefixEncoding`/`itemEncoding` currently fail with a capability
diagnostic before compiler artifacts are modified. Future versions also fail.
This is bounded 3.2 support; changing a version string cannot remove unsupported
semantics. See the [OpenAPI 3.2 specification](https://spec.openapis.org/oas/v3.2.0.html).

Local referenced files contribute to the compiler cache digest and `source.json`
manifest, so editing a child document invalidates the cache. Generation provenance
records this closure digest. Remote `$ref` URLs are rejected: bundle them explicitly
rather than forwarding download credentials to referenced hosts.
