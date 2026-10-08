# CLI multi-package example

This recipe makes a Fetch SDK with Zod validation, a Go SDK, browsable API
documentation, and a Docker mock from one contract:

```sh
cd examples/cli-multi-package
npx poolster generate
```

```text
generated/
  typescript/     # Fetch SDK + validation/zod.ts
  go/             # Go SDK
  docs/           # ReDoc HTML + MCP tool manifest
  mock-server/    # Docker Compose HTTP mock
```

To run the generated contract mock:

```sh
cd generated/mock-server
docker compose up --build
```

This is the compact version of the full
[TypeScript stack](../typescript-stack/README.md).
