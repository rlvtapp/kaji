# GitHub App token broker

Kaji can use your GitHub App directly with Actions secrets, or use this optional
broker to keep the App private key outside workflows. The broker exchanges a
signed GitHub Actions OIDC identity for a token restricted to configured SDK
repositories. Source repositories can also be configured as destinations when
Kaji needs to open a source update PR.

The code is self-hostable. This repository does not supply a running endpoint,
registered App, account onboarding service, or deployed infrastructure.

**On this page:** [Trust](#trust-and-permissions) · [Policy](#configure-your-app-and-policy) · [Deploy](#run-and-deploy) · [Action](#use-the-composite-action) · [Protocol](#protocol) · [Tests](#verification)

## Trust and permissions

The broker verifies RS256 signatures using the fixed [GitHub Actions issuer's
JWKS](https://token.actions.githubusercontent.com/.well-known/openid-configuration).
It checks issuer, exact audience, expiration, issue/not-before times, a bounded
lifetime and a single-use `jti`. JOSE-provided key URLs and algorithm substitutions
are rejected. Signature verification precedes repository authorization.

| Source identity check | Policy |
| --- | --- |
| Repository | Exact name and immutable repository/owner IDs |
| Workflow | Exact subject, path/ref and branch; signed `ref_protected` |
| Execution | Allowed event and runner environment |
| Optional restrictions | Environment and caller workflow SHA |
| Reusable workflow | Explicit delegation and pinned `job_workflow_sha` required |
| PR context | All PR events, `pull_request_target` and PR head/base contexts rejected |

[GitHub documents the identity claims](https://docs.github.com/en/actions/reference/security/oidc);
the discovery document includes `ref_protected`.

Destinations are administrator-owned mappings of repository name, immutable ID
and installation ID. Callers send repository names, never installation IDs,
permission grants, owner wildcards, or repository URLs. A request can contain up
to 50 destinations from one installation. Use separate exchanges for different
installations.

The App token request always includes explicit `repository_ids` and configured
`contents`/`pull_requests` permissions. No workflows, administration, organization
or account permissions are supported. Before returning a token, the broker checks
its permissions and independently lists the repositories accessible to that token,
checking names, IDs and count. Broader, transferred or otherwise unexpected scopes
fail closed and trigger best-effort revocation.

The App JWT uses a private RSA key and an expiration below GitHub's ten-minute
limit. Installation tokens expire after one hour; the API does not provide a
custom shorter expiration parameter. [GitHub App JWT
rules](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-json-web-token-jwt-for-a-github-app)
and [installation token
scoping](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-an-installation-access-token-for-a-github-app)
are the upstream contracts. Revoke the returned token at job completion to shorten
its useful lifetime.

## Configure your App and policy

1. Register a GitHub App under your own organization/account with Contents and
   Pull requests read/write, then install it on only the source/destination
   repositories it should manage. Extra App permissions are never copied to
   broker-issued tokens.
2. Generate its private key and store it in your deployment's secret manager or
   a private mounted file. Do not commit it. Workflow files need no App key.
3. Copy [policy.example.json](../packages/github-app-broker/policy.example.json)
   into an administrator-managed deployment configuration. Replace names and
   all source/destination/installation IDs with your own values.
4. Protect the allowed source branch. The workflow must run on that branch with
   an allowed event and its exact configured `workflow_ref`.
5. Set the `subjects` to the exact subject format used by the source repository.
   Newer repositories use immutable owner/repository IDs in their subjects;
   older repositories may retain `repo:OWNER/REPO:ref:refs/heads/main`. A job using
   an environment has a different subject. Custom subject templates must be
   configured explicitly. Never log full bearer tokens to inspect claims.

The default audience is `kaji`. A deployment-specific audience, such as your
broker's HTTPS URL, is preferable when operating multiple independent brokers;
configure that exact value in the policy and the action input.

The example allows only GitHub-hosted runners. `runner_environment: self-hosted`
is an explicit opt-in for administrator-controlled runners. Add `environment`
when the job should also require a protected deployment environment. Optional
`workflow_sha` pins the caller workflow to an approved revision; reusable workflow
policies require both `job_workflow_ref` and immutable `job_workflow_sha`.

Adding the source itself to `targets` allows source update PRs under the same
policy. This does not happen automatically just because the source is trusted.
The broker deliberately lacks the Workflows write permission; initial commits
that create/edit `.github/workflows` require a human or separately authorized
setup credential.

## Run and deploy

Requires Node.js 22 or newer. Supply:

| Variable | Purpose |
| --- | --- |
| `KAJI_BROKER_POLICY` | Administrator-owned policy JSON file |
| `GITHUB_APP_ID` | Numeric GitHub App ID |
| `GITHUB_APP_PRIVATE_KEY_FILE` | Private mounted RSA PEM file |
| `GITHUB_APP_PRIVATE_KEY` | Alternative secret-manager injected PEM value |
| `KAJI_BROKER_REPLAY_DIRECTORY` | Private persistent replay-marker directory |
| `HOST` | Bind address, default `127.0.0.1` |
| `PORT` | Port, default `8787` |

```sh
node packages/github-app-broker/server.mjs
```

Startup validates configuration and credentials before listening. The server
provides `GET /healthz` and `POST /token`; token responses have `Cache-Control:
no-store`. Logs contain issuance/denial metadata and request IDs, never bearer
JWTs, installation tokens, App JWTs or private keys.

For public deployment:

- Put the listener behind HTTPS ingress and run it unprivileged.
- Supply App credentials, administrator-owned policy and persistent replay storage.
- Set ingress rate limits and permit outbound access to the fixed GitHub API/JWKS hosts.
- Exclude Authorization headers and response bodies from proxy/access logs.
- Maintain clock synchronization and GitHub API access.
- Restart after configuration or key changes.

The HTTP layer limits headers, body size, concurrency, request time and requests
per connection-peer address. It never trusts arbitrary `X-Forwarded-For` values.
If a reverse proxy funnels all requests through one peer address, its limits apply
to that shared address; enforce public per-client limits at ingress as well.

| Replay storage | Supported use |
| --- | --- |
| Persistent directory with atomic file creation | Single instance; survives restarts |
| Shared filesystem with atomic exclusive creation | Multiple instances |
| Injected durable store with atomic `consume(key, expiresAt)` | Multiple instances |
| Memory store | Tests and embedded single-process use |

Independent per-replica volumes cannot guarantee global replay protection.
Preserve replay storage across restarts and keep it separate from untrusted files.

## Use the composite action

Vendor `action.yml` and `client.mjs` into `.github/actions/kaji-token`, or reference
a reviewed commit/tag of a published repository containing this package. The local
vendored action works before this code is released publicly.

```yaml
permissions:
  contents: read
  id-token: write

jobs:
  sync:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4 # Pin a reviewed full commit in production.
      - uses: actions/setup-node@v4
        with:
          node-version: '22'
      - id: sdk-token
        uses: ./.github/actions/kaji-token
        with:
          broker-url: https://broker.example
          audience: kaji
          repositories: '["example/typescript-sdk"]'
      # Pass steps.sdk-token.outputs.token only to the SDK sync operation.
      - name: Revoke SDK token
        if: ${{ always() && steps.sdk-token.outputs.token != '' }}
        env:
          KAJI_INSTALLATION_TOKEN: ${{ steps.sdk-token.outputs.token }}
        run: node .github/actions/kaji-token/client.mjs --revoke
```

The action requests OIDC from the official GitHub Actions request URL, sends the
repository array to the broker, masks the returned token before writing
`GITHUB_OUTPUT`, and exposes `token` and `expires-at`.
Use the token as a secret
input/environment value, not a URL or command literal.

Composite actions cannot
register an automatic post hook, so the explicit `always()` cleanup step is
necessary for immediate revocation.
If cleanup cannot run, GitHub's one-hour
expiration remains the limit.

## Protocol

```http
POST /token
Authorization: Bearer <GitHub Actions OIDC JWT>
Content-Type: application/json

{"repositories":["example/typescript-sdk"]}
```

A successful response contains `token`, `expires_at`, `repositories`, and
`permissions`. Reusing an OIDC JWT is rejected; obtain a fresh identity for each
exchange. Failed upstream calls also consume that identity, so retry by requesting
a new GitHub OIDC token. No automatic retries can mint duplicate tokens with the
same identity.

## Verification

```sh
node --test packages/github-app-broker/broker.test.mjs
```

Tests generate real RSA key pairs and signed JWTs, mock the official JWKS/GitHub
APIs, and cover forged signatures, algorithm/key URL attacks, claim mismatches,
PR/fork context, scope and permission escalation, replay, persistent restart
behavior, the local HTTP endpoint, the vendored client, masking and revocation.
The HTTP integration case needs permission to bind to `127.0.0.1`. Tests do not
contact GitHub, create an App, deploy the broker, or use real credentials.
