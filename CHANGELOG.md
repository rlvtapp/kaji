# Changelog

## Unreleased

## 0.3.0 — 2026-09-30

### Added

- Added first-class Ruby and Swift SDK generators. Ruby output is a Ruby 3.1+
  gem using `Net::HTTP`, `URI`, and `JSON`; Swift output is a Swift 5.9+ Swift
  Package Manager library using `URLSession` and `Codable`.
- Added generated TypeScript and native Rust API CLI targets. They derive nested
  commands, request flags, help text, authentication, OAuth flows, API-key
  profiles, and environment-variable support from OpenAPI.
- Added a no-Docker native mock server: `kaji mock serve`. It serves
  schema-shaped dynamic responses, evaluates `x-kaji-mock` scenarios, and
  exposes health and request-log endpoints for people and agents.
- Added `kaji check` contract diagnostics, JSON output, severity controls, and
  reviewable baselines for incrementally improving existing specifications.
- Added path slicing with repeatable `--include-path` and `--exclude-path`
  selectors for direct generation and `kaji.json` recipes.
- Added reproducible generation locks that record secret-free inputs, generator
  settings, selected operations, and artifact hashes.
- Added `kaji show` for inspecting the exact operation slice selected from a
  local contract, including JSON output for automation and agents.
- Added `kaji update` to replay direct-generation locks when their local source
  or compiler-artifact input changes, with a `--force` override.
- Added `kaji auth login`, `logout`, and `status` for named, environment-backed
  credential profiles in authenticated remote OpenAPI inputs. Profiles retain
  only the environment-variable name, never a token value.
- Added `kaji discover` and safe `kaji download` support for the APIs.guru
  directory, with relevance-ranked and JSON output for automation.
- Added `kaji mcp generator`, exposing local generation controls to MCP clients.
- Added runnable examples for API CLIs, native mocks, reproducible generation,
  agentic generation, Ruby, and Swift.

### Changed

- Made `kaji-plugin-csharp` and `csharp` the canonical C#/.NET generator
  surface. The `dotnet` crate and selector remain compatibility aliases.
- Extended CLI configuration, JSON Schema, MCP discovery, examples, and
  documentation for C#, Ruby, and Swift targets.
- Added authenticated remote OpenAPI inputs with custom headers, Basic, Bearer,
  and environment/profile-backed secret values.
- Improved generated Go retry behavior and TypeScript generated-client
  ergonomics.
- Added CI coverage for the canonical C# target and generated Rust SDK builds
  on fresh runners.

- Established the standalone Kaji Rust workspace.
- Added first-party SDK targets and language-neutral contract mocks.
