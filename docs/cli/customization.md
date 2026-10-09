# Customize a generated SDK or eject Poolster

← [CLI](README.md)

Choose where the change belongs:

| Change | Use |
| --- | --- |
| Add helpers or maintained tests | Source customization with `add` |
| Replace or patch generated files | Source customization with `replace` or `patch` |
| Add runtime behavior | Supported middleware and runtime configuration |
| Change the renderer itself | Eject source, edit it and rebuild Poolster |

## Customize one generated package

Keep maintained source outside the generated directory. In a package's JSON
recipe, use `customizations` to add, replace or patch generated source, or
`middleware` to bundle supported HTTP policies. Poolster applies those changes
during generation and tracks the resulting owned files.

[Customization examples](../reference/regeneration/sdk-customization.md) cover
all three overlay operations, exports, tests and bundled middleware. Avoid
editing an owned generated file directly: that can create a regeneration conflict.

If a recipe cannot express the extension, add a
[JavaScript plugin](../plugins/javascript/output.md) through a Node config or a
[Rust plugin](../plugins/rust/output.md) through an embedded generator. To change
the shipped renderer itself, use source ejection below.

## Eject the generator

Install a Poolster CLI version with ejection support, then:

```sh
poolster eject --language typescript --out ./my-poolster
```

The new directory contains the rebuildable generator workspace, selected editing
entry points in `EJECTED.md`, and source hashes in `EJECTED-SOURCES.json`. Ejection
writes source; it does not change the installed npm generator or generate a client.

Edit the source identified in `EJECTED.md`, install Rust and Go, then build and run
that customized CLI:

```sh
cd my-poolster
cargo build --locked -p poolster-cli
(cd openapi && go build -o ../target/debug/poolster-openapi .)
./target/debug/poolster generate --config /absolute/path/to/poolster.json
```

Use that rebuilt binary for subsequent generation. The installed npm package does
not automatically load your edited renderer. Keep the fork in version control,
run its plugin tests, and compile and execute the SDK it generates.

[Full ejection guide](../reference/regeneration/source-customization.md) covers
included sources, compiler resolution, updating your fork and the rebuild probe.

**Next:** [Safe regeneration](../reference/regeneration/safe-regeneration.md) · [Runtime middleware](../guides/runtime-middleware.md)
