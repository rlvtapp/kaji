# GitHub App authentication and SDK sync

Poolster can use a repository-scoped GitHub App installation token for generated SDK
pull requests and Release Please. The App authenticates automation; generation,
checks and release planning still run in the repositories' readable workflows.
Registry publishing is a separate trusted identity or custom publisher command.

**On this page:** [Private App](#private-app) · [OIDC broker](#hosted-app-with-oidc-broker) · [Token](#other-authentication)

## Private App

```sh
poolster sdk app --name "Acme SDK Sync" --dry-run
poolster sdk app --name "Acme SDK Sync"
poolster sdk init --root generated --config poolster.json --auth app --dry-run
poolster sdk sync --root generated --config poolster.json --auth app
```

1. Review the manifest written by `sdk app`.
2. Register the App in your account/organization settings with its name and
   homepage, plus **Contents: write** and **Pull requests: write**.
3. Install it on the selected source and SDK repositories.
4. Set the following repository/organization values:

| Value | Storage |
| --- | --- |
| `SDK_APP_CLIENT_ID` | Variable |
| `SDK_APP_PRIVATE_KEY` | Secret containing the PEM private key |

GitHub Actions triggers generation; no webhook subscription is required.
An automated registration portal can pass the manifest to GitHub and exchange
its temporary code server-side. The CLI supplies neither registration nor a portal.

`actions/create-github-app-token` creates a token for the specific owner/repository
in each workflow and revokes it after the job. App pushes/PRs trigger downstream
CI, unlike the default `GITHUB_TOKEN` behavior. The private App path stores the
App key in Actions secrets; workflows obtain fresh installation tokens per run.
App permission changes require approval in GitHub. Poolster does not ask for workflow
write permission or mutate installations, repository secrets or branch rules.

`sync` refreshes **local** editable workflow/action files, ready to commit. SDK
content synchronization happens through the resulting workflow's `sdk pr`
command. Initial workflow installation is a normal owner-reviewed commit.

For a separate SDK repository, add `--repository owner/sdk-repo`. Generation stays
in the API repository, and destination checks/release configuration/actions go
under `.poolster/sdk-repository-setup/`. Copy that directory's contents to the SDK
repository root and commit them there before enabling generation. Paths match
`output.path` in the source recipe. Do not install the destination release workflow
in the API repository. Routine App tokens cannot replace workflow definitions.

## Hosted App with OIDC broker

A hosted App can exchange a workflow OIDC identity for an installation token.
Poolster includes editable broker sources so you can host this architecture yourself:

```sh
poolster sdk sync --root generated --config poolster.json \
  --auth broker --broker-url https://your-broker.example \
  --repository acme/sdks --dry-run
```

The broker holds the App key; repository workflows store no App key or long-lived
PAT. Each source workflow must be explicitly mapped to its approved destination
repositories/installations in the broker's policy. See
[broker deployment and policy](github-app-broker.md). Register/install your App and
host the broker before enabling this mode. There is no publicly hosted Poolster App
or broker created by these commands.

The broker token action can be vendored into each repository. Composite actions
cannot register a post-job hook, so explicitly revoke a returned token after use
with `client.mjs --revoke`, or let GitHub expire it. The broker uses a persistent
single-use OIDC replay store; multiple instances need shared storage.

## Other authentication

`--auth token` retains the `SDK_GITHUB_TOKEN` secret path. Use an App or a
fine-grained token with target repository access so SDK PRs trigger CI. Scope
cross-repository credentials to the repositories being synchronized.

Editable sources: [broker](../packages/internal/github-app-broker),
[checks](../packages/internal/sdk-check), [publishers](../packages/internal/sdk-publish),
and [workflow generator](../crates/cli/src/sdk_automation.rs).
