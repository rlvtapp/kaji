# Why Kaji

An OpenAPI contract should be the source of truth for more than an SDK.

In a typical API, the client library, frontend hooks, validation schemas, mocks,
documentation, and AI tooling are built separately. They drift separately too:
an endpoint changes, the SDK gets updated, but the mock still accepts the old
shape, the docs are stale, and every consumer has a slightly different idea of
what the API does.

Kaji exists to make the contract the common input for that whole ecosystem.
One explicit, reviewable recipe can generate the artifacts that API consumers
actually need:

```text
OpenAPI
  │
  ▼
Kaji
  ├── typed SDKs        TypeScript · Rust · Go · Python · PHP · Java · .NET · Elixir
  ├── client surfaces   Fetch · Axios · direct operations
  ├── frontend helpers  TanStack React Query · Vue Query · SWR
  ├── schema helpers    Zod · Faker
  ├── testing           MSW · Cypress · HTTP mocks
  ├── documentation     ReDoc
  └── AI tooling        MCP
```

## Why not just an SDK generator?

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

The point is boring reliability: a contract change should have one obvious,
repeatable path to every artifact it affects.

## Our commitment

Kaji is MIT licensed and will remain free to use, including for commercial work.
We will not add a paid tier, feature-gate the generator, sell a commercial
license, or create an enterprise-only edition. The source, release history, and
generated output should remain useful without asking anyone to buy permission.

If you need Kaji to do something it does not yet do, open an issue, propose a
design, or contribute a plugin. The project should improve because its users
need better software—not because a feature can be put behind a sales call.

## Start here

- [CLI quickstart](cli/quickstart.md) for a first generated SDK.
- [`kaji.json` recipes](cli/config.md) for repeatable multi-package output.
- [Generated SDK guide](generated-sdks.md) for runtime expectations.
- [Examples](../examples/README.md) for working projects, mocks, frontend
  integrations, and embedded generation.
