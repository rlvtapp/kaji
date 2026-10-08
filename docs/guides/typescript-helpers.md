# TypeScript helpers

Kaji can emit a TypeScript SDK plus Zod validation, TanStack React/Vue Query,
SWR, Faker, MSW, and Cypress scaffolding. Select only the helpers your app uses.

```json
{
  "language": "typescript",
  "path": "typescript/fetch",
  "plugins": [
    { "name": "sdk", "transport": "fetch" },
    { "name": "zod", "output": "validation" },
    { "name": "tanstack-react-query", "output": "react-query", "clients_import": "../clients" },
    { "name": "faker", "output": "faker" },
    { "name": "msw", "output": "mocks" }
  ]
}
```

Keep helpers in the same package as the SDK whose operation functions they call.
Match `clients_import` and `group_by_tag` to the SDK layout.

## Validation

`zod` targets Zod 4 and emits component schemas, request/response schema maps,
and a `kajiSchemas` registry. Validate untrusted values at application
boundaries; generated clients do not silently validate every request/response.

```ts
import { PetSchema } from "@acme/pet-store/validation/zod";
const pet = PetSchema.parse(untrustedValue);
```

## Hooks

TanStack hooks wrap generated operations. React/Vue Query include GET queries
and non-GET mutations; SWR includes GET hooks. Set up providers, caching, SSR,
retries, and error policy in your framework.

```tsx
const query = useGetPet({ client: api.transport, path: { petId: "pet_123" } });
```

Faker creates schema-shaped samples. MSW and Cypress are editable scaffolding;
they are not complete behavioral mocks. See [testing generated SDKs](testing.md)
and the [full TypeScript example](../../examples/typescript-stack/README.md).

For every option and direct Rust API use, see [auxiliary generators](../auxiliary-generators.md).

See the [generator completion backlog](../generator-backlog.md) for language fixes, output size/splitting, framework APIs and artifact acceptance work. These items are planned, not current capabilities.
