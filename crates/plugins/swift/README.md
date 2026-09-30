# Kaji Swift plugin

Generates a Swift 5.9+ package from Kaji's normalized API model. The package
uses `Foundation`, `URLSession`, and `Codable` only: consumers keep control of
their dependency policy, HTTP session, headers, and observability hooks.

Use `swift::package("swift").with(swift::sdk())` in an embedded generation
profile, or set `"language": "swift"` with the `sdk` plugin in `kaji.json`.

Every operation is an `async throws` method. The namespaced client style adds
resource facades while retaining direct `KajiClient` methods, so switching the
style is additive for generated consumers.
