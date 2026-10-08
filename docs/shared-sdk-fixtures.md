# Shared SDK behavior plans and wire fixtures

Language plugins can use `kaji_core::pagination::normalize_pagination` instead of independently parsing pagination extensions. It returns a typed cursor, offset/limit, page, or URL plan, or an error identifying invalid declarations. An explicit `PaginationRule` takes precedence over `x-kaji-pagination`, which takes precedence over `x-speakeasy-pagination`.

Rules retain the established extension shape:

```json
{
  "type": "cursor",
  "inputs": [{ "name": "cursor", "in": "parameters", "type": "cursor" }],
  "outputs": { "nextCursor": "$.next", "results": "$.items" }
}
```

Inputs refer to actual parameters or fields in a declared JSON request body.
Cursor inputs are strings or integers; page, offset, and limit inputs are integers.
Cursor and page declarations may include an optional limit input.
URL plans require no continuation parameter and explicitly require origin restrictions.
Offset and page plans require a results selector.

Schema references and declared JSON response selectors are validated when response schemas exist.
Missing response schemas still permit a syntax-validated plan.

Selectors support dotted fields, signed array indices, and JSON pointers: `$.pages[-1].next`, `$.items`, and `/links/next`. Filters and wildcard expressions are rejected. The shared `Selector::select` implementation can also be used by test consumers. Native language runtimes must implement the same selector semantics before claiming support for every shared-plan capability.

Existing language paginator renderers remain available; exposing a shared plan does not automatically migrate those renderers or add page pagination to their clients.

## Optional serialization fixture consumers

A test plugin can consume model-provider symbols and emit deterministic inputs with `kaji_core::samples::schema_samples`:

```rust
use kaji_core::samples::{SampleOptions, schema_samples};

let report = schema_samples(api, &schema.value, SampleOptions::default());
for sample in report.samples {
    // Emit language-native decode/encode assertions using provider symbols.
    // Compare normalized JSON values rather than serialized object key order.
    let fixture_json = serde_json::to_string(&sample.value)?;
    emit_fixture(&schema.name, &sample.name, &fixture_json)?;
}
for diagnostic in report.diagnostics {
    report_fixture_gap(&schema.name, diagnostic);
}
```

The generator covers required-only objects, populated optional fields, explicit nullable values, enum/union alternatives, additional properties, arrays, format examples, and object intersections. It caps depth, fixture count, and array length. Recursive optional fields can be omitted; impossible required cycles produce diagnostics instead of invalid null placeholders. Defaults are not silently substituted for other alternatives.

These fixtures exercise wire structure and round trips, not complete JSON Schema conformance. Unsupported constraints such as regular expressions and `not`, unresolved external references, incompatible intersections, or exceeded budgets need user-supplied fixtures. OneOf overlap is not validated. Enumerating a bounded set of fixtures does not cover every combination of nested union and enum values.

A consumer should publish which fixtures it executes and surface missing fixtures. Snapshot comparisons alone do not establish decoder/encoder behavior. Unknown enum/union response handling requires separate intentionally unknown fixtures in addition to schema-derived declared values.
