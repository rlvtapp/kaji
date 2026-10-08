# CLI basic example

Generate one TypeScript Fetch SDK from a small local OpenAPI contract:

```sh
cd examples/cli-basic
npx poolster generate
cd generated/typescript
npm install
npm run build
```

`poolster.json` is the source-controlled recipe. It consumes `openapi.yaml` and
writes only to `generated/`. Try changing a schema or operation ID, regenerate,
and inspect the emitted package. Next, use the
[multi-package example](../cli-multi-package/README.md).
