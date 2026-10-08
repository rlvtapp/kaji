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
    { "name": "tanstack-react-query", "output": "react-query" },
    { "name": "faker", "output": "faker" },
    { "name": "msw", "output": "mocks" }
  ]
}
```

Keep helpers in the same package as the SDK whose operation functions they call.
Native consumers resolve imports through the selected operation provider. Use
`uses.operations` when the package has multiple providers; relocation and renamed
operation symbols are resolved automatically.

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
and non-GET mutations; SWR includes GET hooks. Query/mutation option factories
can be reused for prefetching and native framework observers. Set up providers
and application cache policy in your framework.

```tsx
const query = useGetPet(
  { client: api.transport, path: { petId: "pet_123" }, throwOnError: true },
  { staleTime: 60_000, select: (pet) => pet.name },
  "tenant_123",
);
```

Faker creates schema-shaped samples. MSW and Cypress are editable scaffolding;
they are not complete behavioral mocks. See [testing generated SDKs](testing.md)
and the [full TypeScript example](../../examples/typescript-stack/README.md).

For every option and direct Rust API use, see [auxiliary generators](../auxiliary-generators.md).

See the [generator completion backlog](../generator-backlog.md) for language fixes, output size/splitting, framework APIs and artifact acceptance work. These items are planned, not current capabilities.

TanStack helpers forward cancellation into `requestOptions.signal` and retain a
caller-provided signal. Query keys contain the operation, explicit cache scope
and path/query/body inputs. They omit the client, headers and transport config.
Use a distinct scope when tenants, origins or response-affecting headers differ;
request inputs themselves are still visible in the framework cache. Pass
`throwOnError: true` for SDK errors to reject queries/mutations.

Each GET operation also exports `<operation>QueryKey` and
`<operation>QueryOptions`; each mutation exports `<operation>MutationKey` and
`<operation>MutationOptions`. Hook overrides preserve `select`, stale-time,
retry and mutation callback/context types. SWR accepts its native configuration
as the second argument and a scope as the third.

Native React/Vue/SWR consumers split after 50 operations per module by default,
retaining the configured entrypoint as a barrel. Set
`max_operations_per_file` on the query plugin in `kaji.json`, or use
`.max_operations_per_file(...)` / `.single_file()` in the Rust composition API.
Standalone artifact renderers retain their single-file layout. Infinite-query
and suspense helpers are still tracked in the backlog.
