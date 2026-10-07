# Sync a source OpenAPI file into an SDK review PR

Run this action in the API source repository after checking out the commit containing the specification. It copies one local spec to a destination SDK repository, records provenance and creates or updates a reviewable PR. The destination's existing generation workflow can regenerate SDKs from the spec change. This action does not merge PRs or publish SDK packages.

The action and `sync.mjs` are readable and editable. Copy this directory into your source repository as `.github/actions/kaji-spec-sync`, or reference the version of this action your organization distributes.

## Inputs

| Input | Meaning |
| --- | --- |
| `repository` | Destination SDK repository, `owner/name`. |
| `source-path` | Checkout-relative local `.yaml`, `.yml` or JSON OpenAPI file. |
| `target-path` | Repository-relative destination spec path. It cannot address `.github`, `.git` or `.kaji`. |
| `token` | GitHub App installation or fine-grained token for the destination, with **Contents: write** and **Pull requests: write**. |
| `branch` | Review branch; default `codex/kaji-spec-sync`. Existing commits are preserved. |
| `base` | Destination PR base; default `main`. Must differ from the review branch. |

Outputs are `pull-request-url` (empty when there is no difference requiring a PR) and `changed` (whether this run committed file updates).

After checking out the API source commit and obtaining a destination-scoped token, invoke the local action:

```yaml
- uses: ./.github/actions/kaji-spec-sync
  with:
    repository: acme/customer-sdks
    source-path: api/openapi.yaml
    target-path: specs/openapi.yaml
    token: ${{ secrets.SDK_SPEC_SYNC_TOKEN }}
    branch: codex/kaji-spec-sync
    base: main
```

The source repository's default `GITHUB_TOKEN` generally cannot write a different repository. Use a destination-scoped GitHub App token or fine-grained token. This action does not register an App, grant permissions or provision tokens. Inputs reach the helper through environment variables; no input is interpolated into a shell command. The action uses Node 24 and GitHub's public REST API.

## What happens on a run

1. Validate the local source path, UTF-8 contents and reference shape. Reject symlinks and checkout escapes before contacting GitHub.
2. Read the destination base and review branch. An existing branch's current commit becomes the parent; a new branch starts from the configured base.
3. Compare the spec and `.kaji/spec-source.json` provenance. Existing provenance must identify the same source repository/path, and its digest must match the destination spec. Manual edits or another connection stop the sync without overwriting files.
4. Create blobs and a tree based on the existing branch tree, touching only the spec and provenance. Create a commit and update the ref without force. Concurrent branch changes fail; rerun to start from the new head.
5. Update the matching open PR, or create one if the review branch has commits ahead of base. A merged branch with no new difference does not produce an empty PR.

Provenance contains the source repository, source commit SHA, source path and SHA-256 of the exact spec bytes. When those bytes are unchanged, retain the last spec-changing SHA: unrelated API commits do not create provenance-only regeneration loops. A changed source spec creates a new review commit. Repeated identical runs do not create more commits.

On first adoption, an existing target on the base branch can be replaced through the review PR. An existing review branch with a target spec but no provenance is not adopted implicitly; reconcile it or choose a new review branch. One provenance record represents one source connection. Existing destination files unrelated to the two updated paths remain in the Git tree.

## Reference and format limits

Internal references (`#/components/...`) and absolute URL references are accepted. Relative external references (`./schemas.yaml`, `../common.json#/Thing`) are rejected: copying a single file would leave the dependency behind. Bundle those references into one spec before running the action.

JSON is parsed and recursively checked for `$ref`. YAML uses a conservative literal-scalar scan; reference aliases, anchors, tagged values and multiline reference scalars are unsupported and fail explicitly. Convert/bundle to JSON when you need those YAML constructs. This action is a source sync operation, not an OpenAPI schema validator; run the source/destination project's contract checks as part of CI.

## Local tests

```sh
npm test --prefix packages/spec-sync
node --check packages/spec-sync/sync.mjs
```

Tests mock GitHub's API and perform no live repository writes. They verify branch ancestry, nonforce ref updates, idempotency, PR handling, provenance protection, relative-ref rejection, safe paths and credential-safe failures. Test the destination's regeneration workflow separately before enabling your production source connection.
