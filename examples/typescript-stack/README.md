# Complete TypeScript stack

This is a small, runnable Kaji project. It deliberately produces two SDK
packages from one contract:

- `generated/typescript/fetch` is the complete Fetch SDK plus Zod, TanStack
  React Query, TanStack Vue Query, SWR, Faker, MSW, and Cypress artifacts.
- `generated/typescript/axios` is the same typed SDK surface backed by Axios.
- `generated/mock-server` is a Docker-based contract mock shared by either SDK.

Fetch and Axios are alternatives. The React/Vue/SWR helpers stay with the Fetch
package here so every generated hook imports exactly one set of operations. A
project that prefers Axios can put the same helper plugins in its Axios package
instead; do not combine the two transports in a single published package.

## Generate

Run this from this directory:

```sh
npx kajicli generate --config kaji.json
```

The generated Fetch package contains this layout:

```text
generated/typescript/fetch/
  client.ts                 # new Pets({ baseUrl, apiKey })
  clients/                  # direct, typed operation functions
  models/                   # contract types
  validation/zod.ts         # Zod schemas
  react-query/react-query.ts
  vue-query/vue-query.ts
  swr/swr.ts
  faker.ts
  mocks/msw.ts
  cypress/api.cy.ts
```

Install the generated package's dependencies before compiling or running it.
The framework integrations also require their normal application peer
dependencies (React/Vue and their framework setup). Kaji generates the typed
bindings; it does not install dependencies or configure an application for you.

## Full SDK: Fetch

```ts
import { Pets } from "@example/pets";

const pets = new Pets({
  baseUrl: "http://localhost:4010",
  apiKey: process.env.PETS_API_KEY,
});

const pet = await pets.pets.getPet({
  path: { petId: "pet_123" },
}).unwrap();
```

## Full SDK: Axios

```ts
import { Pets } from "@example/pets-axios";

const pets = new Pets({ baseUrl: "http://localhost:4010" });
const list = await pets.pets.listPets({}).unwrap();
```

## TanStack React Query and SWR

Both helpers call the generated operation functions. Provide the same options
you would provide to the direct operation: including the configured client.

```tsx
import { Pets } from "@example/pets";
import { useGetPet } from "@example/pets/react-query/react-query";

const client = new Pets({ baseUrl: "http://localhost:4010" });

export function Pet({ petId }: { petId: string }) {
  const query = useGetPet({ client: client.transport, path: { petId } });
  if (query.isPending) return <p>Loading…</p>;
  return <p>{query.data?.name}</p>;
}
```

```tsx
import { Pets } from "@example/pets";
import { useGetPet } from "@example/pets/swr/swr";

const client = new Pets({ baseUrl: "http://localhost:4010" });
const { data, error } = useGetPet({ client: client.transport, path: { petId: "pet_123" } });
```

The Vue Query file exposes the same generated hook names from
`@example/pets/vue-query/vue-query` for a Vue application.

## Zod, Faker, MSW, and the HTTP mock

```ts
import { PetSchema } from "@example/pets/validation/zod";
import { createPet } from "@example/pets/faker";
import { handlers } from "@example/pets/mocks/msw";
import { setupServer } from "msw/node";

const input = PetSchema.parse(createPet());
const server = setupServer(...handlers);
server.listen();
```

The MSW output is editable route-handler scaffolding, ideal for component and
unit tests. The generated HTTP mock is the contract-level choice for tests
that should exercise either SDK against a local server:

```sh
cd generated/mock-server
docker compose up --build
```

It listens on `http://localhost:4010` by default. Add durable conditional
responses to the OpenAPI document with `x-kaji-mock`; see
[contract mocking](../../docs/mocking.md).

## What is intentionally separate

- Use one framework integration per application. Generating React Query, Vue
  Query, and SWR together demonstrates the options, but a real app typically
  selects one.
- MSW is an in-process test interceptor. The `mock-server` package is a real
  HTTP service. They solve different test layers and can coexist.
- Cypress is test scaffolding. Add real paths, credentials, request bodies, and
  assertions before treating it as an end-to-end suite.
