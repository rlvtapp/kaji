# Poolster Pets mock server

This is a language-neutral HTTP mock generated from the same OpenAPI contract as the SDKs. Every generated SDK can use it by setting its base URL to `http://localhost:4010`.

## Run

```sh
cp .env.example .env # optional: choose a different host port
docker compose up --build
# or: sh ./run.sh
```

The server reads `fixtures/*.yaml` on startup through [httpmock](https://httpmock.rs/). Those fixtures match HTTP method, path, and any OpenAPI or `x-poolster-mock` scenario conditions. A missing match returns `404`, which makes unmodelled calls visible in tests.

## Use from an SDK

Pass `http://localhost:${POOLSTER_MOCK_PORT:-4010}` as the SDK base URL. Keep test-only scenario selectors (for example `x-test-scenario`) in the request that invokes the SDK; the generated fixture documents the selector.

## Customize safely

- Regenerate this directory when the OpenAPI contract changes.
- Put product-specific scenarios in `x-poolster-mock` in the OpenAPI document so they travel with the contract.
- The generated fixture files are intentionally plain YAML. You may edit them for a local experiment, but commit source-contract scenarios for durable behavior.

See [`fixtures/README.md`](fixtures/README.md) for the generated route inventory.
