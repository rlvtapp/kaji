# Postman collection generation plan

**Design history.** For current APIs and support, use the [postman guide](postman.md). Statements about the prototype below describe the original proposal.

<details>
<summary>Explore the original design and follow-up ideas</summary>

Status: initial portable exporter implemented. See [the user guide](postman.md)
and [combined example](../examples/api-artifacts/README.md) for the supported
API and verification. This document retains the broader roadmap; unsupported
serializers, scenario execution, remote workspace sync and release asset delivery
remain follow-up work.

## Scope and architecture

Add an independent `poolster-plugin-postman` consumer of Poolster's normalized `Api`,
security catalog, request/response media types, parameter serialization metadata,
and bounded request samples.
Produce portable Collection 2.1 JSON, an optional
empty-credential environment template, response examples, and diagnostics.
Keep
collection execution and remote workspace publication separate and opt-in.

Speakeasy exports collections from OpenAPI; Poolster can fit that capability into its
existing plugin graph rather than requiring its SDK or Terraform models.
[Speakeasy's Postman generator](https://www.speakeasy.com/blog/release-postman-generator).

```text
OpenAPI -> sidecar -> neutral Poolster API
                         |-- SDK providers
                         |-- Terraform semantic providers -> provider/state
                         `-- request examples -> Postman collection/environment
```

Use typed contracts `RequestExamples`, `CollectionDocument` and
`EnvironmentTemplate`.
A custom example provider or collection renderer can
replace the defaults.
Reuse `core::samples` for bounded representative values,
but do not treat its samples as complete schema validation.
Examples may need
explicit fixtures for recursive/overlapping unions and unsupported constraints.

Collection item IDs and ordering must be stable across regeneration; key
requests by operation identity and detect collisions rather than depending on
list position or random UUIDs.
File ownership/check mode apply to generated JSON
and diagnostic files just as they do to SDKs.

## Request and response mapping

- Group by tags or path; operation ID/title remains traceable to the source.
- Preserve method, documentation, deprecation, request headers and path/query
  parameters. Required values get examples/placeholders; optional parameters are
  disabled initially rather than silently sent as meaningful defaults.
- Resolve root/path/operation server precedence and server variables before
  rendering a configurable `{{base_url}}`. Missing or conflicting server
  metadata produces a diagnostic.
- Render raw JSON, URL-encoded form, multipart fields/file placeholders and
  binary file placeholders separately. Preserve content type and compatible
  example choices. Unsupported encodings fail or mark the request incomplete;
  do not silently substitute JSON.
- Respect OpenAPI style/explode/allowReserved and encoded query values. Simple
  cases use Collection structured URL/query forms; ambiguous nested/deep-object
  or matrix encodings use a tested explicit serializer or a diagnostic. Avoid
  double encoding placeholders and encoded path segments.
- Save declared response examples by status/content type, including error
  examples. Streaming/binary responses get descriptions or safe placeholders;
  do not claim ordinary Collection execution proves SSE behavior.

Collections follow the official [Collection 2.1 schema](https://schema.postman.com/json/collection/v2.1.0/docs/index.html).
Validate actual generated files against the pinned schema, not just snapshots.

## Authentication and variables

Use empty secret placeholders, never values from the generator process's
credentials.
Bearer/basic/API-key schemes map to Postman authentication or
explicit headers/query/cookies as appropriate.
Resolve each operation's OR
alternatives and AND scheme combinations; one inherited collection auth setting
cannot represent every combination.
Make alternative selection explicit when
necessary.

Per-operation public security overrides suppress inherited auth.
OAuth flows/scopes map only when supported by Postman; otherwise export token
placeholders and instructions, not an invented login script.

Keep environments separate from collection requests. Credential values stay
blank, and write-only/sensitive example fields are scrubbed into placeholders.
Retain user-owned environment values via create-once files or a custom consumer;
regeneration must not fetch them from Postman and re-export them into source.
Postman documents [variable scopes](https://learning.postman.com/docs/use/send-requests/variables/variables/)
and [developer secret handling](https://learning.postman.com/docs/administration/security/developer-security).

## Historical API sketches

The current implementation uses the native builders and `language: postman`
recipe documented in [the guide](postman.md). The artifact-package sketch below
is a design illustration, not accepted configuration.

```rust
// Native plugin composition API.
let examples = postman::examples();
let collection = postman::collection().using_examples(examples.handle());
let package = postman::package("postman")
    .with(examples)
    .with(collection)
    .with(postman::environment());
```

```json
{
  "language": "artifacts",
  "path": "postman",
  "plugins": [{
    "name": "postman",
    "collection": "api.postman_collection.json",
    "environment": "api.postman_environment.json",
    "group_by": "tag",
    "examples": "declared_then_bounded",
    "auth_alternative": "configured"
  }]
}
```

Keep options small: output names, grouping, selected server/auth alternative,
example provider/budget, and optional generated status assertions. Use native
plugin handles for deeper customization. Do not embed arbitrary JavaScript from
OpenAPI by default. Extensions/configured source files can reference preserved
user-owned scripts through an explicit opt-in hook consumer.

## CI and distribution

A collection validation action should check the schema, placeholders, stable
IDs and generated drift.
Execution with Newman/Postman CLI is a separate
workflow against an explicitly configured sandbox/mock API.
It must not execute
create/delete requests against a real service merely because a collection was
generated.

Run ordered multi-operation scenarios only from explicit fixtures;
reuse Poolster's mock/scenario layer where possible.
Export files first; remote
Postman API synchronization would need separately authorized workspace access
and credential handling.

A Postman ZIP/JSON GitHub Release asset can be delivered alongside SDK releases
through an optional artifact publisher. This is a custom plugin capability,
not a package registry enum added to core. The editable check/publish framework
can accept those commands once the collection plugin exists.

## Implementation sequence and files

1. Audit metadata preservation in `openapi/{types,openapi,examples,schema_walk}.go`
   and `core::adapter::openapi_sidecar`; add focused round-trip tests for servers,
   examples, request parameter extensions and response headers. Add only generic
   AST fields where missing.
2. Create `crates/plugins/postman` with `package.rs`, `contracts.rs`,
   `requests.rs`, `auth.rs`, `examples.rs`, `collection.rs`, `environment.rs` and
   `diagnostics.rs`. Unit-test bindings before rendering files.
3. Add a versioned optional recipe plugin through `crates/kaji-cli/src/main.rs`,
   facade reexports/dependency in `crates/kaji`, workspace membership and
   `schemas/v1/poolster.schema.json`. Keep standalone consumers compatible with
   existing artifact packages.
4. Validate generated Collection JSON and mock-server execution fixtures for
   scalar/list/object parameters, media variants, security combinations,
   response examples and namespacing. Check redaction, deterministic ownership,
   create-once environments and safe regeneration.
5. Add editable optional collection CI/release hooks and a runnable example.
   Consider remote workspace sync only after the portable exporter is stable.

Terraform entities/state/upgrades remain in the Terraform plugin. Shared gains
are source metadata, parameter/request binding, security and fixture generation;
neither plugin should import the other's target-specific plan.


</details>
