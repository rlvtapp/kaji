# Test generated SDKs

Choose the test layer that matches the behavior you need.

| Tool | Best for | Not for |
| --- | --- | --- |
| Faker | Schema-shaped test data | Validated/scenario-specific fixtures |
| MSW | In-process frontend tests | Shared HTTP behavior across languages |
| Docker mock | SDK integration through real HTTP | Stateful product behavior |
| Cypress scaffold | Starting API smoke tests | A complete e2e suite |

## Docker contract mock

Add a separate mock package to the recipe:

```json
{ "language": "mock", "path": "mock-server", "plugins": [{ "name": "server", "port": 4010 }] }
```

Then run the generated service and point SDK clients at it:

```sh
cd generated/mock-server
docker compose up --build
```

It returns deterministic contract-derived happy paths. Add durable conditional
responses with `x-kaji-mock` in the OpenAPI source. Keep stateful workflows in
a dedicated test service. Read [contract mocking](../mocking.md) for the full
scenario format.

MSW handlers are editable in-process scaffolding. Cypress output needs real
paths, credentials, bodies, and assertions before use; never run generated
mutation tests against production. The [TypeScript stack example](../../examples/typescript-stack/README.md)
shows Faker, MSW, Cypress, and the Docker mock together.
