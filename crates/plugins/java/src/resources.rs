use super::*;

pub(super) fn resource_operations(api: &Api) -> BTreeMap<String, Vec<(&Operation, String)>> {
    let mut groups = BTreeMap::<String, Vec<(&Operation, String)>>::new();
    for operation in &api.operations {
        let resource = resource_name(operation);
        let candidate = resource_method_name(operation, &resource);
        let operations = groups.entry(resource).or_default();
        let used = operations
            .iter()
            .map(|(_, method)| method.clone())
            .collect::<BTreeSet<_>>();
        let method = if used.contains(&candidate) {
            method_name(&operation.id)
        } else {
            candidate
        };
        operations.push((operation, method));
    }
    groups
}

fn resource_name(operation: &Operation) -> String {
    operation
        .annotations
        .get("tags")
        .and_then(serde_json::Value::as_array)
        .and_then(|tags| tags.first())
        .and_then(serde_json::Value::as_str)
        .map(type_name)
        .filter(|tag| !tag.is_empty())
        .unwrap_or_else(|| {
            operation
                .path
                .split('/')
                .filter(|segment| !segment.is_empty())
                .find(|segment| {
                    !segment.starts_with('{')
                        && !is_version_segment(segment)
                        && *segment != "email"
                        && *segment != "api"
                })
                .map(type_name)
                .filter(|segment| !segment.is_empty())
                .unwrap_or_else(|| "Api".into())
        })
}

fn is_version_segment(segment: &str) -> bool {
    segment.strip_prefix('v').is_some_and(|number| {
        !number.is_empty() && number.chars().all(|character| character.is_ascii_digit())
    })
}

fn resource_method_name(operation: &Operation, resource: &str) -> String {
    let operation_name = method_name(&operation.id);
    let singular = resource.strip_suffix('s').unwrap_or(resource);
    for suffix in [resource, singular] {
        if let Some(method) = operation_name.strip_suffix(suffix) {
            if !method.is_empty() {
                return method_name(method);
            }
        }
    }
    operation_name
}

/// The generated `Client` keeps direct operations for migration. Reserve
/// those method names before exporting resource accessors, so a health probe
/// named `authCheck` becomes `authCheckResource()` instead of duplicating the
/// direct `authCheck(...)` method.
pub(super) fn resource_accessor_name(api: &Api, resource: &str) -> String {
    let candidate = field_name(resource);
    if api
        .operations
        .iter()
        .any(|operation| method_name(&operation.id) == candidate)
    {
        format!("{candidate}Resource")
    } else {
        candidate
    }
}

pub(super) fn render_resource_chunk(
    resource: &str,
    operations: &[(&Operation, String)],
    package: &str,
    index: usize,
) -> String {
    let parent = if index == 0 {
        String::new()
    } else {
        format!(" extends {resource}ResourcePart{:03}", index - 1)
    };
    let constructor = if index == 0 {
        format!(
            "    protected {resource}ResourcePart{index:03}(Client client) {{ this.client = client; }}"
        )
    } else {
        format!(
            "    protected {resource}ResourcePart{index:03}(Client client) {{ super(client); }}"
        )
    };
    let mut output = format!(
        "package {package}.internal.resources;\n\n\nimport com.fasterxml.jackson.databind.JsonNode;\n\nimport java.util.List;\n\nimport java.util.Map;\n\nimport {package}.*;\n\nimport {package}.model.*;\n\n\n{NOTICE}\n/** Bounded operations for the {resource} resource. */\npublic class {resource}ResourcePart{index:03}{parent} {{\n  {field}\n{constructor}\n\n",
        field = if index == 0 {
            "    protected final Client client;\n"
        } else {
            ""
        },
    );
    for (operation, facade_method) in operations {
        let return_type = match response_surface(operation) {
            ResponseSurface::Json(schema) => operation_response_type(schema),
            ResponseSurface::Binary => "byte[]".to_owned(),
            ResponseSurface::Sse => "java.util.stream.Stream<String>".to_owned(),
            ResponseSurface::Empty => "void".to_owned(),
        };
        let has_input =
            !operation_parameters(operation).is_empty() || request_body_schema(operation).is_some();
        let input = if has_input {
            format!("Client.{}Request input", type_name(&operation.id))
        } else {
            String::new()
        };
        let invocation = if has_input { "input" } else { "" };
        let _ = writeln!(
            output,
            "    public {return_type} {facade_method}({input}) {{\n  "
        );
        if return_type == "void" {
            let _ = writeln!(
                output,
                "        client.{}({invocation});",
                method_name(&operation.id)
            );
        } else {
            let _ = writeln!(
                output,
                "        return client.{}({invocation});",
                method_name(&operation.id)
            );
        }
        output.push_str("    }\n\n");
        if java_pagination(operation).is_some() {
            let pages_return_type = match response_surface(operation) {
                ResponseSurface::Json(schema) => operation_response_type(schema),
                _ => unreachable!("validated Java pagination has a JSON response"),
            };
            let _ = writeln!(
                output,
                "    /** Lazily fetches pages through the normal generated operation. */\n    public java.lang.Iterable<{pages_return_type}> {facade_method}Pages({input}) {{\n  return client.{}Pages({invocation});\n  \n\n}}\n\n",
                method_name(&operation.id)
            );
        }
    }
    output.push_str("}\n");
    output
}

pub(super) fn render_resource_facade(resource: &str, package: &str, chunks: usize) -> String {
    let last = chunks.saturating_sub(1);
    format!(
        "package {package};\n\n\nimport {package}.internal.resources.{resource}ResourcePart{last:03};\n\n\n{NOTICE}\n/** Typed namespace for {resource} operations. */\npublic final class {resource}Resource extends {resource}ResourcePart{last:03} {{\n  {resource}Resource(Client client) {{\n    super(client);\n\n  }}\n  \n}}\n\n"
    )
}
