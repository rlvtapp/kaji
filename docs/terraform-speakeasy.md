# Terraform generation: Speakeasy comparison

Reviewed against Speakeasy's public documentation on October 7, 2026. The goal is
reliable Terraform behavior within Kaji's plugin system. Similar feature names do
not imply equivalent coverage or native test evidence.

## What the comparison covers

Speakeasy maps annotated create/read/update/delete operations into resources and
read operations into data sources, and merges their schemas. It supports typed
nested objects, lists/maps, plan validators and plan modifiers. See its
[support matrix](https://www.speakeasy.com/docs/speakeasy-reference/supported/terraform)
and [entity mapping](https://www.speakeasy.com/docs/terraform/customize/entity-mapping).

Composite import IDs use a JSON object containing the required keys, alongside
ordinary single-key import. [Import examples](https://www.speakeasy.com/docs/create-terraform/).
Resource versions start at zero; an explicit version bootstraps editable state
migration functions for breaking type changes. Adding/removing ordinary attributes
does not automatically require a migration. [Resource configuration](https://www.speakeasy.com/docs/terraform/customize-terraform/resource-configuration).

Its asynchronous provisioning can add bounded polling after create/delete, with
success/failure conditions and cancellation-sensitive waits. [Polling guide](https://www.speakeasy.com/docs/terraform/terraform-guides/async-polling).
More advanced mappings can reshape API data and combine lifecycle calls.
[Transformation overview](https://www.speakeasy.com/blog/release-terraform-jq-transformations).

## Kaji's implementation and boundaries

The runnable [provider guide](terraform-provider.md) is authoritative for current
configuration and supported shapes. Kaji keeps native typed provider contracts,
explicit operation bindings, and actionable generation diagnostics. It does not
interpret arbitrary Speakeasy extensions as Kaji configuration.

| Capability | Kaji |
| --- | --- |
| CRUD resource mapping | Explicit bindings and conservative opt-in inference; unsupported lifecycle semantics fail validation |
| Authentication and drift | Supported bearer/basic/API-key schemes; reads refresh state and distinguish missing objects from errors |
| Single-entity data sources | Generated from validated read operations |
| Nested schemas and collections | See the supported-shape section in the provider guide; unions/recursive shapes need explicit support |
| Composite identities/import | See the explicit identity binding in the provider guide |
| Resource state migrations | Explicit versioned upgrades; migration behavior must be tested against old state |
| Plan behavior | Known configured values must agree with final state; unsupported update inputs require replacement |
| Generated docs/release sources | Editable examples and documentation; optional registry/signing scaffold; publication unverified |
| Advanced validators/modifiers | No general Speakeasy-compatible extension or arbitrary executable annotation engine |
| Polling/multiple lifecycle calls | No general asynchronous provisioning or multi-step lifecycle orchestration |
| Data transformations/hoisting | No general jq transformation or entity-hoisting engine |
| Collection data sources/actions | Single-entity data sources only; independent collection data sources and advanced actions remain separate work |
| Write-only arguments | No general Terraform write-only argument/change-trigger generation; `Sensitive` alone does not prevent persistence |

## Evidence required before relying on a feature

Generated Go compilation is the first check. Framework tests then need to exercise
known/unknown/null plan values, request bytes, response-to-state conversion, drift,
missing-object handling and import diagnostics. Nested shapes must preserve wire
names independently of Terraform attribute names, and reject unsupported shapes
before writing files. Migration tests must start from a prior raw-state version
and verify the current schema rather than merely checking emitted source strings.

The real local Terraform CLI lifecycle probe complements Framework object tests.
Neither substitutes for API-specific acceptance tests, real asynchronous service
behavior or registry publication. The [verification guide](verification.md)
records which probes have executed and which await CI.
