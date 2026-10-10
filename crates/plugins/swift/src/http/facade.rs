//! Facade emission for the Swift HTTP SDK.
use crate::*;

pub(crate) fn render_operations(api: &Api) -> String {
    let mut output =
        format!("{NOTICE}\nimport Foundation\n\npublic extension PoolsterClient {{\n  ");
    for operation in &api.operations {
        output.push_str(&render_operation(operation, "    "));
    }
    output.push_str("}\n");
    output
}

pub(crate) fn operation_parameters(operation: &Operation) -> Vec<ParameterRender> {
    let mut values = operation
        .parameters
        .iter()
        .filter(|parameter| {
            matches!(
                parameter.location.as_str(),
                "path" | "query" | "header" | "cookie" | "querystring"
            )
        })
        .map(|parameter| ParameterRender {
            signature: format!(
                "{}: {}{}",
                parameter_name(parameter),
                if parameter.location == "querystring" {
                    if parameter.required {
                        "String".into()
                    } else {
                        "String?".into()
                    }
                } else {
                    parameter.schema.as_ref().map_or_else(
                        || {
                            if parameter.required {
                                "JSONValue".to_owned()
                            } else {
                                "JSONValue?".to_owned()
                            }
                        },
                        |schema| swift_type(schema, !parameter.required),
                    )
                },
                if parameter.required { "" } else { " = nil" }
            ),
        })
        .collect::<Vec<_>>();
    if let Some(body) = &operation.request_body {
        let ty = if let Some(ty) = multipart::body_type(operation) {
            format!("{ty}{}", if body.required { "" } else { "?" })
        } else {
            body.media_types
                .iter()
                .find(|media| {
                    media.content_type == "application/json"
                        || media.content_type.ends_with("+json")
                })
                .or_else(|| body.media_types.first())
                .and_then(|media| media.schema.as_ref())
                .map_or_else(
                    || {
                        if body.required {
                            "JSONValue".to_owned()
                        } else {
                            "JSONValue?".to_owned()
                        }
                    },
                    |schema| swift_type(schema, !body.required),
                )
        };
        values.push(ParameterRender {
            signature: format!("body: {ty}{}", if body.required { "" } else { " = nil" }),
        });
    }
    values
}

pub(crate) fn operation_groups(api: &Api) -> BTreeMap<String, Vec<&Operation>> {
    let mut groups = BTreeMap::new();
    for operation in &api.operations {
        let segment = operation
            .path
            .split('/')
            .find(|part| !part.is_empty() && !part.starts_with('{'))
            .unwrap_or("api");
        groups
            .entry(type_name(segment))
            .or_insert_with(Vec::new)
            .push(operation);
    }
    groups
}

pub(crate) fn render_resource(
    api: &Api,
    resource: &str,
    operations: &[&Operation],
    extension: bool,
) -> String {
    let property = match identifier(&lower_camel(resource)).as_str() {
        "session" | "options" | "hooks" | "transport" | "encoder" | "decoder" => {
            format!("{}Resource", identifier(&lower_camel(resource)))
        }
        _ => identifier(&lower_camel(resource)),
    };
    let mut output = if extension {
        format!("{NOTICE}\nimport Foundation\n\nextension {resource}Resource {{\n  ")
    } else {
        format!(
            "{NOTICE}\nimport Foundation\n\npublic extension PoolsterClient {{\n  var {property}: {resource}Resource {{\n    {resource}Resource(client: self)\n  }}\n  \n}}\n\n\npublic struct {resource}Resource: Sendable {{\n  internal let client: PoolsterClient\n    internal init(client: PoolsterClient) {{\n    self.client = client\n  }}\n  \n"
        )
    };
    for operation in operations {
        let name = function_name(&operation.id);
        let parameters = operation_parameters(operation);
        let sse = operation_is_sse(operation);
        let response = if sse {
            "PoolsterEventSequence".to_owned()
        } else if operation_is_multipart_response(operation) {
            "Data".to_owned()
        } else {
            operation
                .success_schema()
                .map(|schema| swift_type(schema, false))
                .unwrap_or_else(|| "Void".to_owned())
        };
        let _ = write!(output, "\n    public func {name}(");
        for (index, parameter) in parameters.iter().enumerate() {
            if index > 0 {
                output.push_str(", ");
            }
            output.push_str(&parameter.signature);
        }
        let effect = if sse { "" } else { " async throws" };
        let _ = writeln!(output, "){effect} -> {response} {{");
        let call_args = parameters
            .iter()
            .map(|parameter| {
                let label = parameter.signature.split(':').next().unwrap_or_default();
                format!("{label}: {label}")
            })
            .collect::<Vec<_>>()
            .join(", ");
        if sse {
            let _ = writeln!(output, "        return self.client.{name}({call_args})");
        } else if response == "Void" {
            let _ = writeln!(output, "        try await self.client.{name}({call_args})");
        } else {
            let _ = writeln!(
                output,
                "        return try await self.client.{name}({call_args})"
            );
        }
        output.push_str("    }\n");
    }
    if operations.iter().any(|operation| {
        poolster_core::poolster_extension(&operation.annotations, "pagination").is_some()
            || operation.annotations.contains_key("x-speakeasy-pagination")
    }) {
        let mut resource_api = api.clone();
        resource_api
            .operations
            .retain(|operation| operations.iter().any(|item| item.id == operation.id));
        if let Ok(pages) = pagination::render(&resource_api) {
            output.push_str(
                &pages
                    .replace("    func ", "    public func ")
                    .replace("self.", "client.")
                    .replace("{ current in", "{ [client] current in")
                    .replace("{ nextURL in", "{ [client] nextURL in"),
            );
        }
    }
    output.push_str("}\n");
    output
}

pub(crate) fn lower_camel(value: &str) -> String {
    let upper = type_name(value);
    let mut chars = upper.chars();
    chars.next().map_or_else(
        || "api".to_owned(),
        |first| first.to_ascii_lowercase().to_string() + chars.as_str(),
    )
}
