# Changelog

## Unreleased

## 0.3.0 — 2026-09-30

### Added

- Added first-class Ruby and Swift SDK generators. Ruby output is a Ruby 3.1+
  gem using the standard-library HTTP stack; Swift output is a Swift 5.9+
  Swift Package Manager library using `URLSession` and `Codable`.
- Added `kaji show` for inspecting the exact operation slice selected from a
  local contract, including JSON output for automation and agents.
- Added `kaji update` to replay direct-generation locks when their local source
  or compiler-artifact input changes, with a `--force` override.
- Added `kaji auth login`, `logout`, and `status` for named, environment-backed
  credential profiles in authenticated remote OpenAPI inputs. Profiles retain
  only the environment-variable name, never a token value.
- Added reproducible generation workflows: secret-free generation locks,
  path slicing, API discovery/download, an agent-friendly MCP generator
  interface, and runnable examples for the new workflows and targets.

### Changed

- Made `kaji-plugin-csharp` and `csharp` the canonical C#/.NET generator
  surface. The `dotnet` crate and selector remain compatibility aliases.
- Extended CLI configuration, JSON Schema, MCP discovery, examples, and
  documentation for C#, Ruby, and Swift targets.
- Expanded native contract-mock workflows and generated API CLI support,
  including OAuth, API-key profiles, nested commands, and dynamic scenarios.
- Added CI coverage for the canonical C# target and generated Rust SDK builds
  on fresh runners.

- Established the standalone Kaji Rust workspace.
- Added first-party SDK targets and language-neutral contract mocks.
