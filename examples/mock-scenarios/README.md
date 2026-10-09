# Mock scenarios example

This contract has a normal response and an explicit rate-limit scenario. It is
useful for SDK integration tests that need real requests but not a live API.

Run the native mock without Docker:

```sh
cd examples/mock-scenarios
npx poolster mock serve openapi.yaml --port 4010
```

Or generate and run the Docker fixture:

```sh
cd examples/mock-scenarios
npx poolster generate
cd generated/mock-server
docker compose up --build
```

In a second terminal, compare the normal and scenario responses:

```sh
curl http://localhost:4010/notes/note_123
curl -H 'x-test-scenario: rate-limited' http://localhost:4010/notes/note_123
```

## Diagnose an agent or SDK call

The native mock is especially useful for agent evaluation because it exposes
both a health check and a bounded, machine-readable request log—without a
dashboard or Docker. Start the native server, then make a few calls:

```sh
curl http://127.0.0.1:4010/_poolster/health
curl http://127.0.0.1:4010/notes/random
curl http://127.0.0.1:4010/notes/random
curl http://127.0.0.1:4010/_poolster/requests
```

`/notes/random` has no fixed examples, so the two successful responses contain
different schema-shaped values. The request log records which OpenAPI operation
matched, status code, optional scenario, and a safely bounded request body.
That lets a person or agent distinguish a malformed call, an unmatched route,
and an intentional `rate-limited` scenario quickly.

The `x-poolster-mock` extension belongs in the OpenAPI source. Regenerate after
changing it instead of editing generated fixture YAML. See
[contract mocking](../../docs/reference/outputs/mocking.md) for all predicates and response fields.
