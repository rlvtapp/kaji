# Inspect a generation plan

← [CLI](README.md)

See declared plugins and contract connections before running generation:

```sh
poolster plan --config poolster.json
```

The default is a terminal overview. Each package shows Generate and Post plugins,
contract publications, selected provider connections, lifecycle stages and
planning diagnostics. IDs are local to the package and plan; they are not native
contract entity IDs or runtime revision identifiers.

## Export for a custom viewer

```sh
poolster plan --config poolster.json --format json --output plan.json
```

The JSON envelope has `version: 1` and a `packages` array. Each package contains
`plugins`, `edges`, `stages`, `status` and an optional `diagnostic`.
An edge's `provider` and `consumer` refer to plugin IDs within that package;
`provider: null` means no selected provider. An optional absent requirement is
valid. Invalid graphs retain their declarations and diagnostic for inspection.

The JSON format is a planning envelope, separate from contract value codecs.
Contract names identify declarations. Actual input revisions, block contents,
conditional publications, generated files and timings require execution.

## Optional HTML renderer

```sh
poolster plan --config poolster.json --format html --output plan.html
```

Open the standalone local file to see Generate, contract and Post lanes. Select
a node for its declarations. It uses no CDN or external scripts. You can build
your own viewer from the JSON instead.

## Scope

The CLI uses the same Rust package resolver as generation for typed dependencies.
It reads and validates the JSON recipe and configured customization sources,
but does not load protocol inputs or execute generator callbacks. A ready plan
means the declared graph resolves; input validation and generated-code checks
still happen during generation.

Legacy HTTP generators can read the HTTP context without a typed edge. CLI
artifact additions and delivery metadata inserted outside the package graph are
not shown as independently resolved plugin nodes. Skipped native packages are
reported with their incompatibility reason. Custom plugin internals remain opaque
unless they expose planning metadata.

A malformed recipe may fail before a graph can be built. A JSON plan is an
inspection result, not a successful-generation report or execution trace.

**SDKs:** [JavaScript planning](../javascript/planning.md) · [Rust planning](../rust/planning.md)
