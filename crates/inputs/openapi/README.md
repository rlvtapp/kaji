# poolster-input-openapi

Owns the adapter for artifacts emitted by Poolster's existing embedded OpenAPI
compiler. CLI, Node and the facade use this package; parser behavior and generated
HTTP SDK behavior are preserved. It does not introduce another YAML/JSON parser.

`OpenApiSidecar::new(directory, name, version).load()` returns the existing
`AdaptedApi`. `publish(adapted, source_id)` publishes three typed contracts:

- `AdaptedApi`: authoritative API and reusable security definitions.
- `Blocks<Schema>`: rich model shapes, preserving constraints, defaults,
  nullability, field presence, composition and extensions.
- `Blocks<Operation>`: endpoints, parameters, media types, responses and security
  requirements. Reusable security definitions remain in `AdaptedApi`.

`OpenApiInput` registers as `openapi.compiler-artifacts` and accepts a compiler
artifact directory, not a raw schema filename. Its default name/version are
`Api`/`0.0.0`; use the explicit adapter constructor to retain caller-supplied
metadata. Existing CLI OpenAPI recipes still compile raw documents as before.

Block IDs use schema names and HTTP method/path coordinates and remain stable
under ordering changes. Blocks are optional projections; edits do not implicitly
rewrite the original contract. Unknown native locations are left unset rather
than fabricated. HTTP renderers still use the existing whole API path; block
consumers can be added independently.

Rust imports formerly under `poolster_core::adapter::OpenApiSidecar` and
`poolster_core::adapter::openapi_sidecar` move to this package. Core cannot
reexport the package without creating a dependency cycle.

For raw documents, register `OpenApiCompilerInput { executable, name, version }`
and select `openapi.compiler`. It compiles into a temporary artifact directory
and publishes all three contracts. The compiler executable remains supplied by
the caller; platform installation and remote downloading stay with the CLI.
`compiler::compile()` is also used by the CLI's existing direct generation path.
