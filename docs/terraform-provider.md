# Generate a typed Terraform provider

Kaji's `terraform` plugin generates a Go provider using HashiCorp's Terraform
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
contracts. The generated `.kaji/terraform-plan.json` explains accepted resources
and excluded operations. Examine it even when generation succeeds.

## Understand the supported state behavior

The current implementation handles fixed nested JSON objects, typed lists/maps,
string, boolean, integer and number attributes; single or composite string identity; resource import; ordinary CRUD;
and supported single-scheme HTTP bearer/basic/API-key authentication.
Configured values, computed response fields, identity and replacement decisions
have distinct roles in the plan. Create/update must preserve known planned values;
server normalization that would violate Terraform state consistency is reported.
Read refreshes state and removes a resource on a missing-object response.
Declared `202` or wildcard `2XX` lifecycle success is excluded; unexpected `202`
responses fail without treating pending work as completed. Delete
also treats an already-missing object as success. Error messages avoid dumping
HTTP response bodies or credentials.

Nullable values, unions, recursive shapes, unsupported constraints, differing nested
read/write projections, write-only secrets, polling, independent/list data sources
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
state. Independent read-only endpoints, lists and polling remain unsupported. See HashiCorp's
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
KAJI_TERRAFORM_BIN=/path/to/terraform \
  cargo test -p kaji-plugin-terraform terraform_cli_local_mock_lifecycle -- --ignored
```

This was verified with Terraform 1.13.4 and pinned Framework dependencies. Go
modules must be available in `KAJI_TERRAFORM_GOMODCACHE` (default
`/tmp/kaji-tf-mod-cache`); the harness builds offline and uses provider development
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
use kaji::{terraform, prelude::*};
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
Use explicit resource bindings or target-specific `x-kaji-terraform` annotations
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
