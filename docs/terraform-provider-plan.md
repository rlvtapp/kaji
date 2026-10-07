# Terraform provider generation plan

Status: typed scalar CRUD and opt-in resource-read data sources are available; a real Terraform CLI lifecycle harness passes against a local mock. See [the implementation guide](terraform-provider.md) for its supported subset and actual recipe API. Repository inspection and primary-source research: 7 October 2026. Advanced configuration and Rust structures below remain proposed APIs unless covered by that guide.

## 1. Current architecture and gaps

Kaji already has a workspace Terraform crate, `crates/plugins/terraform`. Its `sdk().resource(...)` API requires explicit create/read/update/delete operation IDs and an identity path parameter. It renders a Go Plugin Framework provider with an `id` and a raw JSON `body`, a base URL, and a hard-coded bearer API key. It does not infer entities, expose data sources/import, model individual attributes, or consume the core security catalog. Create/update currently replace the configured body with the API response; a server-added or normalized field can therefore violate Terraform's planned-value consistency. Read does not remove a missing object from state. This is a prototype to evolve, not a production lifecycle implementation to expose unchanged.

`crates/kaji-core/src/ast.rs` already represents operations, request/response schemas, path/query/header parameters, security requirements, schema constraints, readOnly/writeOnly, composition, and annotations. `openapi/types.go`, `openapi/openapi.go`, and `openapi/schema_walk.go` produce the Go sidecar representation; `crates/kaji-core/src/adapter/openapi_sidecar.rs` converts it. Operation and schema extensions survive today, but parameter extensions, response headers/links/extensions, root metadata, and server provenance need a deliberate preservation audit. Generic metadata gaps should be fixed there; Terraform-specific types should not be added there.

The facade and CLI do not currently expose Terraform as a normal target. The existing plugin engine, typed provider/consumer contracts, generated-file ownership, create-once files, and check mode can support it without adding lifecycle logic to unrelated generators.

## 2. Semantic ownership and proposed IR

Keep the source AST transport-neutral. Put entity interpretation, inference, Terraform schema rules, and lifecycle decisions in `crates/plugins/terraform`. Normalize all supported configuration sources into one immutable, validated plan before rendering. Publish that plan through a typed contract so a custom Terraform renderer, documentation consumer, or test generator can replace the default implementation.

```rust
// Proposed types owned by the Terraform plugin.
struct ProviderPlan {
    entities: Vec<EntityPlan>,
    data_sources: Vec<DataSourcePlan>,
    authentication: AuthenticationPlan,
    diagnostics: Vec<InferenceDiagnostic>,
}
struct EntityPlan {
    key: EntityKey, name: String, schema_version: u64,
    identity: IdentityPlan, attributes: Vec<AttributePlan>,
    lifecycle: LifecyclePlan, relationships: Vec<RelationshipPlan>,
    provenance: Vec<Evidence>,
}
struct LifecyclePlan {
    create: Vec<StepPlan>, read: Vec<StepPlan>,
    update: Option<Vec<StepPlan>>, delete: Vec<StepPlan>,
    timeouts: TimeoutPlan,
}
struct StepPlan {
    operation: OperationId, inputs: Vec<ValueBinding>,
    outputs: Vec<ValueBinding>, success: SuccessPolicy,
    polling: Option<PollPlan>,
}
struct AttributePlan {
    api_paths: OperationFieldBindings, terraform_path: AttributePath,
    ty: TerraformType, presence: PresencePolicy,
    replacement: ReplacementPolicy, sensitivity: SensitivityPolicy,
    validators: Vec<ValidatorPlan>, modifiers: Vec<ModifierReference>,
    update: UpdateFieldPolicy, provenance: Vec<Evidence>,
}
```

Bindings distinguish configuration, planned values, prior state, identity, provider settings, and preceding step outputs. They preserve unknown, null, absent, and known values separately. Identity includes ordered typed components and a versioned import codec. Selectors use the existing neutral selector parser where compatible; Terraform-specific binding validation remains local. Stable entity keys must not depend on generated names or enumeration order.

Proposed contracts: `EntityCatalog`, `TerraformSchemaCatalog`, `TerraformTransport`, and `ProviderArtifact`. A convenience `sdk()` composes the same providers consumed by custom plugins. Do not require a full Go SDK merely to generate a provider. Reuse neutral request/security/schema helpers or a supplied transport adapter with an explicit ABI. Core should never switch on a Terraform target name.

## 3. Conservative inference algorithm

1. Resolve local schema references and classify operation paths into literal segments and typed parameters. Retain source pointers and operation IDs as evidence.
2. Identify collection/item pairs: `POST /projects` and `GET /projects/{project_id}` with compatible JSON object schemas, matching prefix, and an unambiguous durable identity mapping. Parent path parameters must bind consistently across the lifecycle.
3. Match create inputs and read outputs by resolved field identity, explicit selectors, and compatible types. Response envelopes require an unambiguous object selector; multiple successful response shapes must agree or require configuration.
4. Associate item `PATCH` or `PUT` and `DELETE` only when their paths, identity inputs, security compatibility, and entity schema support the same object. An operation ID or tag is supporting evidence, never sufficient proof. Singularizing names is a presentation heuristic, not resource identity.
5. Require create, refresh, and delete before automatically emitting a managed resource. An absent update is valid only with explicit or provable replacement behavior for configurable fields. A read-only item becomes a data source. A collection GET becomes a collection data source, not a managed resource.
6. Infer scalar identity only when create/read supply it and item operations consume it. A string or integer `id` is a candidate, not proof. Client-selected IDs, compound keys, Location headers, eventual IDs, UUID aliases, and response transformations need explicit bindings unless uniquely established by the source.
7. Produce candidate plans and diagnostics. Only fully validated conventional candidates are enabled automatically. An ambiguous candidate is excluded with a concrete suggested override; it must never become a guessed destructive lifecycle.

Do not infer CRUD from RPC names, POST searches, bulk endpoints, action suffixes, upserts, soft deletes, or a delete-looking operation ID alone. Multiple update endpoints, conflicting response unions, missing identity, and incompatible create/read shapes block automatic managed-resource emission. Inference performs no API calls. A user can explicitly accept a candidate, but cannot bypass schema/binding validation accidentally.

## 4. Native extensions and configuration

Use small hints for common cases and structured configuration for complex lifecycles. Existing shorthand requested by the user remains accepted:

```yaml
components:
  schemas:
    Project:
      x-kaji-entity: Project
      properties:
        region:
          type: string
          x-kaji-terraform:
            name: location
            replacement: always
        token:
          type: string
          writeOnly: true
          x-kaji-terraform:
            sensitive: true
paths:
  /projects:
    post:
      x-kaji-entity-operation: Project#create
```

Proposed target configuration, illustrated without claiming current CLI support:

```yaml
terraform:
  module: github.com/acme/terraform-provider-acme
  provider: acme
  inference: conservative
  entities:
    Project:
      name: project
      version: 0
      identity:
        fields: [organization_id, id]
        import: {format: json-array, version: 1}
      lifecycle:
        create:
          - operation: createProject
            outputs: {id: response.body.id}
        read: [{operation: getProject}]
        update:
          - operation: patchProject
            patch: {mode: changed-fields, null: send, absent: omit}
        delete: [{operation: deleteProject}]
      attributes:
        organization_id: {replacement: always}
        password: {write_only: true, change_trigger: password_version}
        password_version: {optional: true}
  exclude_operations: [deleteAllProjects]
```

Precedence: explicit exclusions; target configuration; Kaji extensions; opt-in Speakeasy compatibility translation; conservative inference. Conflicting explicit directives are errors rather than silently ordered guesses. Include an `explain` report with the effective source of each decision. Avoid putting arbitrary executable Go expressions in OpenAPI: custom types, validators, modifiers, and hooks reference typed plugin registrations or generated create-once interfaces. Existing explicit `.resource(...)` callers get a migration adapter with warnings; do not silently reinterpret their raw-body state.

## 5. Schema and advanced attribute semantics

Build a per-operation field matrix before merging into Terraform attributes. Required create inputs generally become Required; response-only/readOnly values become Computed; optional configurable values remain Optional. Optional+Computed needs explicit default/refresh behavior, not automatic assignment to every response field. Conflicting types or requirements need bindings/overrides. API defaults and server normalization must respect known planned values; implement semantic equality only for documented equivalent representations.

Map enums, numeric/string/list bounds and patterns into supported validators, checking RE2 compatibility. Support conflicts, exactly-one, at-least-one, required-with, nested paths, and cross-field plan validators. Validate those constraints together for contradictions. Infer replacement for identity/path-parent fields and fields proven create-only; unresolved mutability is a blocking diagnostic, not a blanket ForceNew guess. Render replacement through Framework plan modifiers, with `UseStateForUnknown` only for values guaranteed stable. Framework requires final state to agree with known planned values. [HashiCorp plan modification](https://developer.hashicorp.com/terraform/plugin/framework/resources/plan-modification).

Treat OpenAPI writeOnly as “not returned by the API,” distinct from Terraform write-only storage. Sensitive suppresses presentation but does not itself prevent persistence. Terraform write-only arguments require Terraform 1.11+, cannot be Computed, and need explicit change triggers because their values are not retained in state. Reject unsupported combinations, including set elements and a naive unconditional replacement modifier. [HashiCorp write-only arguments](https://developer.hashicorp.com/terraform/plugin/framework/resources/write-only-arguments).

Preserve nullable vs omitted update behavior explicitly. PATCH changed-fields mode compares plan to prior state, excluding computed-only fields and preserving deletion/null semantics; PUT normally sends a complete configured representation. JSON Patch, Merge Patch, and custom PATCH media types need separate codecs. Collections default to API-order-preserving lists; infer sets only with a proven stable identity/order-insensitive contract. Arbitrary maps, unions, recursive schemas, semantic JSON strings, and custom types need explicit supported mappings or actionable rejection. Custom naming and ignored API-only fields must detect collisions and ensure omitted fields are not lifecycle-required.

## 6. Resource, data source, import, and state lifecycle

Create builds requests from plan/config, records identity as soon as available, then refreshes computed values while preserving configured planned values. A failed post-create poll must retain recoverable identity rather than repeat an unsafe POST. Read refreshes drift; a documented missing-object response removes managed state, whereas authentication, rate limits, transport errors, and malformed responses produce diagnostics. Update preserves identity and checks state consistency. Delete verifies the configured success condition; a documented already-missing response succeeds. Retry safe operations only under an explicit idempotency policy.

Implement single-object and collection data sources separately, including pagination through the shared normalized pagination plan. Missing data-source results produce errors rather than removing resource state. Do not invent user filters that the server cannot support unless explicitly configured client-side filtering is bounded and documented.

Imports populate only identity components and then refresh. Use a stable, versioned codec for compound IDs; avoid ambiguous delimiter splitting. Required secrets unavailable after import need a documented Optional/write-only recovery policy or an import diagnostic. Resource schema versions and typed state upgraders must be explicit and tested; renames/type changes cannot silently discard old state. [HashiCorp import](https://developer.hashicorp.com/terraform/plugin/framework/resources/import), [state upgrades](https://developer.hashicorp.com/terraform/plugin/framework/resources/state-upgrade).

## 7. Relationships and nested resources

`/organizations/{organization_id}/projects/{project_id}` maps parent identity into a configurable parent attribute and the import identity. Changing it normally replaces the child. A response reference to another entity becomes an ID/reference attribute; ordinary Terraform expressions establish dependencies. Do not automatically create referenced entities or cascade deletes.

Distinguish independently addressable child resources from nested configuration blocks. A nested object is a block/object attribute unless a validated separate lifecycle exists. Multi-step creates can create child objects only through explicit steps and recovery rules. Flattening/hoisting is an explicit projection with collision checks; arrays need stable element identity for mutation. Document partial failure, orphan recovery, and ownership boundaries.

## 8. Authentication and provider configuration

Derive provider settings from the core security catalog, preserving OpenAPI OR alternatives and AND combinations per operation. API keys retain header/query/cookie location; HTTP basic/bearer remain distinct. Choose authentication alternatives explicitly when multiple are available; generate required scopes validation and per-operation credential checks. Never hard-code bearer semantics for all schemes.

Preserve root/path/operation server precedence and server variables generically, then expose a validated base URL/provider setting where appropriate. Environment fallback names and proxy/TLS/timeouts are configurable; unknown configuration during planning must not trigger API calls. Secrets are never embedded in generated fixtures, ownership metadata, diagnostics, or release workflows. OAuth refresh/client credentials and custom signing use pluggable transports with explicit capabilities rather than inferred login flows. Missing security metadata must not imply a guessed public operation.

## 9. Framework and project structure

Continue with Terraform Plugin Framework, the current recommended basis for new providers. Pin and test a supported framework/Go/Terraform compatibility matrix rather than retaining the prototype's version forever. [HashiCorp Framework](https://developer.hashicorp.com/terraform/plugin/framework).

Proposed generated layout:

```text
main.go
internal/provider/provider.go
internal/provider/configuration.go
internal/resource/project/{resource,schema,model,bindings,import,state_upgrade}.go
internal/datasource/project/{data_source,schema,model}.go
internal/api/{transport,operations,auth,errors}.go
internal/shared/{values,diagnostics,polling}.go
internal/custom/                 # create-once hooks, validators, modifiers
internal/provider/*_test.go
examples/{provider,resources,data-sources}/
docs/{index,resources,data-sources}/
go.mod
.kaji/{ownership.json,terraform-plan.json}
```

Generate models using Framework value types, not Go pointers as a proxy for unknown/null. Share request/response codecs where useful, but Terraform state reconciliation is its own implementation. Custom files remain create-once and ownership-aware. Registry metadata/release integration is optional and separate from generation; this plan does not authorize publishing.

## 10. Speakeasy capabilities to account for

Speakeasy maps entities and lifecycle operations explicitly, supports one operation in multiple entities, ordered lifecycle steps, response projections, pagination, polling, PATCH changed-field requests, and nonstandard delete flows. Kaji should translate these into typed bindings/steps rather than duplicate their extension architecture. Complex workflows remain explicit; conventional REST inference is Kaji's additional convenience. [Speakeasy entity mapping](https://www.speakeasy.com/docs/terraform/customize-terraform/entity-mapping).

Its resource customization includes naming, import guidance, state schema versions/upgrades, and preserved custom boilerplate. These belong in Kaji's plan and create-once customization boundaries. [Speakeasy resource configuration](https://www.speakeasy.com/docs/terraform/customize-terraform/resource-configuration).

Property controls include sensitive/write-only fields, ignored properties, semantic/custom types, JSON-string handling, and response filtering. Add explicit projections and semantic type contracts; computed-diff suppression must require a refresh correctness justification. [Speakeasy property customization](https://www.speakeasy.com/docs/terraform/customize-terraform/property-customization).

Dependencies and custom validators/modifiers need typed references, diagnostics, and preserved user implementations. [Speakeasy validation dependencies](https://www.speakeasy.com/docs/terraform/customize-terraform/validation-dependencies), [plan modification](https://www.speakeasy.com/docs/terraform/customize/plan-modification).

Advanced controls include forced optional/read-only/replacement behavior, plan-only update input behavior, and structural Terraform type deduplication. Kaji can model these policies explicitly; deduplication is a renderer optimization and must not erase field-specific validators or documentation. [Speakeasy advanced features](https://www.speakeasy.com/docs/terraform/customize-terraform/advanced-features).

Provider settings require a configurable authentication/server surface beyond an API-key shortcut. [Speakeasy provider configuration](https://www.speakeasy.com/docs/terraform/customize-terraform/provider-configuration).

Later milestones should include explicit asynchronous polling, multi-step workflows, soft delete, upsert semantics, response transforms, collection filtering, custom semantic equality, import examples, and typed state migrations. Ephemeral resources and actions are distinct contracts, not ordinary CRUD resources with a switch. Ephemeral resources require Terraform 1.10+ and must avoid persistent state; actions have a separate invoke lifecycle. Gate compatibility and test each target explicitly. [HashiCorp ephemeral resources](https://developer.hashicorp.com/terraform/plugin/framework/ephemeral-resources), [actions](https://developer.hashicorp.com/terraform/plugin/framework/actions).

## 11. Diagnostics and minimal-configuration experience

Every decision carries source location, operation/schema IDs, evidence, effective directive, and confidence category (`explicit`, `proven-conventional`, `ambiguous`, `unsupported`). Emit a deterministic human report and machine-readable JSON. Example:

```text
Inferred resource project (proven-conventional)
  create POST /projects          operation=createProject
  read   GET /projects/{id}      identity=response.id -> path.id
  update PATCH /projects/{id}    configured fields: name
  delete DELETE /projects/{id}
Blocked resource workspace: two read endpoints match identity
  choose entities.Workspace.lifecycle.read, or x-kaji-entity-operation
Excluded operation deleteAllProjects: bulk destructive operation
```

Clean CRUD APIs should need only provider/module settings. Names, conventional identity bindings, attributes, validators, data-source inputs, and obvious replacement fields can be inferred. Authentication alternatives, async completion, nonstandard identity/import formats, secret change triggers, state migrations, custom hooks, destructive multi-step behavior, and semantic transforms need explicit settings. Strict mode fails on requested unresolved entities; exploratory mode reports excluded candidates. No numerical score should authorize a destructive operation.

## 12. Testing and acceptance criteria

- Semantic fixtures: clean CRUD, nested/composite identity, mixed envelopes, read-only APIs, ambiguous RPC/bulk operations, unions, nullable updates, extensions precedence, incompatible schemas, malformed bindings, and stable naming. Assert plans and evidence, not just generated text.
- Parser fixtures: operation/parameter/root extensions, security OR/AND, servers, response headers/links, source pointers, and overlays survive sidecar conversion without affecting existing generators.
- Go compile/vet tests and goldens for all supported schema shapes; Framework validators/modifiers and custom transport substitution execute.
- Mock HTTP lifecycle tests: create/read/update/delete/import, 404 vs auth errors, drift, null vs omitted PATCH, partial create recovery, retries, pagination, async waits, compound identity, and secret redaction.
- Real Terraform CLI acceptance tests against local deterministic API fixtures: plan/apply/refresh/destroy/import, second-plan no changes, known-plan consistency, replacement, state upgrades, and write-only secret absence. Use `terraform-plugin-testing`, explicit acceptance opt-in, and no live production API by default. [HashiCorp acceptance testing](https://docs.hashicorp.com/terraform/plugin/testing/acceptance-tests).
- Regeneration tests preserve custom hooks, reject edits to generated files, and make `--check` report drift without mutation. Cross-generator regression fixtures ensure Terraform annotations remain irrelevant to SDK outputs.

Do not call schema-sample roundtrips sufficient provider tests: the existing bounded samples API is useful for codec fixtures, but lifecycle, Terraform unknown values, state migrations, and semantic equality require dedicated tests.

## 13. Milestones and exact repository changes

| Milestone | Deliverable and acceptance gate |
|---|---|
| 0: design review | Approve this plan, supported schema subset, extension versioning, and conservative inference behavior before implementation. |
| 1: semantic catalog | Parse hints/config, infer conventional candidates, validate typed plans, emit explain JSON; no provider generation changes until plans are tested. |
| 2: correct conventional CRUD | Typed Framework attributes, transport/auth, import, missing-object refresh, replacement, nullable PATCH, local acceptance tests; migrate explicit prototype API deliberately. |
| 3: composition and CLI | Public providers/contracts and convenience facade, config schema/CLI integration, create-once custom hooks, documentation/examples, ownership/check tests. |
| 4: advanced lifecycle | Composite identity, polling/ordered steps, collection data sources, state migrations, validators/modifiers, semantic types, explicit secret policies. |
| 5: optional parity targets | Ephemeral resources/actions, special upsert/soft-delete projections, compatibility translator, optional registry release scaffolding after dedicated tests. |

Existing files to change when implementation is approved:

| File | Planned responsibility |
|---|---|
| `crates/plugins/terraform/src/lib.rs` | Public provider builders/contracts, convenience API, compatibility adapter. |
| `crates/plugins/terraform/src/package.rs` | Replace prototype rendering with validated-plan orchestration; retain versioned migration path. |
| `crates/plugins/terraform/Cargo.toml` | Dependencies for semantic serialization/testing/shared helpers. |
| `openapi/types.go`, `openapi/openapi.go`, `openapi/schema_walk.go` | Preserve missing generic metadata, source provenance, headers/links/server/extensions; focused compiler tests. |
| `crates/kaji-core/src/adapter/openapi_sidecar.rs`, `crates/kaji-core/src/ast.rs` | Generic metadata conversion only, backward-compatible/defaulted fields; no Terraform entity types. |
| `crates/kaji/Cargo.toml`, `crates/kaji/src/lib.rs` | Optional native Terraform plugin dependency/reexports following existing target conventions. |
| `crates/kaji-cli/src/main.rs` | Terraform config decoding/target selection and explain command integration through plugin APIs. |
| `schemas/v1/kaji.schema.json`, `schemas/kaji.schema.json` | Versioned Terraform target config and extension documentation pointers. |
| Existing workspace manifests/tests | Add dependencies/features only as required; Terraform crate is already a workspace member. |

New Terraform-local modules: `plan.rs`, `extensions.rs`, `inference.rs`, `bindings.rs`, `schema.rs`, `auth.rs`, `diagnostics.rs`, `providers.rs`, `render/{mod,provider,resource,data_source,transport,tests}.rs`, and `tests/{inference,generated_provider,lifecycle}.rs` with fixture directories. Add `docs/terraform-provider.md`, extension reference, migration guide, and examples only during implementation. Core `semantics.rs` changes are justified only for genuinely reusable generic behavior; existing pagination/samples contracts can be consumed unchanged where appropriate.

## 14. Optional neighboring consumer: Postman

A Postman exporter should be a separate plugin consuming neutral operations/security/server metadata and bounded request examples. It should emit collections/environments with secret placeholders, deterministic request naming, response examples, and explicit authentication alternatives. It should not consume Terraform entity lifecycle plans by default or infer destructive collection execution. There is no existing Postman crate in this repository. Shared improvements are source metadata, request bindings/codecs, examples, and authentication representation; Postman-specific output and Terraform-specific state remain independent contracts. A separate detailed Postman plan can define format/version and tests.

## 15. Decisions to settle before implementation

Approve the automatic-emission gate; absence-of-update replacement policy; supported schemas and media types; import codec; state migration compatibility promise; write-only change-trigger syntax; extension compatibility policy; framework/Terraform version matrix; and custom plugin ABI. The recommended first release handles a narrow, correct conventional CRUD subset and explains exclusions. Advanced capabilities enter only with explicit typed semantics and executable state/lifecycle coverage.
