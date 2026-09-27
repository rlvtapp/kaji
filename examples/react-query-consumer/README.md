# React Query consumer example

This is a small application-side companion to a generated Fetch SDK. It keeps
TanStack hooks in the generated package and shows the only integration code an
application needs: create the SDK transport and provide a Query Client.

```sh
cd examples/react-query-consumer
npx @relevate/kaji generate
cd generated/sdk && npm install && npm run build && cd ../..
npm install
npm run typecheck
```

The example validates TypeScript wiring only. Mount `App` in your own Vite,
Next.js, or React application; framework lifecycle, SSR, caching, and error
policy remain app decisions.

The generated hook imports match this recipe because `clients_import` points
from `react-query/` back to the generated `clients/` directory. If you change
the output path or `group_by_tag`, update both together.
