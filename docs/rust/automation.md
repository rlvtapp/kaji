# CI, SDK delivery and ejection

← [Rust SDK](README.md)

## Run an embedded generator in CI

Your Rust application owns provider registration and package composition. Commit
its manifests and lockfile, install the required input toolchain, and run it with
`cargo run --locked`. For the OpenAPI quickstart, supply `POOLSTER_OPENAPI_BIN`.
Other inputs may require their official compiler or a local protocol server.

Check the generated tree before writing using the [file APIs](files.md). Compile
the output package with its target toolchain as a separate check. Building the
Rust generator alone does not compile the generated SDK.

## Open SDK PRs and publish releases

The CLI provides delivery orchestration for packages carrying `.poolster/package.json`
metadata. Embedded generators can add native `release::metadata::<Language>(...)`
plugins to publish that metadata; the CLI JSON recipe has a `release` section for
the same purpose.

Follow [SDK automation](../reference/automation/sdk-automation.md) for package
commands, workflow scaffolding, destination setup and GitHub authentication.
The [publishing guide](../reference/automation/sdk-publishing.md) covers npm,
PyPI, crates.io, Go and custom publisher commands. These delivery steps are
separate from a library call that returns a generated tree.

## Customize the generator

An embedded Rust application can depend on its own plugin crates or a renderer
fork directly. That is often sufficient when you already maintain the generator
application. Use [Rust plugin authoring](../plugins/rust/README.md) to extend it.

To customize the shipped CLI instead, run:

```sh
poolster eject --language rust --out ./my-poolster
```

Ejection exports a rebuildable source workspace. Follow
[the ejection guide](../reference/regeneration/source-customization.md) to edit,
rebuild and run it. It does not change an existing library dependency or load a
new plugin into an installed prebuilt CLI.

**Next:** [Package composition](packages.md) · [File ownership](files.md)
