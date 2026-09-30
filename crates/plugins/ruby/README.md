# Kaji Ruby plugin

Generates a Ruby 3.1+ gem from Kaji's normalized API model. The resulting gem
uses Ruby's standard `net/http`, `uri`, and `json` libraries, so application
owners keep control of their HTTP stack and dependency policy.

Use `ruby::package("ruby").with(ruby::sdk())` in an embedded profile, or set
`"language": "ruby"` with the `sdk` plugin in `kaji.json`.
