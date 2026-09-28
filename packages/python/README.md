# kaji-cli

`kaji-cli` installs Kaji's native OpenAPI SDK generator through pip. It exposes
the same `kaji` command as the npm package, and includes the Rust generator plus
its adjacent Go OpenAPI compiler. Installing it does not require Node.js, Rust,
or Go and never downloads an executable at runtime.

```sh
python -m pip install kaji-cli
kaji init --input ./openapi.yaml --output ./generated
kaji generate
```

The package currently publishes wheels for macOS ARM64 and Intel, Linux x64 with
glibc 2.35 or later, and Windows x64. Alpine/musl and Linux ARM64 are not yet
packaged. See the [CLI guide](https://github.com/rlvtapp/kaji/blob/main/docs/cli.md)
for recipes, supported generation targets, and source-build instructions.
