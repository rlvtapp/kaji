# Poolster GitHub App broker

An optional, self-hosted GitHub Actions OIDC exchange service. It issues GitHub
App installation tokens for administrator-configured source-to-target mappings.
It uses Node.js builtins and has no dependencies.

- `broker.mjs`: signature verification, trust policy, scope checks, token minting.
- `server.mjs`: bounded HTTP server and environment-based startup.
- `action.yml` / `client.mjs`: vendorable composite action and revocation client.
- `policy.example.json`: non-secret source/target policy example.
- `broker.test.mjs`: signed JWT and mocked GitHub security/runtime tests.

See [deployment and configuration](../../../docs/reference/automation/github-app-broker.md). No hosted
endpoint, GitHub App registration, account, or infrastructure is created by this
package.

```sh
node --test packages/internal/github-app-broker/broker.test.mjs
```

The HTTP integration test binds to localhost. All GitHub/JWKS traffic in the tests
is mocked, and the RSA keys are generated test fixtures.
