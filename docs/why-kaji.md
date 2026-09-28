# Why Relevate built Kaji

At Relevate, we made Kaji to generate the Relevate Email SDKs. We needed the
Relevate Email OpenAPI contract to drive more than a client library: client
interfaces, frontend hooks, validation schemas, mocks, documentation, and tools
used by AI agents, without maintaining each one as a separate, hand-written
integration.

Kaji exists to make that possible: one OpenAPI contract, multiple outputs.

It is built first for Relevate Email's own SDK and integration workflow. An API
change should have one explicit, reviewable path to every developer-facing
artifact it affects, rather than becoming a string of manual updates that drift
apart over time.

```text
OpenAPI
  │
  ▼
Kaji
  ├── SDKs      TypeScript · Go · Python · Rust · Java · .NET · PHP · Elixir
  ├── Clients   Fetch · Axios
  ├── Frontend  TanStack React Query · Vue Query · SWR
  ├── Schema    Zod · Faker
  ├── Testing   MSW · Cypress · HTTP mocks
  ├── Docs      ReDoc
  └── AI        MCP
```

## Built for our own use, released for yours

Kaji is not a generic product idea looking for an enterprise tier. We maintain
it because Relevate Email uses it. Publishing it as open source means other teams
can use it too, inspect how it works, and help make it better.

## What Kaji is not

- not a hosted code-generation platform
- not a paid SDK generator with a free teaser tier
- not a commercial-license funnel
- not an enterprise-only product with the useful features held back

## More than an SDK generator

An SDK generator solves only one of the contract's downstream problems. Kaji
keeps the generated pieces together without forcing every consumer into the same
runtime or workflow:

- Generate one language or several, from one command or a committed `kaji.json` recipe.
- Choose client surfaces deliberately: namespaced or flat clients, Fetch or Axios,
  or TypeScript operations without a client class.
- Keep generated SDKs, fixtures, mocks, docs, and helper code traceable to the
  same contract and easy to reproduce in CI.
- Use the native CLI in a repository or CI job, or embed the Rust library and
  compose typed plugins when generation belongs inside an application.

The point is boring reliability: a Relevate Email contract change should have
one obvious, repeatable path to every artifact it affects.

## Our commitment

Kaji is MIT licensed and will remain free to use, including for commercial work.
We will not add a paid tier, feature-gate the generator, sell a commercial
license, or create an enterprise-only edition. The source, release history, and
generated output should remain useful without asking anyone to buy permission.

If you need Kaji to do something it does not yet do, open an issue, propose a
design, or contribute a plugin. The project should improve because its users
need better software, not because a feature can be put behind a sales call.

## Start here

- [CLI quickstart](cli/quickstart.md) for a first generated SDK.
- [`kaji.json` recipes](cli/config.md) for repeatable multi-package output.
- [Generated SDK guide](generated-sdks.md) for runtime expectations.
- [Examples](../examples/README.md) for working projects, mocks, frontend
  integrations, and embedded generation.
