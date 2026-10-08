# Postman plugin

Generate a portable Postman Collection 2.1 from Poolster's normalized API. Generation
does not execute requests, import specification scripts, or write to Postman.

```rust
use poolster_plugin_postman::{self as postman, PackageExt};
let fixtures = postman::examples();
let collection = postman::collection()
    .using_examples(fixtures.handle())
    .strict(true);
let environment = postman::environment().using_collection(collection.handle());
let package = postman::package("postman")
    .name("Example API")
    .with(fixtures)
    .with(collection)
    .with(environment);
```

The example provider is optional. A custom `Plugin<Postman>` can publish
`RequestExamples`; collection generation still scrubs its untrusted values.
`CollectionDocument` and `EnvironmentTemplate` are independently consumable typed
contracts. Explicit handles disambiguate multiple providers.

Collection output and `diagnostics.json` are owned generated files. The blank
environment is create-once and preserves local edits. Native `.strict(false)` is
the default; CLI recipes default to strict generation. `.strict(true)` rejects
error-level mappings. Missing origins, multiple server choices, OAuth token
acquisition and stream placeholders generate review diagnostics. Unsupported
parameter styles, nested form encodings, unknown security schemes and ambiguous
headers generate errors. OAuth/OpenID placeholders require an independently
acquired bearer token. No login/refresh scripts are generated.

Use `.split_by_group(true)` to also emit standalone `collections/group-0000.json`
exports per folder, retaining aggregate request IDs and shared blank variables.
The aggregate collection contract remains available to dependent plugins.

Requests group by their first tag; `.group_by_tag(false)` groups by their first
path segment. Alternative request media and OR security requirements produce
separate deterministic request IDs. AND security requirements combine compatible
helpers/API-key locations. Optional parameters and optional bodies start disabled.
JSON, URL-encoded, multipart/file and opaque binary bodies use distinct modes.
Declared samples precede bounded schema samples. Sensitive credential names,
`writeOnly` and `x-sensitive` schemas are scrubbed; credentials start blank.

Run ordinary behavior tests with `cargo test -p poolster-plugin-postman`. The ignored
`validates_actual_official_draft04_schema` test validates generated representations
using Python `jsonschema`; set `POOLSTER_TEST_PYTHON` and `PYTHONPATH` as needed:

```sh
cargo test -p poolster-plugin-postman validates_actual_official_draft04_schema -- --ignored
```

The unmodified pinned schema in `tests/schema/collection-v2.1.0.json` comes from
[Postman's official Collection 2.1 schema](https://schema.postman.com/json/collection/v2.1.0/collection.json).
Schema validation verifies export shape; live API behavior requires a deliberate
execution test against a sandbox or mock.

See the [author guide](../../../docs/postman.md) for CLI recipes and distribution.
