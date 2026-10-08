# Poolster Swift plugin

Generates a Swift 5.9+ package from Poolster's normalized API model. The package
uses `Foundation`, `URLSession`, and `Codable` only: consumers keep control of
their dependency policy, HTTP session, headers, and observability hooks.

Use `swift::package("swift").with(swift::sdk())` in an embedded generation
profile, or set `"language": "swift"` with the `sdk` plugin in `poolster.json`.

Every operation is an `async throws` method. The namespaced client style adds
resource facades while retaining direct `PoolsterClient` methods, so switching the
style is additive for generated consumers.

Generated object models preserve unknown keys when additional properties are
allowed and track explicit null separately from omitted optional fields.
Closed models do not retain unknown keys. Extra properties use the declared
value type, or `JSONValue` for unconstrained schemas. `JSONValue` decodes signed
and unsigned 64-bit integers before floating-point values, preserving integer
values above JavaScript's safe integer range. Arbitrary-precision JSON numbers
are not supported.
