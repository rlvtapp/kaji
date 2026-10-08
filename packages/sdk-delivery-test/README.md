# SDK delivery verification harness

Run from the repository root with Node 20+ and a built Poolster CLI:

```sh
cargo build -p poolster-cli
node --test packages/sdk-delivery-test/test/delivery.test.mjs
node packages/sdk-delivery-test/delivery.mjs --mode local --binary "$PWD/target/debug/poolster"
```

The local run creates a temporary contract and recipe, generates a TypeScript
SDK, verifies zero drift, inspects owned artifacts, runs metadata-declared build
and test commands, and reviews delivery scaffold in dry-run mode. The probe
commands run Node itself; this test does not claim to compile the TypeScript
SDK. Its temporary checkout is always removed. Mock tests verify step order,
stop-on-failure and absence of PR/install/publish commands.

An opt-in authenticated observation uses an explicitly named disposable SDK
repository and registry label:

```sh
node packages/sdk-delivery-test/delivery.mjs --mode live \
  --binary "$PWD/target/debug/poolster" \
  --repository YOUR_ORG/DISPOSABLE_SDK_TEST_REPO --registry TEST_REGISTRY
```

This adds read-only remote workflow/PR/configuration-name observations through
`gh`. It does not publish, create repositories, open PRs, install workflow files,
or inspect credentials. The registry argument records the intended integration
destination; it does not verify registry access. Complete live publication
verification separately using the reviewed SDK delivery workflows, disposable
packages and registry trust configured by the operator. No live run was performed
as part of the implementation tests.

## Prepare a manually approved disposable delivery workflow

Copy [workflow.yml.template](workflow.yml.template) into the workflow repository
as `.github/workflows/poolster-delivery-test.yml`; retain this package's validation
script in that repository. Configure an environment named `poolster-delivery-test`
with required reviewers before dispatching. GitHub environment approvals are
repository settings; the template cannot create or enforce those rules itself.

Set repository variables to JSON arrays containing only disposable test targets:

- `POOLSTER_DELIVERY_TEST_REPOSITORIES`: source and destination `OWNER/REPO` names.
- `POOLSTER_DELIVERY_TEST_PACKAGE_PREFIXES`: registry package prefixes reserved for
  tests, for example `["@YOUR_TEST_SCOPE/poolster-delivery-"]`.

Set environment secrets `POOLSTER_TEST_SOURCE_READ_TOKEN` (source contents read only)
and `POOLSTER_TEST_SDK_WRITE_TOKEN` (only disposable destination contents/PR write).
Use short-lived installation tokens or scoped credentials. Do not put registry
credentials in the source verification job. Registry trust belongs in the
reviewed normal release workflow of the disposable destination repository.
Install that SDK repository's regular CI/Release Please/publishing scaffold first.

Dispatch with an existing immutable source tag, exact released Poolster version,
selected SDK language, registry and allowlisted prefix. `preview` is the default:
it validates identities, checks out that exact tag, generates in a disposable
copy, checks drift, runs native metadata checks, and prints delivery/PR previews.
`open_pr` uses the protected environment and destination-scoped token to open an
isolated SDK PR from the pristine tagged source checkout. Neither mode merges a
PR, pushes a release tag or directly publishes a package. Review/merge the SDK PR,
then review/merge the ordinary Release Please PR; the destination's existing
release workflow performs publication. The template therefore exercises the
normal reviewed delivery path rather than bypassing it.

Tests cover input allowlists, pinned versions, unsafe refs/paths, package
identities and a local Git release tag feeding the real build/test and publisher
modules with mocked npm/registry boundaries. This is mock publication evidence;
no GitHub workflow or remote registry publication has been executed.

## Repository CI coverage

The repository CI runs this package's mock tests and local seven-step delivery
probe. It also exports one generated runtime-contract artifact and executes its
public-operation probe in all ten native language jobs, preserving the separate
SDK build checks. Newman execution and Terraform CLI lifecycle checks run against
local mock servers in the workspace job. The standalone custom-plugin example is
built and tested through its own Cargo manifest, so plugin compatibility is not
inferred solely from workspace compilation.

The CI workflow contains no package publication step. Configuring these jobs
does not establish that every remote runner has passed: the workflow run supplies
that evidence, and missing native tools/dependencies fail their jobs.
