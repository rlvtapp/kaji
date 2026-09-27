# kaji

Optional unscoped command-line facade for `@relevate/kaji`:

```sh
npx @relevate/kaji generate ./openapi.yaml --output ./sdk --language go,typescript
```

This source package forwards to the scoped CLI without implementing generation.
The scoped package selects a prebuilt Rust executable and bundled Go OpenAPI
compiler. End users need Node, not Rust or Go toolchains.

Neither this source directory nor its version number implies npm availability.
Publish the platform packages and `@relevate/kaji` first. Publishing this optional
facade additionally requires npm to approve the unscoped `kaji` name; the scoped
CLI works independently of that approval.

Source and documentation: <https://github.com/rlvtapp/kaji>

## License

MIT
