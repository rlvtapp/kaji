//! Facade emission for the csharp HTTP SDK.
use crate::*;

pub(crate) fn bounded_filename(value: &str, index: usize, extension: &str) -> String {
    let readable = pascal_case(value);
    let prefix = readable.chars().take(96).collect::<String>();
    let hash = value
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    format!("{prefix}_{index:05}_{hash:016x}.{extension}")
}

/// Emits one bounded partial declaration. All pieces compile as one
/// `PoolsterClient`, so this changes source layout without changing the public API.
pub(crate) fn render_operation_chunk(
    api: &Api,
    operations: &[Operation],
    namespace: &str,
) -> String {
    let mut output = format!(
        "{NOTICE}\nusing System.Globalization;\n\nusing System.Net.Http.Headers;\n\nusing System.Net.Http.Json;\n\nusing System.Text.Json;\n\n\nnamespace {namespace};\n\n\npublic sealed partial class PoolsterClient\n{{\n  "
    );
    for operation in operations {
        render_operation(&mut output, operation);
        pagination::render(&mut output, api, operation);
        render_declared_error_mapper(&mut output, operation);
    }
    output.push_str("}\n");
    output
}

pub(crate) fn render_resource_chunk(
    resource: &str,
    operations: &[&Operation],
    namespace: &str,
    includes_declaration: bool,
) -> String {
    let mut output = format!(
        "{NOTICE}\nusing System.Text.Json;\nnamespace {namespace};\n\n// Resource facades delegate to PoolsterClient so configuration stays centralized.\n"
    );
    let _ = writeln!(
        output,
        "\n/// <summary>Typed operations for the {resource} resource.</summary>\npublic sealed partial class {resource}Resource\n{{\n  "
    );
    if includes_declaration {
        let _ = writeln!(
            output,
            "    private readonly PoolsterClient _client;\n\n    internal {resource}Resource(PoolsterClient client) => _client = client;\n"
        );
    }
    for operation in operations {
        render_resource_operation(&mut output, operation);
    }
    output.push_str("}\n");
    output
}

pub(crate) fn render_resource_operation(output: &mut String, operation: &Operation) {
    let name = pascal_case(&operation.id);
    let return_type = match operation_response_surface(operation) {
        DotnetResponseSurface::Json(value) => format!("Task<{value}>"),
        DotnetResponseSurface::Binary => "Task<byte[]>".into(),
        DotnetResponseSurface::Sse => "IAsyncEnumerable<string>".into(),
        DotnetResponseSurface::Empty => "Task".into(),
    };
    let parameters = facade_parameters(operation);
    let arguments = facade_arguments(operation);
    let _ = writeln!(
        output,
        "    /// <summary>Invokes {} {}.</summary>\n    public {return_type} {name}Async({}) => _client.{name}Async({});\n",
        operation.method.as_str(),
        xml_escape(&operation.path),
        parameters.join(", "),
        arguments.join(", "),
    );
}

/// The generated direct client intentionally orders required parameters before
/// optional ones. Facades use the same order and defaults so the delegation is
/// a transparent, strongly typed forwarding call.
pub(crate) fn facade_parameters(operation: &Operation) -> Vec<String> {
    let mut required = Vec::new();
    let mut optional = Vec::new();
    for parameter in &operation.parameters {
        let declaration = format!(
            "{} {}{}",
            if parameter.location == "querystring" {
                if parameter.required {
                    "string".into()
                } else {
                    "string?".into()
                }
            } else {
                parameter
                    .schema
                    .as_ref()
                    .map(|schema| csharp_type(schema, !parameter.required))
                    .unwrap_or_else(|| "JsonElement".into())
            },
            parameter_name(parameter),
            if parameter.required { "" } else { " = default" },
        );
        if parameter.required {
            required.push(declaration);
        } else {
            optional.push(declaration);
        }
    }
    if let Some(body_type) = operation_request_type(operation) {
        let required_body = operation
            .request_body
            .as_ref()
            .is_none_or(|body| body.required);
        let declaration = format!(
            "{} body{}",
            nullable_type(
                if request_body_is_binary(operation) {
                    "byte[]".into()
                } else {
                    body_type
                },
                !required_body
            ),
            if required_body { "" } else { " = default" },
        );
        if required_body {
            required.push(declaration);
        } else {
            optional.push(declaration);
        }
    }
    required.extend(optional);
    required.push("CancellationToken cancellationToken = default".into());
    required
}

pub(crate) fn facade_arguments(operation: &Operation) -> Vec<String> {
    let mut required = Vec::new();
    let mut optional = Vec::new();
    for parameter in &operation.parameters {
        let name = parameter_name(parameter);
        if parameter.required {
            required.push(name);
        } else {
            optional.push(name);
        }
    }
    if operation_request_type(operation).is_some() {
        let required_body = operation
            .request_body
            .as_ref()
            .is_none_or(|body| body.required);
        if required_body {
            required.push("body".into());
        } else {
            optional.push("body".into());
        }
    }
    required.extend(optional);
    required.push("cancellationToken".into());
    required
}

pub(crate) fn operation_groups(api: &Api) -> Vec<String> {
    let mut groups = api
        .operations
        .iter()
        .map(operation_resource_name)
        .collect::<Vec<_>>();
    groups.sort();
    groups.dedup();
    groups
}
