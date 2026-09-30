# Mock scenarios example

This contract has a normal response and an explicit rate-limit scenario. It is
useful for SDK integration tests that need real requests but not a live API.

Run the native mock without Docker:

```sh
cd examples/mock-scenarios
npx @relevate/kaji mock serve openapi.yaml --port 4010
```

Or generate and run the Docker fixture:

```sh
cd examples/mock-scenarios
npx @relevate/kaji generate
cd generated/mock-server
docker compose up --build
```

In a second terminal, compare the normal and scenario responses:

```sh
curl http://localhost:4010/notes/note_123
curl -H 'x-test-scenario: rate-limited' http://localhost:4010/notes/note_123
```

The `x-kaji-mock` extension belongs in the OpenAPI source. Regenerate after
changing it instead of editing generated fixture YAML. See
[contract mocking](../../docs/mocking.md) for all predicates and response fields.
