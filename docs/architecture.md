# Architecture

Kaji has three layers:

1. **Core** holds the language-neutral API model, shared SDK semantics, artifact
   adapter, generated file tree, and typed plugin engine.
2. **Packages** select a language, output directory, package settings, and plugin
   instances. A release groups independently configured packages.
3. **Language plugins** render native files and own their own options. Rust and
   TypeScript live under `crates/plugins/`, just like the other languages.

The bundled Go compiler parses OpenAPI into local JSON artifacts. Rust loads
those artifacts into typed schemas, request bodies, responses, and security
requirements. The compiler is a build-time tool; generated SDKs do not run it.

`Package<L>` accepts `Plugin<L>` instances. Typed contracts connect providers
to consumers and determine generation order. Each language owns a mutable
workspace; immutable published contracts carry declared dependencies between
plugins. `Language::finalize` can merge package metadata after rendering.

TypeScript currently has symbol and dependency tracking in its workspace.
Other first-party SDK plugins use unit workspaces and render complete packages.
The engine supports community languages without adding a central language enum;
the CLI still has an explicit list of bundled targets.

Complete SDK plugins are not decomposed into interchangeable transport/model
providers yet. Auxiliary TypeScript artifacts are standalone renderers; their
imports and dependencies must be configured explicitly. See
[plugin authoring](typed-plugins.md) and
[auxiliary generators](auxiliary-generators.md).

The same typed API drives contract mocks. Shared semantics keep SDKs and test
fixtures aligned, without making the mock dependent on one SDK language.
