use super::*;

pub(super) fn resource_operations(api: &Api) -> BTreeMap<String, Vec<&Operation>> {
    let mut resources = BTreeMap::<String, Vec<&Operation>>::new();
    for operation in &api.operations {
        resources
            .entry(go_type_name(&resource_name(operation)))
            .or_default()
            .push(operation);
    }
    resources
}

pub(super) fn resource_name(operation: &Operation) -> String {
    operation
        .annotations
        .get("tags")
        .and_then(|tags| tags.as_array())
        .and_then(|tags| tags.first())
        .and_then(|tag| tag.as_str())
        .filter(|tag| !tag.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| {
            operation
                .path
                .split('/')
                .filter(|segment| !segment.is_empty() && !segment.starts_with('{'))
                .find(|segment| {
                    !matches!(*segment, "api" | "email") && !is_version_segment(segment)
                })
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "api".into())
}

pub(super) fn is_version_segment(segment: &str) -> bool {
    let mut characters = segment.chars();
    matches!(characters.next(), Some('v' | 'V'))
        && characters
            .next()
            .is_some_and(|character| character.is_ascii_digit())
}

/// Go selectors share a namespace with methods. A resource whose generated
/// field matches any direct operation therefore receives a stable `Resource`
/// suffix instead of producing an uncompilable client (for example an
/// `auth-check` resource alongside `Client.AuthCheck`).
pub(super) fn resource_facade_names(
    resources: &BTreeMap<String, Vec<&Operation>>,
) -> BTreeMap<String, String> {
    let operations: BTreeSet<_> = resources
        .values()
        .flatten()
        .map(|operation| go_type_name(&operation.id))
        .collect();
    let mut reserved = operations.clone();
    reserved.extend(resources.keys().cloned());
    let mut names = BTreeMap::new();
    for resource in resources.keys() {
        let mut name = resource.clone();
        if operations.contains(resource) {
            name.push_str("Resource");
            while reserved.contains(&name) {
                name.push_str("Resource");
            }
        }
        reserved.insert(name.clone());
        names.insert(resource.clone(), name);
    }
    names
}

pub(super) fn resource_method_name(operation: &str, resource: &str) -> String {
    let singular = resource.strip_suffix('s').unwrap_or(resource);
    let mut method = operation;
    while let Some(shorter) = method
        .strip_prefix(resource)
        .or_else(|| method.strip_prefix(singular))
        .filter(|value| !value.is_empty() && value.len() < method.len())
    {
        method = shorter;
    }
    method
        .strip_suffix(resource)
        .or_else(|| method.strip_suffix(singular))
        .filter(|method| !method.is_empty())
        .unwrap_or(method)
        .into()
}

pub(super) fn render_namespaced_operation(
    output: &mut String,
    api: &Api,
    resource: &str,
    facade: &str,
    method: &str,
    direct: &str,
    operation: &Operation,
) {
    let request = operation_request_name(api, operation);
    let has_input = !operation.parameters.is_empty() || operation.request_body.is_some();
    let parameters = if has_input {
        format!(", input *{request}")
    } else {
        String::new()
    };
    let result = match operation_response_kind(operation) {
        GoResponseKind::None => "error".to_owned(),
        GoResponseKind::Json(response) => format!("(*{response}, error)"),
        GoResponseKind::Text => "(string, error)".to_owned(),
        GoResponseKind::Binary => "([]byte, error)".to_owned(),
        GoResponseKind::EventStream => "(io.ReadCloser, error)".to_owned(),
    };
    let _ = writeln!(
        output,
        "// {method} invokes {direct} through the {resource} namespace.\nfunc (service *{facade}Service) {method}(ctx context.Context{parameters}) {result} {{"
    );
    if has_input {
        let _ = writeln!(output, "\treturn service.client.{direct}(ctx, input)\n}}\n");
    } else {
        let _ = writeln!(output, "\treturn service.client.{direct}(ctx)\n}}\n");
    }
}
