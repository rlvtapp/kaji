# Generate Postman collections

| Task | Section |
| --- | --- |
| Export collection and environment | [Generate](#generate-a-collection-and-environment) |
| Check mapping diagnostics | [Review requests](#review-the-exported-requests) |
| Supply custom examples | [Plugin composition](#customize-through-plugins) |
| Validate or execute locally | [Distribution checks](#validate-before-distributing) |

Kaji exports portable Collection 2.1 JSON through an independent Postman plugin. It uses
the same normalized API and security catalog as SDK generation. You can ship a
collection beside your SDK without depending on a generated client. These commands
require a build containing the new target; the published 0.4.0 launcher does not contain
it.

## Generate a collection and environment

The [combined example](../examples/api-artifacts/README.md) generates Postman exports
and a Terraform provider from one specification. A Postman-only recipe package looks
like this:

```json
{
  "language": "postman",
  "path": "postman",
  "plugins": [
    {"name": "collection", "output": "api.postman_collection.json", "strict": true},
    {"name": "environment", "output": "api.postman_environment.json"}
  ]
}
```

Run `kaji generate --config kaji.json`. Alternatively select `--language postman` in a
direct generate command; default filenames are `collection.json` and `environment.json`.
`all` remains the SDK-only shortcut.

Import the collection and environment into Postman. Select the environment and supply
your API origin and credentials locally. Exported credentials start blank; the
environment is create-once so regenerating does not overwrite your values. Keep
populated environment files outside source control.

The collection remains owned generated output and participates in `generate --check` and
safe cleanup.

## Standalone collections per resource

Set `"split_by_group": true` on the collection plugin to also emit
`collections/group-0000.json`, `group-0001.json`, and subsequent folders. Each file
is a standalone Collection 2.1 document containing one tag folder (or path-resource
folder with `"group_by_tag": false`). The aggregate export remains available.
Shared blank credential placeholders and base URL variables are copied to each
collection. Request IDs remain identical to the aggregate; collection IDs are
deterministic and distinct. Numeric filenames safely handle tags containing slashes
or differing only in case. These files are owned generated output; regeneration refuses locally modified
exports until you preserve or restore them. Put authored
scripts or manually edited requests in a separate collection. The environment
remains create-once and preserves local credentials.

## Review the exported requests

Requests are grouped by tags by default. Set `group_by_tag: false` to group by the first
path segment. IDs are deterministic, and each request retains its operation identity,
method, path and documentation. JSON, URL-encoded, multipart and binary bodies are
handled separately. Declared examples take precedence over bounded schema samples.

Schema `writeOnly`, sensitive fields and familiar credential names are redacted from
examples.

Operation-level server precedence comes from the compiler; server defaults and variables
are retained. Set `base_url` on the collection plugin to override the exported origin.
Multiple servers or authentication alternatives must be reviewed alongside the generated
diagnostics.

Credential placeholders do not acquire OAuth tokens: obtain tokens using your own login
flow and supply them locally.

### Resolve diagnostics

The collection plugin writes a diagnostics file alongside its output. CLI recipe
`strict` defaults to true and rejects error-level unsupported mappings. The native
collection builder and direct target report diagnostics by default; call `.strict(true)`
when incomplete exports should fail generation. Incomplete requests are marked in their
descriptions.

Review diagnostics before running a collection; a valid Collection schema alone does not
prove correct API behavior.

## Customize through plugins

The native API exposes `RequestExamples`, `CollectionDocument`, and
`EnvironmentTemplate` typed contracts. Bind named handles when your package has multiple
providers:

```rust
use kaji::{postman, prelude::*};
let examples = postman::examples();
let collection = postman::collection()
    .using_examples(examples.handle())
    .strict(true);
let environment = postman::environment().using_collection(collection.handle());
let package = postman::package("postman")
    .with(examples)
    .with(collection)
    .with(environment);
```

A custom example provider can supply operation/media-specific values; the renderer still
applies redaction. Replace the collection renderer with your own plugin when you need
custom scripts or a different serialization strategy. Kaji does not import JavaScript
from the specification or execute requests during generation.

## Validate before distributing

The editable [Postman check action](../packages/postman-check/README.md) validates
exports against the pinned official Collection 2.1 schema, verifies unique request IDs
and checks that secret environment values stay blank. It does not run requests. Copy its
source into your repository or use a revision that includes the action:

```yaml
- uses: ./path/to/postman-check
  with:
    collection: generated/postman/api.postman_collection.json
    environment: generated/postman/api.postman_environment.json
```

### Run a local execution check

For an opt-in execution check, the editable [local Newman
runner](../packages/postman-execute/README.md) executes one bounded iteration against
its own ephemeral loopback mock. It rejects existing collection scripts, replaces all
request destinations/auth helpers, and asserts the first saved successful response
status.

No environment credentials or remote API access are used:

```sh
npm ci --prefix packages/postman-execute --ignore-scripts
node packages/postman-execute/run.mjs generated/postman/api.postman_collection.json
```

The runner tests and an actual generated collection execution passed with pinned Newman
6.2.1. This verifies executable collection structure, not original path/auth handling,
response semantics, ordered CRUD behavior or streaming. Keep schema validation and
generator mapping tests alongside it.

### Synchronize reviewed changes

Execution through Postman or Newman is a separate, deliberate test step. Use an explicit
sandbox/mock URL and test fixtures, especially for create/delete operations. Portable
export and validation are implemented.

The editable [Postman synchronization helper](../packages/postman-sync/README.md) checks
or updates an explicitly selected existing collection with a reviewed remote hash. The
companion environment action checks and publishes reviewed updates to an existing
environment, preserving remote secrets and manually added variables.

It does not create or relocate workspaces or publish release assets. Live Postman API
execution remains unverified.
