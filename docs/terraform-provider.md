# Generate a typed Terraform provider

Poolster's `terraform` plugin generates a Go provider using HashiCorp's Terraform
Plugin Framework. It builds a validated entity catalog before rendering resources;
OpenAPI HTTP operations alone are not a promise of Terraform lifecycle semantics.
This is the first typed implementation, with a deliberately bounded supported
schema subset. It requires a build newer than the published 0.4.0 launcher.

## Bind resource operations

Start with the [combined example](../examples/api-artifacts/README.md). Its provider
package declares the API's lifecycle explicitly:

```json
{
  "language": "terraform",
  "path": "terraform",
  "plugins": [{
    "name": "provider",
    "provider_name": "widgets",
    "module": "example.com/terraform-provider-widgets",
    "infer": false,
    "resources": [{
      "name": "widget",
      "create": "createWidget",
      "read": "getWidget",
      "update": "updateWidget",
      "delete": "deleteWidget",
      "id_parameter": "id",
      "id_field": "id"
    }]
  }]
}
```

`update` is optional. Configurable fields without a supported update binding
require replacement. A single identity comes from a required string field in
the create/read response and one final item path parameter. Explicit composite
bindings support configured parent IDs and server-generated child IDs. Each binding is
validated against the referenced operations and schemas before files are written.
Invalid explicit bindings fail generation rather than producing a partial resource.

Set `infer: true` or select `--language terraform` to consider conventional
collection POST and matching item GET/PATCH-or-PUT/DELETE operations. Inference
emits only validated candidates; explicit mappings are preferable for production
contracts. The generated `.poolster/terraform-plan.json` explains accepted resources
and excluded operations. Examine it even when generation succeeds.

## Understand the supported state behavior

The current implementation handles fixed nested JSON objects, typed lists/maps,
string, boolean, integer and number attributes; single or composite string identity; resource import; ordinary CRUD;
and supported single-scheme HTTP bearer/basic/API-key authentication.
Configured values, computed response fields, identity and replacement decisions
have distinct roles in the plan. Create/update must preserve known planned values;
server normalization that would violate Terraform state consistency is reported.
Read refreshes state and removes a resource on a missing-object response.
Declared `202` lifecycle success requires explicit polling; wildcard `2XX` is
excluded. Without a configured waiter, unexpected `202` fails. Delete
also treats an already-missing object as success. Error messages avoid dumping
HTTP response bodies or credentials.

Nullable values, unions, recursive shapes, unsupported constraints, differing nested
read/write projections, write-only secrets, independent/list data sources
and advanced actions remain excluded. See the [Speakeasy comparison](terraform-speakeasy.md)
for the supported subset and remaining work.

## Composite identities and state upgrades

For a configured parent ID and server-generated child ID, replace `id_parameter`
and `id_field` with explicit mappings:

```json
"identity": [
  {"parameter": "organization", "field": "organizationId"},
  {"parameter": "id", "field": "id"}
]
```

The planner validates every lifecycle path and response mapping. Composite imports
use a JSON object keyed by parameter names, such as
`'{"organization":"acme","id":"widget-1"}'`. Parent configuration stays out of
the create body unless the API declares it there. Generated documentation includes
nested HCL examples and the exact import shape.

For a root attribute rename, explicitly declare each prior version's direct
upgrade to the current schema:

```json
"schema_version": 1,
"state_upgrades": [{"version": 0, "rename_fields": {"old_name": "name"}}]
```

Framework upgrade callbacks preserve values and reject collisions or incompatible
state. These are root-field renames, not arbitrary type conversions. Test stored
old state before releasing a changed provider; migrations are never inferred.

## Wait for asynchronous lifecycle completion

Explicit resource bindings can attach a waiter to create, update or delete:

```json
"polling": {
  "create": {
    "interval_ms": 2000,
    "max_attempts": 120,
    "timeout_ms": 300000,
    "success": [{"status": 200}, {"pointer": "/status", "equals": "ready"}],
    "failure": [{"status": 200}, {"pointer": "/status", "equals": "failed"}]
  },
  "delete": {"success": [{"status": 404}]}
}
```

The existing authenticated read GET is the waiter; simple and composite IDs use
normal path escaping. Each group is a conjunction evaluated against one response.
Failure takes precedence. Body criteria use RFC 6901 JSON pointers and scalar
JSON equality; missing fields differ from explicit null. Delete must confirm
HTTP 404. Create/update cannot complete on a non-success response or HTTP 202.

Defaults are no initial delay, a 1,000 ms interval, 60 attempts and a 120,000 ms
timeout. `delay_ms` and `interval_ms` are bounded to 60,000 ms (interval at least
one); attempts to 1–1,000 and timeout to 1–3,600,000 ms. The context deadline
covers the mutation and waiter; cancellation interrupts requests and waits.
The mutation runs once. An accepted create must return a usable identity; failed
waiters retain recovery state. Delete retains managed state until absence is
confirmed. API response bodies and credentials stay out of polling diagnostics.

Declare HTTP 202 explicitly for an asynchronous mutation and configure polling
for that lifecycle. Wildcard success declarations remain unsupported. Without
polling, declared or unexpected 202 remains an error. Job URLs, arbitrary
follow-up operations, regex expressions and multi-step mutation orchestration are
outside this implementation. See the [Speakeasy comparison](terraform-speakeasy.md).

## Read existing objects through data sources

Set `data_sources: true` on the `provider` plugin, or call native
`terraform::provider().data_sources(true)`. Each validated resource read operation
then also exposes a same-named data source:

```hcl
data "widgets_widget" "existing" {
  id = "remote-widget-id"
}
```

The string `id` is required; supported nested and scalar response fields are computed, with sensitive
flags preserved. Reads use the declared GET operation, authentication and identity
encoding. HTTP 404, malformed bodies and identity mismatches report diagnostics.
Data sources never create/update/delete an object or remove managed resource
state. Independent read-only endpoints, lists and separate polling data sources remain unsupported. See HashiCorp's
[data-source lifecycle](https://developer.hashicorp.com/terraform/plugin/framework/data-sources).

Reserved root names such as `count`, `for_each` and `depends_on` are rejected by
the planner; they must be explicitly remapped in a custom validated catalog rather
than emitted as an invalid Terraform schema.

## Build and inspect the provider

After generation, build with Go and run the emitted native tests:

```sh
cd generated/terraform
go mod tidy
go test ./...
go build ./...
```

Generated `go.mod` and `go.sum` pin the Framework dependency graph; ordinary
`go mod tidy` leaves the generated manifests unchanged. Add new dependencies
through source customization when authoring additional Go modules.

Review the generated README for provider configuration, resource attributes,
import examples and local provider installation. Generation does not install or
publish a Terraform provider. Native tests use deterministic HTTP fixtures and
Framework state objects. An opt-in repository test also builds the provider and
runs real Terraform CLI validation, apply, update, import, no-change plans and
destroy against an ephemeral loopback mock, including a data-source read:

```sh
POOLSTER_TERRAFORM_BIN=/path/to/terraform \
  cargo test -p poolster-plugin-terraform terraform_cli_local_mock_lifecycle -- --ignored
```

This was verified with Terraform 1.13.4 and pinned Framework dependencies. Go
modules must be available in `POOLSTER_TERRAFORM_GOMODCACHE` (default
`/tmp/poolster-tf-mod-cache`); the harness builds offline and uses provider development
overrides, without registry installation/publication. Local mocks verify the
generated boundary, not your live API’s lifecycle semantics. Test real API
behavior deliberately in a sandbox before release.

The existing editable SDK-check action accepts `language: terraform`, sets up Go,
and runs native provider checks. Use it after checking out and generating your
provider; registry signing, release archives and registry publication require a
separate publisher policy.

## Extend the system

The native API supports independent semantic providers and renderer consumers:

```rust
use poolster::{terraform, prelude::*};
let entities = terraform::entities();
let provider = terraform::provider().using_entities(entities.catalog_handle());
let package = terraform::package("terraform")
    .provider_name("widgets")
    .module("example.com/terraform-provider-widgets")
    .with(entities)
    .with(provider);
```

`terraform::analyze` builds an inspectable catalog without rendering files. The
new native `terraform::provider()` path consumes a typed `EntityCatalog`.
Use explicit resource bindings or target-specific `x-poolster-terraform` annotations
when conventional inference does not describe your service. Keep custom Go source
in package-scoped customization files, and use guarded patches for changes to
owned files. Do not edit generated resource files and expect regeneration to
keep those changes automatically.

The older `terraform::sdk().resource(TerraformResource)` API remains a legacy
raw-JSON prototype for source compatibility. It is not the typed provider path
and does not gain the new state guarantees. Migrate its resource mappings to
`ResourceBinding` and `provider()` before using the new recipe target. State upgrades require explicit versioned mappings and validation against old state.

## Prepare a registry release

The opt-in `release-scaffold` plugin emits create-once GoReleaser configuration,
protocol-6 registry metadata, `RELEASING.md` and a workflow template. Specify the
actual registry namespace in the provider configuration:

```json
{
  "language": "terraform",
  "path": "terraform",
  "plugins": [
    {"name":"provider", "provider_name":"widgets", "registry_namespace":"acme"},
    {"name":"release-scaffold"}
  ]
}
```

The namespace is used in the generated provider address and package README. In
the library use `.provider_name("widgets").registry_namespace("acme")` and
`.with(terraform::release_scaffold())`. The provider exposes `main.version` for
release injection. Archives, manifest checksums and detached signing follow the
[Terraform Registry release requirements](https://developer.hashicorp.com/terraform/registry/providers/publishing).

Use a dedicated `terraform-provider-widgets` repository with the package at its
root. Review/copy the workflow template into `.github/workflows` deliberately;
it is not activated by generation. Register the namespace/provider and public
signing key, configure the protected release environment, then validate with
`goreleaser check` and a local unsigned snapshot before a real release. Generated
sources/native Framework checks were verified; GoReleaser signing, GitHub release
upload and registry ingestion remain unexecuted. Existing Terraform state is not
migrated automatically by changing the provider address.

## Local lifecycle verification matrix

The provider tests use disposable generated providers and local services. They do
not publish a provider or exercise a customer's API. The real Terraform CLI probe
`terraform_cli_local_mock_lifecycle` now verifies this scalar bearer-auth resource:

| Lifecycle | Verified behavior |
| --- | --- |
| Create | Apply records the remote ID and exact 64-bit quantity. |
| Refresh | A remote name change gives Terraform plan exit code 2. |
| Failed refresh | HTTP 401 gives a diagnostic and retains the managed resource. |
| Update | PATCH followed by HTTP 204 refresh records the changed name while preserving quantity. |
| Failed update | HTTP 500 fails apply and retains the managed resource for recovery. |
| Import | Removing local state and importing the encoded ID gives a clean plan. |
| Read-only lookup | The data source reads the created resource in the same apply. |
| Delete | Destroy succeeds against the disposable service. |

Run the gated probe with Go, cached Framework dependencies and an explicitly
selected Terraform executable:

```sh
POOLSTER_TERRAFORM_BIN=/path/to/terraform \
POOLSTER_TERRAFORM_GOMODCACHE=/path/to/go/pkg/mod \
cargo test -p poolster-plugin-terraform terraform_cli_local_mock_lifecycle -- --ignored
```

Additional native Framework tests cover missing resources, malformed or incomplete
responses, pending deletes, normalized create values, composite identity, nested
attributes, polling and state upgrades. These direct Framework probes are separate
from Terraform CLI acceptance. The CLI matrix above does not establish full CLI
coverage for every nested shape, authentication scheme, polling policy or migration.

### Additional CLI acceptance profiles

| Profile | Real CLI evidence | Direct Framework evidence beyond CLI |
| --- | --- | --- |
| Scalar + bearer | CRUD, data source, drift, denied reads, failed update, exact int64, import, clean plan | Malformed/null/missing fields, identity changes, create normalization, absent-resource removal |
| Nested, unauthenticated | Object, list of objects, map of objects, string list/map; CRUD, drift, failed reads/updates, import, data source, clean plan | Recursive unknown/null diagnostics and state upgrade field renames |
| Composite identity + bearer + polling | Authenticated async create/update/delete, update timeout, state retention, secret-free diagnostics, timeout recovery, clean plan; mutation sent once per invocation | Terminal errors, cancellation, timeout variants, accepted-status enforcement, composite import validation |
| Basic and API-key header/query/cookie | Native generated transport tests; CLI coverage not yet claimed | Declared credentials applied only to secured operations |

The nested CLI profile is `terraform_cli_nested_local_mock_lifecycle`; the
asynchronous profile is `terraform_cli_polling_local_mock_lifecycle`. Both use the
same executable/cache variables as the scalar command above. Polling tests hold a
remote operation pending deliberately, verify failed apply retains its identity,
and recover through refresh. They verify bearer authentication on the local
mutation endpoint. They do not exercise hosted API credentials or a registry.
