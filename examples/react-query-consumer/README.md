# React Query consumer example

This is a small application-side companion to a generated Fetch SDK. It keeps
TanStack hooks in the generated package and shows the only integration code an
application needs: create the SDK transport and provide a Query Client.

```sh
cd examples/react-query-consumer
npx poolster generate
cd generated/sdk && npm install && npm run build && cd ../..
npm install
npm run typecheck
```

The example validates TypeScript wiring only. Mount `App` in your own Vite,
Next.js, or React application; framework lifecycle, SSR, caching, and error
policy remain app decisions.

The query plugin resolves actual operation imports from the SDK provider.
Moving its output keeps those bindings intact. `max_operations_per_file` bounds
helper modules while preserving the aggregate entrypoint. See the
[helper guide](../../docs/guides/typescript-helpers.md) for typed overrides,
option factories, cache scopes and cancellation.
