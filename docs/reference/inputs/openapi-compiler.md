# OpenAPI compiler

[Input plugins](input-plugins.md) · [Architecture](../../internals/architecture.md) · [OpenAPI 3.2](../../guides/openapi32.md)

Poolster owns its OpenAPI compiler under `openapi/`. It is Go source checked into
this repository, built with the Go toolchain, and does not call Relevate Docs,
an external sidecar service, or Node.

## Run

```sh
cd openapi
go run . --out ../.poolster/openapi ../openapi.yaml
```

The final argument can be a Swagger 2.0 or OpenAPI 3.0/3.1/3.2 JSON or YAML file. `--out` is the
artifact directory consumed by `poolster::generate_openapi`.

```sh
go test ./...
go build -o ../bin/poolster-openapi .
../bin/poolster-openapi --out ../.poolster/openapi ../openapi.json
```

## Artifacts

The compiler emits stable JSON artifacts rather than language-specific SDK
code:

```text
.poolster/openapi/
  operations.json
  operations-order.json
  operations/
  schemas.json
  security-schemes.json
  api-metadata.json
```

Poolster's Rust generators load these files into a target-neutral AST. That keeps
OpenAPI complexity in one proven parser while every SDK generator remains pure
Rust.

The artifact directory is an internal, versioned-together compiler/generator
boundary, not a stable interchange format.
Regenerate it with this repository's
compiler when updating Poolster.
`schemas.json` and `security-schemes.json` are
required even when their catalogs are empty.
Operation `security_requirements` preserve the full
declared alternatives; request bodies carry their representations under
`media_types`, including media-specific schema and example data.

Rust generators
consume typed `request_body` and `responses`, rather than guessed type-name
strings or a first-scheme authentication summary.

The compiler test suite lives next to its source in `openapi/*_test.go`.
See [the OpenAPI 3.2 SDK guide](../../guides/openapi32.md) for whole-query inputs,
record-oriented JSON and ordered multipart builders.

## Large and recursive documents

Schema example/field previews use path-local cycle detection, a maximum depth
of 32, and a 256-node traversal budget. This limits only synthesized documentation
previews: original schema definitions and references remain in the artifacts.
Independent uses of the same schema are not mistaken for a cycle. Truncated
examples use explicit placeholder text and are not validation fixtures.

Long operation filenames are bounded and receive a deterministic hash suffix;
paths that normalize to the same filename are disambiguated rather than
overwriting another operation. See [large-spec verification](../../verification/large-specs.md).

OpenAPI 3.2 ordinary contracts, `QUERY` operations, nullable request bodies and
webhook-only descriptions are accepted. SDK methods preserve QUERY on the wire,
including its request body. `HEAD`, `OPTIONS` and `TRACE` are also retained.
Custom `additionalOperations` are retained as case-sensitive HTTP tokens (1–256
ASCII token bytes), with standard-method shadowing rejected. Custom methods default
to unsafe retry semantics and retain native transport restrictions.

The compiler retains whole-query `querystring` parameters and their `content`,
sequential media `itemSchema`, positional multipart `prefixEncoding`/`itemEncoding`,
recursive Encoding Objects, device authorization URLs, tag hierarchy and `$self`.
Reusable `components.mediaTypes` references and XML `nodeType` metadata are retained.
The Rust boundary exposes typed content metadata in `poolster_core::openapi32`; sequential
item schemas become array model schemas for buffered request/response APIs.

The bundled libopenapi parser advertises 3.2 support. Poolster also reads resolved source
nodes for positional encodings and the official `deviceAuthorization` flow where
version 0.38.7's high-level models omit or misname fields. Compiler and native runtime
tests cover these paths rather than inferring support from a version number.

Sequential JSON is buffered and decoded record by record.
Multipart requests use
native or explicit ordered builders; custom media formats and multipart response
interpretation still require a native codec or application decoding.
Preserving an
XML Schema annotation does not install an XML codec.
Poolster is not a complete JSON
Schema validator.

Future OpenAPI minor versions and malformed conflicting encodings
fail before artifacts are modified.
See the
[OpenAPI 3.2 specification](https://spec.openapis.org/oas/v3.2.1.html).

Local referenced files contribute to the compiler cache digest and `source.json`
manifest, so editing a child document invalidates the cache.
Generation provenance
records this closure digest.
Remote `$ref` documents are supported over public HTTPS, including relative remote
references.
All fetched bytes contribute to the closure digest.

Fetching forwards
no credentials, uses no proxy, follows no redirects, and pins validated public DNS
addresses before dialing.
Requests have a 30-second timeout; each document is
limited to 16 MiB and the closure to 256 MiB / 16,384 documents.
Private-network,
HTTP and authenticated reference servers require explicit local bundling.

For a root document downloaded by the CLI, `--source-url` communicates its original
HTTP(S) origin to the compiler using the already-downloaded bytes. Relative remote
references resolve from that URL; root authentication is not forwarded to children.
External referenced documents still require public HTTPS. Standalone compiler
users can pass `--source-url` with a downloaded local file themselves.

An OpenAPI document's `$self` establishes its reference base and identity. The compiler
resolves relative references against that URI, recognizes references back to the
already-loaded document, and rejects duplicate identities, credentials and fragments
in `$self`. Source closure hashing counts downloaded documents once.
