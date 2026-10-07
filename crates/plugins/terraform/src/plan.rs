//! Terraform-specific semantic catalog. No lifecycle inference leaks into core.
use anyhow::{Context, Result, bail, ensure};
use kaji_core::engine::Contract;
use kaji_core::{
    Api, Field, HttpMethod, Operation, SchemaKind, SchemaValue, SecuritySchemeCatalog,
    SecuritySchemeKind,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceBinding {
    pub name: String,
    pub create: String,
    pub read: String,
    #[serde(default)]
    pub update: Option<String>,
    pub delete: String,
    #[serde(default)]
    pub id_parameter: Option<String>,
    #[serde(default)]
    pub id_field: Option<String>,
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub state_upgrades: Vec<StateUpgradeBinding>,
    #[serde(default)]
    pub identity: Vec<IdentityBinding>,
    #[serde(default)]
    pub polling: Option<LifecyclePollingBinding>,
}
/// Explicit bounded lifecycle waiters through the resource's read operation.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecyclePollingBinding {
    #[serde(default)]
    pub create: Option<PollingBinding>,
    #[serde(default)]
    pub update: Option<PollingBinding>,
    #[serde(default)]
    pub delete: Option<PollingBinding>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PollingBinding {
    #[serde(default)]
    pub delay_ms: u64,
    #[serde(default = "default_interval_ms")]
    pub interval_ms: u64,
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    pub success: Vec<PollCriterion>,
    #[serde(default)]
    pub failure: Vec<PollCriterion>,
}
fn default_interval_ms() -> u64 {
    1000
}
fn default_max_attempts() -> u32 {
    60
}
fn default_timeout_ms() -> u64 {
    120000
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum PollCriterion {
    Status {
        status: u16,
    },
    Body {
        pointer: String,
        equals: serde_json::Value,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityBinding {
    pub parameter: String,
    pub field: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateUpgradeBinding {
    pub version: u32,
    pub rename_fields: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntityCatalog {
    pub schema_version: u32,
    pub resources: Vec<ResourcePlan>,
    pub diagnostics: Vec<PlanDiagnostic>,
    pub authentication: AuthenticationPlan,
}
impl EntityCatalog {
    /// Persistable explanation: source examples/extensions and request/response
    /// documents remain in the in-memory contract, not generated bookkeeping.
    pub fn explanation(&self) -> serde_json::Value {
        let operation = |operation: &Operation| {
            serde_json::json!({
                "id": operation.id, "method": operation.method, "path": operation.path
            })
        };
        let resources: Vec<_> = self
            .resources
            .iter()
            .map(|resource| {
                serde_json::json!({
                    "name": resource.name,
                    "create": operation(&resource.create),
                    "read": operation(&resource.read),
                    "update": resource.update.as_ref().map(operation),
                    "delete": operation(&resource.delete),
                    "id_parameter": resource.id_parameter, "id_field": resource.id_field,
                    "attributes": resource.attributes, "requires_auth": resource.requires_auth,
                    "schema_version": resource.schema_version, "state_upgrades": resource.state_upgrades, "identity": resource.identity, "polling": resource.polling
                })
            })
            .collect();
        serde_json::json!({"schema_version": self.schema_version,
            "source_model": "operation_summary",
            "resources": resources, "diagnostics": self.diagnostics,
            "authentication": self.authentication})
    }
}
impl Contract for EntityCatalog {
    const NAME: &'static str = "terraform.entities.v1";
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourcePlan {
    pub name: String,
    pub create: Operation,
    pub read: Operation,
    pub update: Option<Operation>,
    pub delete: Operation,
    pub id_parameter: String,
    pub id_field: String,
    pub attributes: Vec<AttributePlan>,
    pub requires_auth: bool,
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub state_upgrades: Vec<StateUpgradeBinding>,
    #[serde(default)]
    pub identity: Vec<IdentityBinding>,
    #[serde(default)]
    pub polling: Option<LifecyclePollingBinding>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttributePlan {
    pub name: String,
    pub wire_name: String,
    pub ty: ScalarType,
    #[serde(default)]
    pub shape: Option<ShapePlan>,
    pub required: bool,
    pub computed: bool,
    pub optional: bool,
    pub replace_on_change: bool,
    pub sensitive: bool,
    pub update_input: bool,
    pub update_required: bool,
    pub response_required: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ShapePlan {
    Scalar { ty: ScalarType },
    Object { fields: Vec<NestedFieldPlan> },
    List { element: Box<ShapePlan> },
    Map { element: Box<ShapePlan> },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NestedFieldPlan {
    pub name: String,
    pub wire_name: String,
    pub required: bool,
    pub shape: ShapePlan,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalarType {
    String,
    Bool,
    Int64,
    Float64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuthenticationPlan {
    None,
    Bearer,
    Basic,
    ApiKey { name: String, location: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanDiagnostic {
    pub operation: String,
    pub status: String,
    pub reason: String,
}

pub fn analyze(api: &Api, explicit: &[ResourceBinding], infer: bool) -> Result<EntityCatalog> {
    analyze_with_security(api, explicit, infer, None)
}
pub fn analyze_with_security(
    api: &Api,
    explicit: &[ResourceBinding],
    infer: bool,
    security: Option<&SecuritySchemeCatalog>,
) -> Result<EntityCatalog> {
    let mut operation_names = BTreeSet::new();
    for operation in &api.operations {
        ensure!(
            !operation.id.is_empty() && operation_names.insert(operation.id.as_str()),
            "operation IDs must be nonempty and unique"
        );
    }
    let mut resources = Vec::new();
    let mut diagnostics = Vec::new();
    let mut used = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut auth_scheme: Option<String> = None;
    let mut authentication = AuthenticationPlan::None;
    let annotations = annotated_bindings(api)?;
    let mut bindings = explicit.to_vec();
    for binding in annotations {
        if !bindings.iter().any(|b| b.name == binding.name) {
            bindings.push(binding);
        }
    }
    for binding in &bindings {
        let (resource, scheme, auth) = resolve(api, binding, security)
            .with_context(|| format!("Terraform resource {}", binding.name))?;
        validate_auth(&mut auth_scheme, &scheme)?;
        if scheme.is_some() {
            authentication = auth;
        }
        ensure!(
            names.insert(resource.name.clone()),
            "duplicate Terraform resource name {}",
            resource.name
        );
        for id in operation_ids(&resource) {
            ensure!(
                used.insert(id.to_owned()),
                "operation {id} belongs to multiple Terraform resources"
            );
        }
        diagnostics.push(PlanDiagnostic {
            operation: resource.create.id.clone(),
            status: "explicit".into(),
            reason: format!(
                "Validated resource {} with root scalar identity and CRUD bindings",
                resource.name
            ),
        });
        resources.push(resource);
    }
    if infer {
        for create in api
            .operations
            .iter()
            .filter(|op| op.method == HttpMethod::Post)
        {
            if used.contains(&create.id) {
                continue;
            }
            let attempt =
                infer_binding(api, create).and_then(|binding| resolve(api, &binding, security));
            match attempt {
                Ok((resource, scheme, auth)) => {
                    if names.contains(&resource.name)
                        || operation_ids(&resource).iter().any(|id| used.contains(*id))
                    {
                        diagnostics.push(PlanDiagnostic{operation:create.id.clone(),status:"excluded".into(),reason:"candidate overlaps an explicitly mapped resource or inferred name; add an explicit unique binding".into()});
                        continue;
                    }
                    if let Err(error) = validate_auth(&mut auth_scheme, &scheme) {
                        diagnostics.push(PlanDiagnostic {
                            operation: create.id.clone(),
                            status: "excluded".into(),
                            reason: error.to_string(),
                        });
                        continue;
                    }
                    if scheme.is_some() {
                        authentication = auth;
                    }
                    names.insert(resource.name.clone());
                    for id in operation_ids(&resource) {
                        used.insert(id.to_owned());
                    }
                    diagnostics.push(PlanDiagnostic{operation:create.id.clone(),status:"inferred".into(),reason:format!("Conventional resource {}: POST collection, GET/DELETE item, matching string identity and scalar schemas",resource.name)});
                    resources.push(resource);
                }
                Err(error) => diagnostics.push(PlanDiagnostic {
                    operation: create.id.clone(),
                    status: "excluded".into(),
                    reason: error.to_string(),
                }),
            }
        }
    }
    for operation in &api.operations {
        if !used.contains(&operation.id) && !diagnostics.iter().any(|d| d.operation == operation.id)
        {
            diagnostics.push(PlanDiagnostic{operation:operation.id.clone(),status:"excluded".into(),reason:"operation is not part of a validated managed CRUD resource; data sources and actions are not emitted in v1".into()});
        }
    }
    resources.sort_by(|a, b| a.name.cmp(&b.name));
    diagnostics.sort_by(|a, b| a.operation.cmp(&b.operation));
    Ok(EntityCatalog {
        schema_version: 1,
        resources,
        diagnostics,
        authentication,
    })
}
fn operation_ids(resource: &ResourcePlan) -> Vec<&str> {
    let mut ids = vec![
        resource.create.id.as_str(),
        resource.read.id.as_str(),
        resource.delete.id.as_str(),
    ];
    if let Some(update) = &resource.update {
        ids.push(&update.id);
    }
    ids
}
fn validate_auth(current: &mut Option<String>, candidate: &Option<String>) -> Result<()> {
    if let Some(candidate) = candidate {
        ensure!(
            current.as_ref().is_none_or(|current| current == candidate),
            "v1 supports one named security scheme per provider; use separate packages for incompatible schemes"
        );
        *current = Some(candidate.clone());
    }
    Ok(())
}
fn infer_binding(api: &Api, create: &Operation) -> Result<ResourceBinding> {
    ensure!(
        !create.path.contains('{') && create.path.starts_with('/') && !create.path.ends_with('/'),
        "collection create path must have no parent parameters or trailing slash"
    );
    let suffix = create.path.rsplit('/').next().unwrap_or_default();
    ensure!(
        ![
            "search", "query", "batch", "bulk", "actions", "action", "rpc", "create", "upsert",
            "import"
        ]
        .contains(&suffix.to_ascii_lowercase().as_str()),
        "action/bulk/RPC-like collection path requires explicit lifecycle bindings"
    );
    ensure!(
        !create.id.to_ascii_lowercase().contains("upsert")
            && !create.id.to_ascii_lowercase().contains("bulk"),
        "operation identity indicates non-CRUD semantics; explicit bindings required"
    );
    let prefix = format!("{}/{{", create.path);
    let reads: Vec<_> = api
        .operations
        .iter()
        .filter(|op| {
            op.method == HttpMethod::Get
                && op.path.starts_with(&prefix)
                && op.path.ends_with('}')
                && op.path[prefix.len()..].find('/').is_none()
        })
        .collect();
    ensure!(
        reads.len() == 1,
        "collection requires exactly one unambiguous GET item endpoint; found {}",
        reads.len()
    );
    let read = reads[0];
    let parameter = read.path[prefix.len()..read.path.len() - 1].to_owned();
    let matches = |method| {
        api.operations
            .iter()
            .filter(move |op| op.method == method && op.path == read.path)
            .collect::<Vec<_>>()
    };
    let deletes = matches(HttpMethod::Delete);
    ensure!(
        deletes.len() == 1,
        "managed resource requires exactly one DELETE item endpoint"
    );
    let updates: Vec<_> = api
        .operations
        .iter()
        .filter(|op| {
            matches!(op.method, HttpMethod::Put | HttpMethod::Patch) && op.path == read.path
        })
        .collect();
    ensure!(
        updates.len() <= 1,
        "multiple PUT/PATCH updates require explicit binding"
    );
    // Shared named schemas make the entity identity stronger than path singularization.
    let request = body_schema(create)?;
    let response = response_schema(read)?;
    let conventional_name = request
        .kind
        .reference_name()
        .filter(|name| response.kind.reference_name() == Some(*name))
        .unwrap_or(suffix.strip_suffix('s').unwrap_or(suffix));
    let resolved_response = resolve_schema(api, response)?;
    let name = if let Some(entity) = resolved_response.extensions.get("x-kaji-entity") {
        entity
            .as_str()
            .context("v1 x-kaji-entity must name one entity")?
    } else {
        conventional_name
    };
    Ok(ResourceBinding {
        name: attribute_name(name)?,
        create: create.id.clone(),
        read: read.id.clone(),
        update: updates.first().map(|op| op.id.clone()),
        delete: deletes[0].id.clone(),
        id_parameter: Some(parameter),
        id_field: Some("id".into()),
        schema_version: 0,
        state_upgrades: vec![],
        identity: vec![],
        polling: None,
    })
}
fn resolve(
    api: &Api,
    binding: &ResourceBinding,
    security: Option<&SecuritySchemeCatalog>,
) -> Result<(ResourcePlan, Option<String>, AuthenticationPlan)> {
    let find = |id: &str| {
        api.operations
            .iter()
            .find(|op| op.id == id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unknown operation {id}"))
    };
    validate_polling(binding)?;
    for (id, polling) in [
        (
            &binding.create,
            binding.polling.as_ref().and_then(|p| p.create.as_ref()),
        ),
        (&binding.read, None),
        (
            &binding.delete,
            binding.polling.as_ref().and_then(|p| p.delete.as_ref()),
        ),
    ]
    .into_iter()
    .chain(
        binding
            .update
            .as_ref()
            .map(|id| (id, binding.polling.as_ref().and_then(|p| p.update.as_ref()))),
    ) {
        let operation = find(id)?;
        ensure!(
            !operation
                .responses
                .iter()
                .any(|response| response.status.eq_ignore_ascii_case("2XX")
                    || (response.status == "202" && polling.is_none())),
            "HTTP 202 requires explicit lifecycle polling; wildcard 2XX success remains unsupported by polling"
        );
        ensure!(
            operation.path.starts_with('/')
                && !operation.path.starts_with("//")
                && !operation.path.contains(['?', '#', '\\'])
                && !operation.path.chars().any(|c| c.is_control()),
            "lifecycle paths must be literal absolute API paths without query/fragment"
        );
    }
    let create = find(&binding.create)?;
    let read = find(&binding.read)?;
    let delete = find(&binding.delete)?;
    let update = binding.update.as_deref().map(find).transpose()?;
    ensure!(
        create.method == HttpMethod::Post
            && read.method == HttpMethod::Get
            && delete.method == HttpMethod::Delete,
        "create/read/delete must use POST/GET/DELETE"
    );
    ensure!(
        update
            .as_ref()
            .is_none_or(|op| matches!(op.method, HttpMethod::Patch | HttpMethod::Put)),
        "update must use PATCH or PUT"
    );
    ensure!(
        !create.path.contains('{') || !binding.identity.is_empty(),
        "parent/create path parameters require explicit composite identity"
    );
    let id_parameter = binding
        .id_parameter
        .clone()
        .or_else(|| {
            binding
                .identity
                .iter()
                .find(|id| !create.path.contains(&format!("{{{}}}", id.parameter)))
                .map(|id| id.parameter.clone())
        })
        .or_else(|| single_path_parameter(&read.path))
        .context("read requires one explicit identity path parameter")?;
    let id_field = binding
        .id_field
        .clone()
        .or_else(|| {
            binding
                .identity
                .iter()
                .find(|id| !create.path.contains(&format!("{{{}}}", id.parameter)))
                .map(|id| id.field.clone())
        })
        .unwrap_or_else(|| "id".into());
    let composite = !binding.identity.is_empty();
    let identity_parameters: BTreeSet<_> = binding
        .identity
        .iter()
        .map(|id| id.parameter.as_str())
        .collect();
    if composite {
        ensure!(
            binding.identity.len() >= 2
                && binding.identity.len() <= 8
                && identity_parameters.len() == binding.identity.len(),
            "composite identity requires 2..8 unique parameter bindings"
        );
        ensure!(
            binding.id_parameter.is_none() && binding.id_field.is_none(),
            "composite identity cannot combine legacy id mappings"
        );
        let fields: BTreeSet<_> = binding
            .identity
            .iter()
            .map(|id| id.field.as_str())
            .collect();
        ensure!(
            fields.len() == binding.identity.len(),
            "composite identity fields must be unique"
        );
        for id in &binding.identity {
            ensure!(
                attribute_name(&id.parameter).is_ok() && !id.field.is_empty(),
                "invalid composite identity component"
            );
        }
    }
    ensure!(
        !composite
            || binding
                .identity
                .iter()
                .any(|id| !create.path.contains(&format!("{{{}}}", id.parameter))),
        "composite identity requires a server-chosen child component"
    );
    let template = format!("{{{id_parameter}}}");
    for op in [&read, &delete].into_iter().chain(update.as_ref()) {
        ensure!(
            if composite {
                path_parameters(&op.path).len() == identity_parameters.len()
                    && path_parameters(&op.path)
                        .iter()
                        .map(String::as_str)
                        .collect::<BTreeSet<_>>()
                        == identity_parameters
            } else {
                single_path_parameter(&op.path).as_deref() == Some(&id_parameter)
            },
            "item operation {} must have only identity parameter {}",
            op.id,
            id_parameter
        );
        ensure!(
            op.path == read.path,
            "all item lifecycle operations must share a path"
        );
        ensure!(
            op.parameters.iter().all(|p| p.location == "path"
                && if composite {
                    identity_parameters.contains(p.name.as_str())
                } else {
                    p.name == id_parameter
                }),
            "query/header/cookie operation parameters are unsupported in v1"
        );
        if composite {
            ensure!(
                op.parameters.len() == identity_parameters.len()
                    && op.parameters.iter().all(|p| p.required)
                    && op
                        .parameters
                        .iter()
                        .map(|p| p.name.as_str())
                        .collect::<BTreeSet<_>>()
                        == identity_parameters,
                "composite item operations must declare every required string path parameter exactly once"
            );
        }
        for p in &op.parameters {
            ensure!(
                p.schema
                    .as_ref()
                    .is_some_and(|v| scalar(api, v).ok() == Some(ScalarType::String)),
                "identity path parameter must be a string"
            );
        }
        ensure!(
            composite || op.path.ends_with(&template),
            "identity parameter must be the final item path segment"
        );
    }
    ensure!(
        if composite {
            create.parameters.iter().all(|p| {
                p.location == "path"
                    && p.required
                    && identity_parameters.contains(p.name.as_str())
                    && create.path.contains(&format!("{{{}}}", p.name))
            }) && path_parameters(&create.path).len() == create.parameters.len()
                && create
                    .parameters
                    .iter()
                    .map(|p| p.name.as_str())
                    .collect::<BTreeSet<_>>()
                    .len()
                    == create.parameters.len()
        } else {
            create.parameters.is_empty()
        },
        "create operation parameters are unsupported in v1"
    );
    ensure!(
        create
            .request_body
            .as_ref()
            .is_some_and(|body| body.required),
        "optional create request body semantics unsupported in v1"
    );
    let mut create_fields = object_fields(api, body_schema(&create)?)?;
    create_fields.retain(|field| !field.value.read_only);
    for parameter in &create.parameters {
        let component = binding
            .identity
            .iter()
            .find(|id| id.parameter == parameter.name)
            .context("create path identity mapping missing")?;
        let value = parameter
            .schema
            .clone()
            .context("create path parameter schema missing")?;
        ensure!(
            scalar(api, &value)? == ScalarType::String,
            "parent identity must be a string"
        );
        ensure!(
            !create_fields
                .iter()
                .any(|field| field.name == component.field),
            "parent identity cannot also be a body attribute"
        );
        create_fields.push(Field {
            name: component.field.clone(),
            value,
            required: true,
            annotations: BTreeMap::new(),
        });
    }
    let read_fields = object_fields(api, response_schema(&read)?)?;
    let created = object_fields(api, response_schema(&create)?)?;
    let id = read_fields
        .iter()
        .find(|f| f.name == id_field)
        .context("read response must contain identity field")?;
    ensure!(
        id.required && scalar(api, &id.value)? == ScalarType::String,
        "identity must be a required non-null string in read response"
    );
    let created_id = created
        .iter()
        .find(|f| f.name == id_field)
        .context("create response must contain identity field")?;
    ensure!(
        created_id.required && scalar(api, &created_id.value)? == ScalarType::String,
        "create identity must be a required non-null string"
    );
    for component in &binding.identity {
        for fields in [&read_fields, &created] {
            let field = fields
                .iter()
                .find(|f| f.name == component.field)
                .context("composite response identity field missing")?;
            ensure!(
                field.required && scalar(api, &field.value)? == ScalarType::String,
                "composite identities require nonnullable required string response fields"
            );
        }
        ensure!(
            !create_fields.iter().any(|f| f.name == component.field)
                || create
                    .parameters
                    .iter()
                    .any(|p| p.name == component.parameter),
            "client-chosen composite body identity unsupported"
        );
    }
    ensure!(
        !create_fields.iter().any(|f| f.name == id_field),
        "client-chosen create identity is unsupported in v1"
    );
    ensure!(
        read.request_body.is_none() && delete.request_body.is_none(),
        "read/delete request bodies unsupported in v1"
    );
    let mut update_fields = update
        .as_ref()
        .map(|op| object_fields(api, body_schema(op)?))
        .transpose()?
        .unwrap_or_default();
    update_fields.retain(|field| !field.value.read_only);
    ensure!(
        update_fields
            .iter()
            .all(|f| create_fields.iter().any(|c| c.name == f.name)),
        "update-only configurable fields are unsupported in v1"
    );
    let mut attributes = Vec::new();
    let mut names = BTreeSet::new();
    let mut go_names = BTreeSet::from(["ID".to_owned()]);
    for read_field in &read_fields {
        ensure!(
            !read_field.value.write_only,
            "write-only fields need explicit secret/import semantics, unsupported in v1"
        );
        let shape = field_shape(api, &read_field.value)?;
        let ty = match shape {
            ShapePlan::Scalar { ty } => ty,
            _ => ScalarType::String,
        };
        let create_field = create_fields.iter().find(|f| f.name == read_field.name);
        let update_field = update_fields.iter().find(|f| f.name == read_field.name);
        if let Some(field) = create_field {
            ensure!(
                !field.value.read_only
                    && !field.value.write_only
                    && (!read_field.value.read_only
                        || binding.identity.iter().any(|id| id.field == field.name
                            && create.parameters.iter().any(|p| p.name == id.parameter))),
                "conflicting read-only/write-only configurable field {}",
                field.name
            );
            ensure!(
                field_shape(api, &field.value)? == shape,
                "create/read field {} types differ",
                field.name
            );
            let response_field = created
                .iter()
                .find(|f| f.name == field.name)
                .context("create response must include configurable fields for state validation")?;
            ensure!(
                field_shape(api, &response_field.value)? == shape,
                "create response field {} type differs",
                field.name
            );
        }
        if let Some(field) = update_field {
            ensure!(
                field_shape(api, &field.value)? == shape,
                "update/read field {} types differ",
                field.name
            );
        }
        let name = if read_field.name == id_field {
            "id".into()
        } else {
            attribute_name(&read_field.name)?
        };
        ensure!(
            !matches!(
                name.as_str(),
                "count"
                    | "depends_on"
                    | "for_each"
                    | "provider"
                    | "lifecycle"
                    | "connection"
                    | "provisioner"
            ),
            "Terraform reserved root attribute {name} requires an explicit schema mapping"
        );
        ensure!(
            names.insert(name.clone()),
            "Terraform attribute name collision {name}"
        );
        if name != "id" {
            let go_name: String = name
                .split('_')
                .filter(|part| !part.is_empty())
                .map(|part| {
                    let mut chars = part.chars();
                    chars.next().unwrap().to_ascii_uppercase().to_string() + chars.as_str()
                })
                .collect();
            ensure!(
                go_names.insert(go_name),
                "native Go field name collision for Terraform attribute {name}"
            );
        }
        let mut forced = false;
        let mut sensitive = None;
        for field in [Some(read_field), create_field, update_field]
            .into_iter()
            .flatten()
        {
            if let Some(policy) = field
                .value
                .extensions
                .get("x-kaji-terraform")
                .or_else(|| field.annotations.get("x-kaji-terraform"))
            {
                let policy = policy
                    .as_object()
                    .context("x-kaji-terraform must be an object")?;
                ensure!(
                    policy
                        .keys()
                        .all(|key| matches!(key.as_str(), "replacement" | "sensitive")),
                    "v1 x-kaji-terraform supports only replacement and sensitive; other controls need implementation"
                );
                if let Some(replacement) = policy.get("replacement") {
                    ensure!(
                        replacement.as_str() == Some("always"),
                        "v1 replacement override supports only always"
                    );
                    forced = true;
                }
                if let Some(value) = policy.get("sensitive") {
                    let value = value.as_bool().context("sensitive must be a boolean")?;
                    ensure!(
                        sensitive.is_none_or(|previous| previous == value),
                        "conflicting sensitive directives across lifecycle schemas"
                    );
                    sensitive = Some(value);
                }
            }
        }
        let parent_identity = binding.identity.iter().any(|id| {
            id.field == read_field.name && create.parameters.iter().any(|p| p.name == id.parameter)
        });
        attributes.push(AttributePlan {
            name,
            wire_name: read_field.name.clone(),
            ty,
            shape: if matches!(shape, ShapePlan::Scalar { .. }) {
                None
            } else {
                Some(shape)
            },
            required: create_field.is_some_and(|f| f.required),
            optional: create_field.is_some_and(|f| !f.required),
            computed: create_field.is_none_or(|f| !f.required),
            replace_on_change: parent_identity
                || create_field.is_some() && (update_field.is_none() || forced),
            sensitive: sensitive.unwrap_or(false),
            update_input: !parent_identity && update_field.is_some(),
            update_required: !parent_identity && update_field.is_some_and(|field| field.required),
            response_required: read_field.required,
        });
    }
    for field in &create_fields {
        ensure!(
            read_fields.iter().any(|f| f.name == field.name),
            "create-only field {} cannot be refreshed/imported; unsupported in v1",
            field.name
        );
    }
    attributes.sort_by(|a, b| a.name.cmp(&b.name));
    let operations: Vec<_> = [&create, &read, &delete]
        .into_iter()
        .chain(update.as_ref())
        .collect();
    let scheme = security_name(operations[0])?;
    for op in operations {
        ensure!(
            security_name(op)? == scheme,
            "mixed lifecycle authentication requires explicit advanced policy, unsupported in v1"
        );
    }
    let auth = match scheme.as_deref() {
        None => AuthenticationPlan::None,
        Some(name) => authentication_plan(name, security)?,
    };
    let plan = ResourcePlan {
        name: attribute_name(&binding.name)?,
        create,
        read,
        update,
        delete,
        id_parameter,
        id_field,
        attributes,
        requires_auth: scheme.is_some(),
        identity: binding.identity.clone(),
        schema_version: binding.schema_version,
        state_upgrades: binding.state_upgrades.clone(),
        polling: binding.polling.clone(),
    };
    validate_migrations(&plan)?;
    Ok((plan, scheme, auth))
}
fn validate_polling(binding: &ResourceBinding) -> Result<()> {
    let Some(polling) = &binding.polling else {
        return Ok(());
    };
    ensure!(
        polling.create.is_some() || polling.update.is_some() || polling.delete.is_some(),
        "polling must configure at least one lifecycle"
    );
    ensure!(
        polling.update.is_none() || binding.update.is_some(),
        "update polling requires an update operation"
    );
    for waiter in [&polling.create, &polling.update].into_iter().flatten() {
        ensure!(waiter.success.iter().all(|criterion| !matches!(criterion, PollCriterion::Status {status} if !(200..=299).contains(status) || *status == 202)), "create/update polling success status must be completed HTTP 2xx");
    }
    if let Some(waiter) = &polling.delete {
        ensure!(
            waiter
                .success
                .iter()
                .any(|criterion| matches!(criterion, PollCriterion::Status { status: 404 })),
            "delete polling success requires HTTP 404 absence"
        );
    }
    for waiter in [&polling.create, &polling.update, &polling.delete]
        .into_iter()
        .flatten()
    {
        ensure!(
            waiter.delay_ms <= 60000
                && (1..=60000).contains(&waiter.interval_ms)
                && (1..=1000).contains(&waiter.max_attempts)
                && (1..=3600000).contains(&waiter.timeout_ms),
            "polling delay/interval/attempts/timeout exceeds supported bounds"
        );
        ensure!(
            !waiter.success.is_empty(),
            "polling requires nonempty success criteria"
        );
        validate_criteria(&waiter.success)?;
        validate_criteria(&waiter.failure)?;
        if !waiter.failure.is_empty() {
            let success: BTreeMap<_, _> = waiter.success.iter().map(criterion_key_value).collect();
            let failure: BTreeMap<_, _> = waiter.failure.iter().map(criterion_key_value).collect();
            ensure!(
                success != failure,
                "polling success and failure criteria cannot be identical"
            );
        }
    }
    Ok(())
}
fn criterion_key_value(criterion: &PollCriterion) -> (String, serde_json::Value) {
    match criterion {
        PollCriterion::Status { status } => ("status".into(), serde_json::json!(status)),
        PollCriterion::Body { pointer, equals } => (format!("body:{pointer}"), equals.clone()),
    }
}
fn validate_criteria(criteria: &[PollCriterion]) -> Result<()> {
    ensure!(
        criteria.len() <= 32,
        "polling supports at most 32 criteria per group"
    );
    let mut values = BTreeMap::new();
    for criterion in criteria {
        match criterion {
            PollCriterion::Status { status } => ensure!(
                (100..=599).contains(status),
                "polling status must be an HTTP status"
            ),
            PollCriterion::Body { pointer, equals } => {
                ensure!(
                    !equals.is_array() && !equals.is_object(),
                    "polling body equality supports scalar values only"
                );
                ensure!(
                    pointer.is_empty() || pointer.starts_with('/'),
                    "polling pointer must be RFC6901"
                );
                let mut chars = pointer.chars();
                while let Some(character) = chars.next() {
                    if character == '~' {
                        ensure!(
                            matches!(chars.next(), Some('0' | '1')),
                            "polling pointer has invalid RFC6901 escape"
                        );
                    }
                }
            }
        }
        let (key, value) = criterion_key_value(criterion);
        if let Some(previous) = values.insert(key, value.clone()) {
            ensure!(previous == value, "polling criteria are contradictory");
        }
    }
    Ok(())
}
fn path_parameters(path: &str) -> Vec<String> {
    path.split('{')
        .skip(1)
        .filter_map(|part| part.split_once('}').map(|(name, _)| name.to_owned()))
        .collect()
}
fn validate_migrations(plan: &ResourcePlan) -> Result<()> {
    ensure!(
        plan.schema_version <= 16,
        "schema version exceeds supported migration bound (16)"
    );
    let names: BTreeSet<_> = plan
        .attributes
        .iter()
        .map(|a| a.name.as_str())
        .chain(["id"])
        .collect();
    let mut versions = BTreeSet::new();
    for upgrade in &plan.state_upgrades {
        ensure!(
            upgrade.version < plan.schema_version && versions.insert(upgrade.version),
            "migration versions must be unique and older than current schema"
        );
        ensure!(
            !upgrade.rename_fields.is_empty(),
            "migration requires explicit root field renames"
        );
        let mut targets = BTreeSet::new();
        for (from, to) in &upgrade.rename_fields {
            ensure!(
                attribute_name(from)? == *from && from != "id" && !names.contains(from.as_str()),
                "migration source must be a removed root attribute; identity renames unsupported"
            );
            ensure!(
                names.contains(to.as_str()) && to != "id" && targets.insert(to),
                "migration target must be a unique current root attribute"
            );
        }
    }
    ensure!(
        versions.len() == plan.schema_version as usize,
        "every prior schema version must have an explicit direct-to-current state upgrade"
    );
    Ok(())
}
fn security_name(op: &Operation) -> Result<Option<String>> {
    if op.security.is_empty() {
        return Ok(None);
    }
    ensure!(
        op.security.len() == 1 && op.security[0].schemes.len() <= 1,
        "OR/AND security alternatives require explicit policy, unsupported in v1"
    );
    if let Some((name, scopes)) = op.security[0].schemes.iter().next() {
        ensure!(
            scopes.is_empty(),
            "OAuth scope requirements unsupported in v1"
        );
        Ok(Some(name.clone()))
    } else {
        Ok(None)
    }
}
fn authentication_plan(
    name: &str,
    catalog: Option<&SecuritySchemeCatalog>,
) -> Result<AuthenticationPlan> {
    let scheme = catalog
        .and_then(|c| c.schemes.iter().find(|s| s.name == name))
        .context("named security scheme catalog is required")?;
    match &scheme.kind {
        SecuritySchemeKind::Http {
            scheme: Some(scheme),
            ..
        } if scheme.eq_ignore_ascii_case("bearer") => Ok(AuthenticationPlan::Bearer),
        SecuritySchemeKind::Http {
            scheme: Some(scheme),
            ..
        } if scheme.eq_ignore_ascii_case("basic") => Ok(AuthenticationPlan::Basic),
        SecuritySchemeKind::ApiKey {
            name: Some(name),
            location: Some(location),
        } if ["header", "query", "cookie"].contains(&location.as_str()) => {
            Ok(AuthenticationPlan::ApiKey {
                name: name.clone(),
                location: location.clone(),
            })
        }
        _ => bail!(
            "security scheme {name} is unsupported in Terraform v1; supported: HTTP basic/bearer and API keys"
        ),
    }
}
fn single_path_parameter(path: &str) -> Option<String> {
    let start = path.find('{')?;
    let end = path.find('}')?;
    if end <= start + 1 || path[start + 1..].contains('{') || path[end + 1..].contains('}') {
        return None;
    }
    Some(path[start + 1..end].into())
}
fn body_schema(operation: &Operation) -> Result<&SchemaValue> {
    let body = operation
        .request_body
        .as_ref()
        .context("JSON request body is required")?;
    ensure!(
        body.media_types.len() == 1 && body.media_types[0].content_type == "application/json",
        "exactly one application/json request body is supported"
    );
    body.media_types[0]
        .schema
        .as_ref()
        .context("request body schema is required")
}
fn response_schema(operation: &Operation) -> Result<&SchemaValue> {
    let responses: Vec<_> = operation
        .responses
        .iter()
        .filter(|r| r.status.starts_with('2'))
        .collect();
    ensure!(
        responses.len() == 1,
        "exactly one declared success response is required"
    );
    let response = responses[0];
    ensure!(
        response.media_types.len() == 1
            && response.media_types[0].content_type == "application/json",
        "one application/json success response is required"
    );
    response.media_types[0]
        .schema
        .as_ref()
        .context("response schema is required")
}
fn resolve_schema<'a>(api: &'a Api, mut value: &'a SchemaValue) -> Result<&'a SchemaValue> {
    let mut seen = BTreeSet::new();
    while let SchemaKind::Reference { reference } = &value.kind {
        unsupported_validation(value)?;
        ensure!(
            reference.starts_with("#/components/schemas/") || !reference.contains('/'),
            "external schema references unsupported in v1"
        );
        ensure!(
            seen.insert(reference.clone()),
            "recursive reference unsupported in v1"
        );
        let name = value.kind.reference_name().unwrap();
        value = &api
            .schemas
            .iter()
            .find(|s| s.name == name)
            .context("schema reference is missing")?
            .value;
    }
    Ok(value)
}
fn unsupported_validation(value: &SchemaValue) -> Result<()> {
    ensure!(
        value.constraints.is_empty()
            && value.enum_values.is_empty()
            && value.const_value.is_none()
            && value.default.is_none(),
        "constraints, enums, constants and schema defaults require validators/modifiers, unsupported in Terraform v1"
    );
    Ok(())
}
fn object_fields(api: &Api, value: &SchemaValue) -> Result<Vec<Field>> {
    unsupported_validation(value)?;
    let value = resolve_schema(api, value)?;
    unsupported_validation(value)?;
    ensure!(
        !value.nullable && !value.nullish,
        "nullable resource body unsupported in v1"
    );
    let SchemaKind::Object {
        fields,
        additional_properties,
    } = &value.kind
    else {
        bail!("root object schemas are required; envelopes/arrays/unions unsupported in v1")
    };
    ensure!(
        matches!(
            additional_properties,
            kaji_core::AdditionalProperties::Unspecified
                | kaji_core::AdditionalProperties::Forbidden
        ),
        "explicit additional properties unsupported in v1"
    );
    let mut names = BTreeSet::new();
    for field in fields {
        ensure!(
            names.insert(&field.name),
            "duplicate JSON field {}",
            field.name
        );
    }
    Ok(fields.clone())
}
fn field_shape(api: &Api, value: &SchemaValue) -> Result<ShapePlan> {
    fn walk(
        api: &Api,
        value: &SchemaValue,
        depth: usize,
        seen: &mut BTreeSet<String>,
    ) -> Result<ShapePlan> {
        ensure!(depth < 12, "nested Terraform schema exceeds depth bound");
        unsupported_validation(value)?;
        ensure!(
            !value.nullable && !value.nullish && !value.optional,
            "nullable/optional nested value wrappers require explicit policy"
        );
        ensure!(
            !value.read_only && !value.write_only,
            "nested readOnly/writeOnly fields require lifecycle projection"
        );
        if let SchemaKind::Reference { reference } = &value.kind {
            ensure!(
                reference.starts_with("#/components/schemas/") || !reference.contains('/'),
                "external nested reference unsupported"
            );
            ensure!(
                seen.insert(reference.clone()),
                "recursive nested reference unsupported"
            );
            let name = value.kind.reference_name().unwrap();
            let shape = walk(
                api,
                &api.schemas
                    .iter()
                    .find(|schema| schema.name == name)
                    .context("missing nested schema reference")?
                    .value,
                depth + 1,
                seen,
            )?;
            seen.remove(reference);
            return Ok(shape);
        }
        Ok(match &value.kind {
            SchemaKind::String => {
                ensure!(
                    value.format.is_none(),
                    "nested string format requires validators"
                );
                ShapePlan::Scalar {
                    ty: ScalarType::String,
                }
            }
            SchemaKind::Boolean => ShapePlan::Scalar {
                ty: ScalarType::Bool,
            },
            SchemaKind::Integer => {
                ensure!(
                    value
                        .format
                        .as_deref()
                        .is_none_or(|format| format == "int64"),
                    "nested integer format unsupported"
                );
                ShapePlan::Scalar {
                    ty: ScalarType::Int64,
                }
            }
            SchemaKind::Number => {
                ensure!(value.format.is_none(), "nested number format unsupported");
                ShapePlan::Scalar {
                    ty: ScalarType::Float64,
                }
            }
            SchemaKind::Array { items } => ShapePlan::List {
                element: Box::new(walk(api, items, depth + 1, seen)?),
            },
            SchemaKind::Object {
                fields,
                additional_properties,
            } => {
                if fields.is_empty() {
                    if let kaji_core::AdditionalProperties::Schema { value } = additional_properties
                    {
                        return Ok(ShapePlan::Map {
                            element: Box::new(walk(api, value, depth + 1, seen)?),
                        });
                    }
                }
                ensure!(
                    matches!(
                        additional_properties,
                        kaji_core::AdditionalProperties::Forbidden
                    ),
                    "nested objects require fixed properties or typed additionalProperties-only maps"
                );
                ensure!(fields.len() <= 128, "nested object exceeds property bound");
                let mut names = BTreeSet::new();
                let mut planned = vec![];
                for field in fields {
                    let name = attribute_name(&field.name)?;
                    ensure!(
                        names.insert(name.clone()),
                        "nested Terraform attribute name collision"
                    );
                    planned.push(NestedFieldPlan {
                        name,
                        wire_name: field.name.clone(),
                        required: field.required,
                        shape: walk(api, &field.value, depth + 1, seen)?,
                    });
                }
                ShapePlan::Object { fields: planned }
            }
            _ => bail!("nested unions/unknown schemas require explicit policy"),
        })
    }
    // Preserve the established scalar policy, including read-only output fields.
    if scalar(api, value).is_ok() {
        return scalar(api, value).map(|ty| ShapePlan::Scalar { ty });
    }
    walk(api, value, 0, &mut BTreeSet::new())
}
fn scalar(api: &Api, value: &SchemaValue) -> Result<ScalarType> {
    unsupported_validation(value)?;
    ensure!(
        !value.optional,
        "optional scalar wrappers require field-presence policy, unsupported in v1"
    );
    ensure!(
        !value.nullable && !value.nullish,
        "nullable scalar semantics require explicit policy, unsupported in v1"
    );
    let value = resolve_schema(api, value)?;
    unsupported_validation(value)?;
    ensure!(
        !value.nullable && !value.nullish,
        "nullable referenced scalar unsupported in v1"
    );
    match value.kind {
        SchemaKind::String => Ok(ScalarType::String),
        SchemaKind::Boolean => Ok(ScalarType::Bool),
        SchemaKind::Integer => Ok(ScalarType::Int64),
        SchemaKind::Number => Ok(ScalarType::Float64),
        _ => bail!("only string/bool/int64/float64 resource fields supported in v1"),
    }
}
pub(crate) fn attribute_name(value: &str) -> Result<String> {
    let mut result = String::new();
    let mut prev_lower = false;
    for c in value.chars() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase() && prev_lower {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
            prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        } else if c == '_' || c == '-' {
            if !result.ends_with('_') {
                result.push('_');
            }
            prev_lower = false;
        } else {
            bail!("Terraform names must use ASCII letters/digits/underscore/hyphen");
        }
    }
    ensure!(
        result
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase()),
        "Terraform name must start with a letter"
    );
    Ok(result.trim_end_matches('_').into())
}
fn annotated_bindings(api: &Api) -> Result<Vec<ResourceBinding>> {
    let mut entities: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for op in &api.operations {
        if let Some(value) = op.annotations.get("x-kaji-entity-operation") {
            let text = value
                .as_str()
                .context("x-kaji-entity-operation must be Entity#lifecycle string")?;
            let parts: Vec<_> = text.split('#').collect();
            ensure!(
                parts.len() == 2 && ["create", "read", "update", "delete"].contains(&parts[1]),
                "entity-operation must be Entity#create/read/update/delete"
            );
            ensure!(
                entities
                    .entry(parts[0].into())
                    .or_default()
                    .insert(parts[1].into(), op.id.clone())
                    .is_none(),
                "duplicate lifecycle annotation for {}",
                parts[0]
            );
        }
    }
    entities
        .into_iter()
        .map(|(name, mut operations)| {
            let create = operations
                .remove("create")
                .context("annotated entity is missing create")?;
            let read = operations
                .remove("read")
                .context("annotated entity is missing read")?;
            let delete = operations
                .remove("delete")
                .context("annotated entity is missing delete")?;
            Ok(ResourceBinding {
                name,
                create,
                read,
                delete,
                update: operations.remove("update"),
                id_parameter: None,
                id_field: None,
                schema_version: 0,
                state_upgrades: vec![],
                identity: vec![],
                polling: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaji_core::{
        AdditionalProperties, OperationParameter, OperationRequestBody, OperationResponse, Schema,
        SecurityRequirement, SecurityScheme,
    };
    fn field(name: &str, kind: SchemaKind, required: bool) -> Field {
        Field {
            name: name.into(),
            value: SchemaValue::new(kind),
            required,
            annotations: Default::default(),
        }
    }
    fn object(fields: Vec<Field>) -> SchemaValue {
        SchemaValue::new(SchemaKind::Object {
            fields,
            additional_properties: AdditionalProperties::Forbidden,
        })
    }
    pub(crate) fn fixture() -> Api {
        let mut id = field("id", SchemaKind::String, true);
        id.value.read_only = true;
        let input = object(vec![
            field("name", SchemaKind::String, true),
            field("enabled", SchemaKind::Boolean, true),
            field("quantity", SchemaKind::Integer, false),
            field("ratio", SchemaKind::Number, false),
        ]);
        let output = object(vec![
            id,
            field("name", SchemaKind::String, true),
            field("enabled", SchemaKind::Boolean, true),
            field("quantity", SchemaKind::Integer, false),
            field("ratio", SchemaKind::Number, false),
        ]);
        let mut create = Operation {
            id: "createProject".into(),
            method: HttpMethod::Post,
            path: "/projects".into(),
            ..Default::default()
        };
        create.request_body = Some(OperationRequestBody::json(input, true));
        create.responses = vec![OperationResponse::json("201", output.clone())];
        let param = OperationParameter {
            name: "projectId".into(),
            location: "path".into(),
            required: true,
            schema: Some(SchemaValue::new(SchemaKind::String)),
            description: None,
            annotations: Default::default(),
        };
        let read = Operation {
            id: "getProject".into(),
            method: HttpMethod::Get,
            path: "/projects/{projectId}".into(),
            parameters: vec![param.clone()],
            responses: vec![OperationResponse::json("200", output)],
            ..Default::default()
        };
        let update = Operation {
            id: "patchProject".into(),
            method: HttpMethod::Patch,
            path: read.path.clone(),
            parameters: vec![param.clone()],
            request_body: Some(OperationRequestBody::json(
                object(vec![
                    field("name", SchemaKind::String, false),
                    field("enabled", SchemaKind::Boolean, false),
                ]),
                true,
            )),
            ..Default::default()
        };
        let delete = Operation {
            id: "deleteProject".into(),
            method: HttpMethod::Delete,
            path: read.path.clone(),
            parameters: vec![param],
            ..Default::default()
        };
        Api {
            name: "acme".into(),
            version: "1.0.0".into(),
            operations: vec![create, read, update, delete],
            ..Default::default()
        }
    }
    fn binding() -> ResourceBinding {
        ResourceBinding {
            name: "project".into(),
            create: "createProject".into(),
            read: "getProject".into(),
            update: Some("patchProject".into()),
            delete: "deleteProject".into(),
            id_parameter: Some("projectId".into()),
            id_field: None,
            schema_version: 0,
            state_upgrades: vec![],
            identity: vec![],
            polling: None,
        }
    }
    #[test]
    fn infers_typed_crud_with_evidence_and_operation_specific_inputs() {
        let catalog = analyze(&fixture(), &[], true).unwrap();
        assert_eq!(catalog.resources.len(), 1);
        let resource = &catalog.resources[0];
        assert_eq!(resource.name, "project");
        assert_eq!(resource.id_parameter, "projectId");
        let name = resource
            .attributes
            .iter()
            .find(|a| a.name == "name")
            .unwrap();
        assert!(name.required && name.update_input && !name.replace_on_change);
        let count = resource
            .attributes
            .iter()
            .find(|a| a.name == "quantity")
            .unwrap();
        assert!(count.optional && count.computed && count.replace_on_change && !count.update_input);
        assert_eq!(count.ty, ScalarType::Int64);
        assert_eq!(catalog.diagnostics[0].status, "inferred");
    }
    #[test]
    fn ambiguous_missing_delete_parent_paths_and_nested_shapes_are_excluded() {
        let mut api = fixture();
        api.operations[2].method = HttpMethod::Put;
        let mut update = api.operations[2].clone();
        update.method = HttpMethod::Patch;
        update.id = "otherUpdate".into();
        api.operations.push(update);
        assert!(analyze(&api, &[], true).unwrap().resources.is_empty());
        assert!(analyze(&api, &[binding()], false).is_ok()); // Explicit update disambiguates.
        let mut api = fixture();
        api.operations.retain(|op| op.method != HttpMethod::Delete);
        assert!(analyze(&api, &[], true).unwrap().resources.is_empty());
        let mut api = fixture();
        for op in &mut api.operations {
            op.path = format!("/orgs/{{orgId}}{}", op.path);
        }
        assert!(analyze(&api, &[], true).unwrap().resources.is_empty());
        let mut api = fixture();
        api.operations[0].request_body = Some(OperationRequestBody::json(
            object(vec![field(
                "nested",
                SchemaKind::Object {
                    fields: vec![],
                    additional_properties: AdditionalProperties::Forbidden,
                },
                true,
            )]),
            true,
        ));
        assert!(analyze(&api, &[], true).unwrap().resources.is_empty());
    }
    #[test]
    fn constraints_defaults_enums_null_and_reference_wrappers_fail_closed() {
        for mode in 0..5 {
            let mut api = fixture();
            let request = api.operations[0].request_body.as_mut().unwrap().media_types[0]
                .schema
                .as_mut()
                .unwrap();
            let SchemaKind::Object { fields, .. } = &mut request.kind else {
                unreachable!()
            };
            match mode {
                0 => {
                    fields[0]
                        .value
                        .constraints
                        .insert("minLength".into(), serde_json::json!(1));
                }
                1 => fields[0].value.default = Some(serde_json::json!("a")),
                2 => fields[0].value.enum_values = vec![serde_json::json!("a")],
                3 => fields[0].value.nullable = true,
                _ => {
                    let mut wrapped = SchemaValue::reference("Label");
                    wrapped.const_value = Some(serde_json::json!("a"));
                    fields[0].value = wrapped;
                    api.schemas
                        .push(Schema::new("Label", SchemaValue::new(SchemaKind::String)));
                }
            }
            assert!(analyze(&api, &[binding()], false).is_err());
        }
    }
    #[test]
    fn shared_entity_readonly_identity_is_not_a_create_argument() {
        let mut api = fixture();
        let output = api.operations[1].responses[0].media_types[0]
            .schema
            .clone()
            .unwrap();
        api.operations[0].request_body = Some(OperationRequestBody::json(output, true));
        let catalog = analyze(&api, &[], true).unwrap();
        assert_eq!(catalog.resources.len(), 1);
        assert!(
            !catalog.resources[0]
                .attributes
                .iter()
                .find(|a| a.name == "id")
                .unwrap()
                .required
        );
    }
    #[test]
    fn explicit_annotations_and_absent_updates_preserve_replacement_semantics() {
        let mut api = fixture();
        api.operations.retain(|op| op.method != HttpMethod::Patch);
        for op in &mut api.operations {
            let lifecycle = match op.method {
                HttpMethod::Post => "create",
                HttpMethod::Get => "read",
                _ => "delete",
            };
            op.annotations.insert(
                "x-kaji-entity-operation".into(),
                serde_json::json!(format!("Project#{lifecycle}")),
            );
        }
        let catalog = analyze(&api, &[], false).unwrap();
        assert_eq!(catalog.resources.len(), 1);
        assert!(
            catalog.resources[0]
                .attributes
                .iter()
                .filter(|a| a.required || a.optional)
                .all(|a| a.replace_on_change)
        );
        let mut custom = binding();
        custom.name = "Project".into();
        custom.update = None;
        assert_eq!(analyze(&api, &[custom], false).unwrap().resources.len(), 1);
    }
    #[test]
    fn named_security_is_resolved_without_guessing_bearer_or_ands() {
        let mut api = fixture();
        for op in &mut api.operations {
            op.security = vec![SecurityRequirement {
                schemes: BTreeMap::from([("Key".into(), vec![])]),
            }];
        }
        assert!(analyze(&api, &[binding()], false).is_err());
        let catalog = SecuritySchemeCatalog {
            schemes: vec![SecurityScheme {
                name: "Key".into(),
                description: None,
                kind: SecuritySchemeKind::ApiKey {
                    name: Some("X-Key".into()),
                    location: Some("header".into()),
                },
            }],
        };
        let plan = analyze_with_security(&api, &[], true, Some(&catalog)).unwrap();
        assert_eq!(
            plan.authentication,
            AuthenticationPlan::ApiKey {
                name: "X-Key".into(),
                location: "header".into()
            }
        );
        assert!(plan.resources[0].requires_auth);
        api.operations[0].security[0]
            .schemes
            .insert("Second".into(), vec![]);
        assert!(analyze_with_security(&api, &[binding()], false, Some(&catalog)).is_err());
    }
    #[test]
    fn provider_and_entity_contracts_compose_and_preserve_explain_output() {
        use crate::{PackageExt, entities, provider};
        let entity = entities();
        let handle = entity.catalog_handle();
        let tree = kaji_core::engine::Packages::new()
            .package(
                crate::package("tf")
                    .module("example.com/provider")
                    .provider_name("acme")
                    .with(provider().using_entities(handle))
                    .with(entity),
            )
            .generate(&fixture(), None)
            .unwrap();
        let source = tree
            .get("tf/internal/provider/resource_project.go")
            .unwrap();
        assert!(source.contains("ImportStatePassthroughID"));
        assert!(source.contains("RemoveResource"));
        assert!(!source.contains("JSON request document"));
        let catalog: EntityCatalog =
            serde_json::from_str(tree.get("tf/.kaji/terraform-plan.json").unwrap()).unwrap();
        assert_eq!(catalog.resources.len(), 1);
    }
    #[test]
    fn malformed_native_hints_duplicate_ids_and_empty_provider_fail_closed() {
        let mut api = fixture();
        let response = api.operations[1].responses[0].media_types[0]
            .schema
            .as_mut()
            .unwrap();
        let SchemaKind::Object { fields, .. } = &mut response.kind else {
            unreachable!()
        };
        fields[1].value.extensions.insert(
            "x-kaji-terraform".into(),
            serde_json::json!({"replacement":true}),
        );
        assert!(analyze(&api, &[binding()], false).is_err());
        let mut api = fixture();
        api.operations[1].id = api.operations[0].id.clone();
        assert!(analyze(&api, &[], true).is_err());
        let mut api = fixture();
        api.operations.retain(|op| op.method != HttpMethod::Delete);
        let result = kaji_core::engine::Packages::new()
            .package(crate::package("tf").with(crate::provider()))
            .generate(&api, None);
        let error = format!("{:#}", result.err().unwrap());
        assert!(error.contains("No supported Terraform resources"));
    }
    #[test]
    fn explanation_omits_plaintext_source_examples_but_contract_preserves_them() {
        let mut api = fixture();
        api.operations[0].annotations.insert(
            "x-source-example".into(),
            serde_json::json!({"credential":"private-source-example"}),
        );
        let request = api.operations[0].request_body.as_mut().unwrap().media_types[0]
            .schema
            .as_mut()
            .unwrap();
        let SchemaKind::Object { fields, .. } = &mut request.kind else {
            unreachable!()
        };
        fields[0].value.extensions.insert(
            "x-example".into(),
            serde_json::json!("private-field-example"),
        );
        let catalog = analyze(&api, &[], true).unwrap();
        assert!(
            catalog.resources[0]
                .create
                .annotations
                .contains_key("x-source-example")
        );
        let explanation = catalog.explanation().to_string();
        assert!(
            !explanation.contains("private-source-example")
                && !explanation.contains("private-field-example")
        );
        assert!(explanation.contains("createProject"));
        let tree = kaji_core::engine::Packages::new()
            .package(crate::package("tf").with(crate::provider()))
            .generate(&api, None)
            .unwrap();
        let artifact = tree.get("tf/.kaji/terraform-plan.json").unwrap();
        assert!(
            !artifact.contains("private-source-example")
                && !artifact.contains("private-field-example")
        );
    }
    #[test]
    fn async_lifecycle_and_wildcard_success_are_excluded_until_polling_exists() {
        for operation_index in 0..4 {
            for status in ["202", "2XX"] {
                let mut api = fixture();
                api.operations[operation_index].responses = vec![OperationResponse::json(
                    status,
                    object(vec![field("id", SchemaKind::String, true)]),
                )];
                let explanation = analyze(&api, &[], true).unwrap();
                assert!(explanation.resources.is_empty());
                assert!(
                    explanation
                        .diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.reason.contains("polling"))
                );
                let explicit = analyze(&api, &[binding()], false).err().unwrap();
                assert!(format!("{explicit:#}").contains("polling"));
            }
        }
    }
    fn waiter() -> PollingBinding {
        serde_json::from_value(serde_json::json!({"success":[{"status":200},{"pointer":"/status","equals":"ready"}],"failure":[{"status":200},{"pointer":"/status","equals":"failed"}]})).unwrap()
    }
    #[test]
    fn explicit_polling_admits_only_selected_async_lifecycles() {
        let mut api = fixture();
        api.operations[0].responses[0].status = "202".into();
        let mut binding = binding();
        binding.polling = Some(LifecyclePollingBinding {
            create: Some(waiter()),
            ..Default::default()
        });
        let catalog = analyze(&api, &[binding.clone()], false).unwrap();
        assert!(
            catalog.resources[0]
                .polling
                .as_ref()
                .unwrap()
                .create
                .is_some()
        );
        assert_eq!(
            catalog.explanation()["resources"][0]["polling"]["create"]["interval_ms"],
            1000
        );
        api.operations[0].responses[0].status = "2XX".into();
        assert!(analyze(&api, &[binding.clone()], false).is_err());
        api.operations[0].responses[0].status = "201".into();
        api.operations[1].responses[0].status = "202".into();
        assert!(analyze(&api, &[binding], false).is_err());
    }
    #[test]
    fn polling_rejects_invalid_bounds_criteria_and_missing_update() {
        let mut binding = binding();
        let good = waiter();
        assert_eq!(
            (
                good.delay_ms,
                good.interval_ms,
                good.max_attempts,
                good.timeout_ms
            ),
            (0, 1000, 60, 120000)
        );
        let mut invalid = Vec::new();
        for field in ["delay_ms", "interval_ms", "max_attempts", "timeout_ms"] {
            let mut json = serde_json::to_value(&good).unwrap();
            json[field] = serde_json::json!(4000000);
            invalid.push(serde_json::from_value::<PollingBinding>(json).unwrap());
        }
        for criteria in [
            serde_json::json!([]),
            serde_json::json!([{"status":99}]),
            serde_json::json!([{"status":200},{"status":404}]),
            serde_json::json!([{"pointer":"status","equals":true}]),
            serde_json::json!([{"pointer":"/bad~2","equals":null}]),
            serde_json::json!([{"pointer":"/status","equals":[]}]),
            serde_json::json!([{"pointer":"/status","equals":"a"},{"pointer":"/status","equals":"b"}]),
        ] {
            let mut json = serde_json::to_value(&good).unwrap();
            json["success"] = criteria;
            invalid.push(serde_json::from_value(json).unwrap());
        }
        let mut excessive = good.clone();
        excessive.success = vec![PollCriterion::Status { status: 200 }; 33];
        invalid.push(excessive);
        for status in [0, 202, 404] {
            let mut pending = good.clone();
            pending.success = vec![PollCriterion::Status { status }];
            invalid.push(pending);
        }
        let mut identical = good.clone();
        identical.failure = identical.success.clone();
        invalid.push(identical);
        for waiter in invalid {
            binding.polling = Some(LifecyclePollingBinding {
                create: Some(waiter),
                ..Default::default()
            });
            assert!(validate_polling(&binding).is_err());
        }
        binding.polling = Some(LifecyclePollingBinding {
            delete: Some(good.clone()),
            ..Default::default()
        });
        assert!(validate_polling(&binding).is_err());
        let deleted: PollingBinding =
            serde_json::from_value(serde_json::json!({"success":[{"status":404}]})).unwrap();
        binding.polling = Some(LifecyclePollingBinding {
            delete: Some(deleted),
            ..Default::default()
        });
        assert!(validate_polling(&binding).is_ok());
        binding.update = None;
        binding.polling = Some(LifecyclePollingBinding {
            update: Some(good),
            ..Default::default()
        });
        assert!(validate_polling(&binding).is_err());
        binding.polling = Some(LifecyclePollingBinding::default());
        assert!(validate_polling(&binding).is_err());
        for value in [
            serde_json::json!({"status":200,"extra":true}),
            serde_json::json!({"pointer":"/x","equals":true,"status":200}),
        ] {
            assert!(serde_json::from_value::<PollCriterion>(value).is_err());
        }
        assert!(
            validate_criteria(&[PollCriterion::Body {
                pointer: "/a~1b/~0key/0".into(),
                equals: serde_json::json!(false)
            }])
            .is_ok()
        );
    }
    #[test]
    fn reserved_root_attributes_fail_with_mapping_diagnostic() {
        let mut api = fixture();
        let schema = api.operations[1].responses[0].media_types[0]
            .schema
            .as_mut()
            .unwrap();
        if let SchemaKind::Object { fields, .. } = &mut schema.kind {
            fields
                .iter_mut()
                .find(|field| field.name == "quantity")
                .unwrap()
                .name = "count".into();
        }
        let catalog = analyze(&api, &[], true).unwrap();
        assert!(catalog.resources.is_empty());
        assert!(
            catalog
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.reason.contains("reserved root attribute count"))
        );
    }
    #[test]
    fn typed_nested_shapes_and_boundaries_are_explicit() {
        let api = Api::default();
        let nested = object(vec![
            field("displayName", SchemaKind::String, true),
            field("enabled", SchemaKind::Boolean, false),
        ]);
        assert!(matches!(
            field_shape(&api, &nested).unwrap(),
            ShapePlan::Object { .. }
        ));
        for value in [
            SchemaValue::new(SchemaKind::Array {
                items: Box::new(nested.clone()),
            }),
            SchemaValue::new(SchemaKind::Object {
                fields: vec![],
                additional_properties: AdditionalProperties::Schema {
                    value: Box::new(nested.clone()),
                },
            }),
        ] {
            assert!(field_shape(&api, &value).is_ok());
        }
        let mut nullable = nested.clone();
        nullable.nullable = true;
        assert!(field_shape(&api, &nullable).is_err());
        let mut constrained = nested.clone();
        constrained
            .constraints
            .insert("minProperties".into(), serde_json::json!(1));
        assert!(field_shape(&api, &constrained).is_err());
        let collision = object(vec![
            field("fooBar", SchemaKind::String, true),
            field("foo_bar", SchemaKind::String, true),
        ]);
        assert!(field_shape(&api, &collision).is_err());
        let recursive = Api {
            schemas: vec![Schema::new(
                "Node",
                object(vec![Field {
                    name: "next".into(),
                    value: SchemaValue::reference("Node"),
                    required: false,
                    annotations: Default::default(),
                }]),
            )],
            ..Default::default()
        };
        assert!(field_shape(&recursive, &SchemaValue::reference("Node")).is_err());
    }
    #[test]
    fn composite_parent_path_is_explicit_and_migration_versions_are_complete() {
        let mut api = fixture();
        let parent = OperationParameter {
            name: "org".into(),
            location: "path".into(),
            required: true,
            schema: Some(SchemaValue::new(SchemaKind::String)),
            description: None,
            annotations: Default::default(),
        };
        for op in &mut api.operations {
            op.path = format!("/organizations/{{org}}{}", op.path);
            op.parameters.push(parent.clone());
            for response in &mut op.responses {
                for media in &mut response.media_types {
                    if let Some(SchemaValue {
                        kind: SchemaKind::Object { fields, .. },
                        ..
                    }) = &mut media.schema
                    {
                        fields.push(field("organizationId", SchemaKind::String, true));
                    }
                }
            }
        }
        let mut binding = binding();
        binding.id_parameter = None;
        binding.identity = vec![
            IdentityBinding {
                parameter: "org".into(),
                field: "organizationId".into(),
            },
            IdentityBinding {
                parameter: "projectId".into(),
                field: "id".into(),
            },
        ];
        let catalog = analyze(&api, &[binding.clone()], false).unwrap();
        assert_eq!(catalog.resources.len(), 1, "{:?}", catalog.diagnostics);
        let parent = catalog.resources[0]
            .attributes
            .iter()
            .find(|a| a.name == "organization_id")
            .unwrap();
        assert!(parent.required && parent.replace_on_change && !parent.update_input);
        binding.schema_version = 1;
        assert!(analyze(&api, &[binding.clone()], false).is_err());
        binding.state_upgrades = vec![StateUpgradeBinding {
            version: 0,
            rename_fields: BTreeMap::from([("old_name".into(), "name".into())]),
        }];
        assert_eq!(
            analyze(&api, &[binding.clone()], false)
                .unwrap()
                .resources
                .len(),
            1
        );
        binding.state_upgrades[0].rename_fields = BTreeMap::from([("id".into(), "name".into())]);
        assert!(analyze(&api, &[binding], false).is_err());
    }
}
