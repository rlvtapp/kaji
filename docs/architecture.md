# Architecture

Kaji has three deliberately separate layers:

1. **Core** normalizes an API into a stable Rust AST and owns generation
   semantics such as auth, pagination, declared errors, and streaming.
2. **Profiles** select publishable packages and give each output a stable
   directory and public client shape.
3. **Language plugins** render idiomatic package layouts without changing the
   core contract.

The same AST also feeds contract mocks. This keeps documentation, SDKs, and
test environments aligned without coupling mock behaviour to a specific SDK
language.

The experimental typed engine composes `Package<L>` values containing
`Plugin<L>` instances. Contracts determine generation order and bind each
consumer to a provider; language workspaces collect shared package state before
finalization. Rust and TypeScript generators, including their configuration,
live in plugin crates alongside the other maintained languages. The facade
retains the old profile shortcuts as compatibility adapters.

See [typed plugin packages](typed-plugins.md) for the API and the boundaries of
this first migration stage.
