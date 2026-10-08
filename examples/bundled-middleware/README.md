# Ship middleware that customers do not have to register

This example generates a TypeScript Fetch SDK for a Notes API. The SDK author
keeps a policy in `middleware/author-policy.ts`; Poolster copies it into the SDK and
registers it automatically. Every generated request receives
`X-SDK-Policy: bundled`. A customer just calls `new Notes(...)` and `listNotes()`.

The example also supplies an executable SDK test and release metadata. It does
not publish anything or require a running API to verify the policy.

## Prerequisites

The example uses the middleware and delivery APIs in the current source tree.
Use a Poolster release containing those APIs once published. For source builds,
you need Rust and Go; compiling/testing the SDK needs Node 22+ and npm.

From the repository root, build the generator and its OpenAPI compiler:

```sh
cargo build -p poolster-cli
(cd openapi && go build -o ../target/debug/poolster-openapi .)
```

## Generate, build, and prove the default behavior

```sh
./target/debug/poolster generate --config examples/bundled-middleware/poolster.json
cd examples/bundled-middleware/generated/typescript
npm install --ignore-scripts
npm run build
node tests/verify.cjs
```

Expected test output:

```text
SDK author policy runs without consumer registration.
```

The test supplies a fake HTTP driver, checks the policy's header and decoded
result, and never sends a network request. It compiles an isolated CommonJS test
copy; the package's normal ESM output remains for bundler consumers. Direct Node
ESM use may require a build adapter because the generated TypeScript source uses
extensionless imports.

## What belongs in source control

```text
poolster.json                         recipe, middleware registration, release metadata
openapi.yaml                      API contract
middleware/author-policy.ts       author-maintained policy source
tests/verify.cjs                   author-maintained executable SDK test
generated/typescript/              generated SDK and copied source/test
```

Commit the recipe, contract, middleware, and test. Commit generated output too
if you review SDK changes in pull requests. Edit the original policy, not its
copy under `generated/`.

`middleware[].source` is relative to `poolster.json`; `middleware[].path` is relative
to the SDK package. The `symbol` names the exported `ClientMiddleware` function.
The policy imports runtime types with `import type`, avoiding a runtime import
cycle. `customizations` copies the test; `release` declares build/test commands
and opts into the standard npm publisher.

## Change the policy and regenerate

From the repository root:

```sh
./target/debug/poolster generate --config examples/bundled-middleware/poolster.json --check
```

This succeeds with current output. Change the original policy, then repeat the
check: it reports drift without writing. Run the same command without `--check`
to update the SDK, then run its build/test again. Removing the middleware entry
removes the automatic registration and deletes its unchanged owned source copy.

## Prepare delivery in your own repository

Copy this example into the root of your API repository and use the matching
Poolster executable. Generate there, then preview the automation:

```sh
poolster sdk init --root . --config poolster.json --actions local --dry-run
```

Review the proposed files before running without `--dry-run`. The scaffolder
needs the generated `.poolster/package.json`, supplied by this example's `release`
entry. It does not install a GitHub App or register a trusted npm publisher.
Replace `@example/notes` with a package name you own before any publication.

For a separate SDK repository, follow [SDK repository automation](../../docs/sdk-automation.md)
for the destination bootstrap and scoped App authentication. See
[SDK publishing](../../docs/sdk-publishing.md) for trusted-publisher registration,
exact-tag checks, and retry behavior. See [SDK customization](../../docs/sdk-customization.md)
for other language contracts and source overrides.
