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
                    "update": resource.update.as_ref().map(&operation),
                    "delete": operation(&resource.delete),
                    "id_parameter": resource.id_parameter, "id_field": resource.id_field,
                    "attributes": resource.attributes, "requires_auth": resource.requires_auth
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
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttributePlan {
    pub name: String,
    pub wire_name: String,
    pub ty: ScalarType,
    pub required: bool,
    pub computed: bool,
    pub optional: bool,
    pub replace_on_change: bool,
    pub sensitive: bool,
    pub update_input: bool,
    pub update_required: bool,
    pub response_required: bool,
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
    for id in [&binding.create, &binding.read, &binding.delete]
        .into_iter()
        .chain(binding.update.as_ref())
    {
        let operation = find(id)?;
        ensure!(
            !operation
                .responses
                .iter()
                .any(|response| response.status == "202"
                    || response.status.eq_ignore_ascii_case("2XX")),
            "HTTP 202 or wildcard 2XX lifecycle success requires explicit polling/completion semantics, unsupported in Terraform v1"
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
        !create.path.contains('{'),
        "parent/create path parameters are unsupported in v1"
    );
    let id_parameter = binding
        .id_parameter
        .clone()
        .or_else(|| single_path_parameter(&read.path))
        .context("read requires one explicit identity path parameter")?;
    let id_field = binding.id_field.clone().unwrap_or_else(|| "id".into());
    let template = format!("{{{id_parameter}}}");
    for op in [&read, &delete].into_iter().chain(update.as_ref()) {
        ensure!(
            single_path_parameter(&op.path).as_deref() == Some(&id_parameter),
            "item operation {} must have only identity parameter {}",
            op.id,
            id_parameter
        );
        ensure!(
            op.path == read.path,
            "all item lifecycle operations must share a path"
        );
        ensure!(
            op.parameters
                .iter()
                .all(|p| p.location == "path" && p.name == id_parameter),
            "query/header/cookie operation parameters are unsupported in v1"
        );
        for p in &op.parameters {
            ensure!(
                p.schema
                    .as_ref()
                    .is_some_and(|v| scalar(api, v).ok() == Some(ScalarType::String)),
                "identity path parameter must be a string"
            );
        }
        ensure!(
            op.path.ends_with(&template),
            "identity parameter must be the final item path segment"
        );
    }
    ensure!(
        create.parameters.is_empty(),
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
        let ty = scalar(api, &read_field.value)?;
        let create_field = create_fields.iter().find(|f| f.name == read_field.name);
        let update_field = update_fields.iter().find(|f| f.name == read_field.name);
        if let Some(field) = create_field {
            ensure!(
                !field.value.read_only && !field.value.write_only && !read_field.value.read_only,
                "conflicting read-only/write-only configurable field {}",
                field.name
            );
            ensure!(
                scalar(api, &field.value)? == ty,
                "create/read field {} types differ",
                field.name
            );
            let response_field = created
                .iter()
                .find(|f| f.name == field.name)
                .context("create response must include configurable fields for state validation")?;
            ensure!(
                scalar(api, &response_field.value)? == ty,
                "create response field {} type differs",
                field.name
            );
        }
        if let Some(field) = update_field {
            ensure!(
                scalar(api, &field.value)? == ty,
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
        attributes.push(AttributePlan {
            name,
            wire_name: read_field.name.clone(),
            ty,
            required: create_field.is_some_and(|f| f.required),
            optional: create_field.is_some_and(|f| !f.required),
            computed: create_field.is_none_or(|f| !f.required),
            replace_on_change: create_field.is_some() && (update_field.is_none() || forced),
            sensitive: sensitive.unwrap_or(false),
            update_input: update_field.is_some(),
            update_required: update_field.is_some_and(|field| field.required),
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
    };
    Ok((plan, scheme, auth))
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
            field("count", SchemaKind::Integer, false),
            field("ratio", SchemaKind::Number, false),
        ]);
        let output = object(vec![
            id,
            field("name", SchemaKind::String, true),
            field("enabled", SchemaKind::Boolean, true),
            field("count", SchemaKind::Integer, false),
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
            .find(|a| a.name == "count")
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
}
