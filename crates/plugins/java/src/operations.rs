use super::*;

pub(super) fn render_operation_chunk(
    api: &Api,
    operations: &[Operation],
    package: &str,
    index: usize,
) -> String {
    let parent = if index == 0 {
        format!("{package}.ClientBase")
    } else {
        format!("Operations{:03}", index - 1)
    };
    let mut output = format!(
        "package {package}.internal;\n\nimport com.fasterxml.jackson.core.JsonProcessingException;\nimport com.fasterxml.jackson.databind.JsonNode;\nimport java.util.*;\nimport {package}.*;\nimport {package}.model.*;\n\n{NOTICE}\n/** Bounded generated operation partition. */\npublic class Operations{index:03} extends {parent} {{\n    protected Operations{index:03}(ClientConfig config) {{ super(config); }}\n\n"
    );
    for operation in operations {
        render_operation(&mut output, operation);
        if let Some(pagination) = java_pagination(operation) {
            render_pagination_operation(&mut output, operation, &pagination);
        }
    }
    let mut chunk_api = api.clone();
    chunk_api.operations = operations.to_vec();
    render_declared_error_types(&mut output, &chunk_api);
    output.push_str("}\n");
    output
}

pub(super) fn render_client_facade(
    api: &Api,
    package: &str,
    style: SdkClientStyle,
    chunks: usize,
) -> String {
    let parent = if chunks == 0 {
        "ClientBase".to_owned()
    } else {
        format!("{package}.internal.Operations{:03}", chunks - 1)
    };
    let mut output = format!(
        "package {package};\n\n{NOTICE}\n/** Public API facade. Operations are inherited from bounded internal partitions. */\npublic final class Client extends {parent} {{\n"
    );
    if style == SdkClientStyle::Namespaced {
        for (resource, _) in resource_operations(api) {
            let _ = writeln!(
                output,
                "    private final {resource}Resource {}Resource;",
                field_name(&resource)
            );
        }
    }
    output.push_str("\n    public Client(ClientConfig config) {\n        super(config);\n");
    if style == SdkClientStyle::Namespaced {
        for (resource, _) in resource_operations(api) {
            let property = field_name(&resource);
            let _ = writeln!(
                output,
                "        this.{property}Resource = new {resource}Resource(this);"
            );
        }
    }
    output.push_str("    }\n\n");
    if style == SdkClientStyle::Namespaced {
        for (resource, _) in resource_operations(api) {
            let accessor = resource_accessor_name(api, &resource);
            let field = field_name(&resource);
            let _ = writeln!(
                output,
                "    public {resource}Resource {accessor}() {{ return {field}Resource; }}\n"
            );
        }
    }
    output.push_str("}\n");
    output
}

pub(super) fn render_operation(output: &mut String, operation: &Operation) {
    let operation_name = type_name(&operation.id);
    let request_name = format!("{operation_name}Request");
    let parameters = operation_parameters(operation);
    let body_schema = request_body_schema(operation);
    let binary_body = request_body_is_binary(operation);
    let has_input = !parameters.is_empty() || body_schema.is_some();
    if has_input {
        let _ = writeln!(output, "    /** Inputs accepted by {operation_name}. */");

        let mut fields = Vec::new();
        for parameter in &parameters {
            fields.push(format!(
                "            {} {}",
                parameter_type(parameter),
                parameter_name(parameter)
            ));
        }
        if let Some(schema) = body_schema {
            fields.push(format!(
                "            {} body",
                if multipart::mixed(operation) {
                    "java.lang.Object".into()
                } else if multipart::selected(operation) {
                    multipart::body_name(operation)
                } else if binary_body {
                    "byte[]".to_owned()
                } else {
                    java_type(schema)
                }
            ));
        }
        if fields.len() > 200 {
            let _ = writeln!(output, "    public static final class {request_name} {{");
            for declaration in fields {
                let (ty, name) = declaration.trim().rsplit_once(' ').expect("typed argument");
                let _ = writeln!(
                    output,
                    "        private {ty} {name};\n        public {ty} {name}() {{ return {name}; }}\n        public {request_name} {name}({ty} value) {{ this.{name} = value; return this; }}"
                );
            }
            output.push_str("    }\n\n");
        } else {
            let _ = writeln!(output, "    public record {request_name}(");
            output.push_str(&fields.join(",\n"));
            output.push_str("\n    ) {}\n\n");
        }
    }
    let response = response_surface(operation);
    let return_type = match response {
        ResponseSurface::Json(schema) => operation_response_type(schema),
        ResponseSurface::Binary => "byte[]".to_owned(),
        ResponseSurface::Sse => "java.util.stream.Stream<String>".to_owned(),
        ResponseSurface::Empty => "void".to_owned(),
    };
    let input = if has_input {
        format!("{request_name} input")
    } else {
        String::new()
    };
    let _ = writeln!(
        output,
        "    /** Invokes {} {}. */",
        operation.method.as_str(),
        operation.path
    );
    let _ = writeln!(
        output,
        "    public {return_type} {method}({input}) {{",
        method = method_name(&operation.id)
    );
    if has_input {
        output.push_str("        Objects.requireNonNull(input, \"input\");\n");
    }
    if multipart::selected(operation) && operation.request_body.as_ref().is_some_and(|b| b.required)
    {
        output.push_str(
            "        Objects.requireNonNull(input.body(), \"required multipart body\");\n",
        );
    }
    let maps_declared_errors = !declared_error_responses(operation).is_empty()
        && !matches!(response, ResponseSurface::Sse);
    if maps_declared_errors {
        output.push_str("        try {\n");
    }
    let _ = writeln!(output, "        var path = {:?};", operation.path);
    output.push_str("        var query = new ArrayList<QueryParameter>();\n        var headers = new java.util.LinkedHashMap<String, String>();\n");
    for parameter in &parameters {
        let accessor = if has_input {
            format!("input.{}()", parameter_name(parameter))
        } else {
            String::new()
        };
        if parameter_json_content(parameter)
            && matches!(
                parameter.location.as_str(),
                "path" | "query" | "header" | "cookie"
            )
        {
            let serialized = format!("jsonParameter({accessor})");
            let statement = match parameter.location.as_str() {
                "path" => format!(
                    "path=path.replace({:?},pathValue({serialized}));",
                    format!("{{{}}}", parameter.name)
                ),
                "query" => format!(
                    "query.add(new QueryParameter({:?},{serialized}));",
                    parameter.name
                ),
                "cookie" => format!(
                    "headers.merge(\"Cookie\",{:?}+\"=\"+pathValue({serialized}),(left,right)->left+\"; \"+right);",
                    parameter.name
                ),
                _ => format!("headers.put({:?},{serialized});", parameter.name),
            };
            if parameter.required {
                let _ = writeln!(output, "        {statement}");
            } else {
                let _ = writeln!(output, "        if ({accessor}!=null) {{{statement}}}");
            }
            continue;
        }
        match parameter.location.as_str() {
            "path" => {
                let _ = writeln!(
                    output,
                    "        if ({accessor} != null) path = path.replace(\"{{{}}}\", pathValue({accessor}));",
                    parameter.name
                );
            }
            "cookie" => {
                let _ = writeln!(
                    output,
                    "        if ({accessor} !=null) headers.merge(\"Cookie\",{:?}+\"=\"+pathValue({accessor}),(left,right)->left+\"; \"+right);",
                    parameter.name
                );
            }
            "querystring" => {
                let _ = writeln!(
                    output,
                    "        if ({accessor} != null && !{accessor}.isEmpty()) {{ validateWholeQuery({accessor}); path += \"?\" + {accessor}; }}"
                );
            }
            "header" => {
                let _ = writeln!(
                    output,
                    "        if ({accessor} != null) headers.put({:?}, String.valueOf({accessor}));",
                    parameter.name
                );
            }
            "query" => {
                let _ = writeln!(
                    output,
                    "        query.add(new QueryParameter({:?}, {accessor}));",
                    parameter.name
                );
            }
            _ => {}
        }
    }
    let idempotency_header = poolster_core::idempotency::resolved(operation)
        .map(|policy| {
            if policy.auto_generate {
                let _ = writeln!(
                    output,
                    "        headers.putIfAbsent({:?}, java.util.UUID.randomUUID().toString());",
                    policy.header
                );
            }
            format!("{:?}", policy.header)
        })
        .unwrap_or_else(|| "null".into());
    if let Some(media) = request_content_type(operation).filter(|media| {
        matches!(
            *media,
            "application/x-ndjson"
                | "application/ndjson"
                | "application/jsonl"
                | "application/json-seq"
        )
    }) {
        let _ = writeln!(output, "        headers.put(\"Content-Type\",{media:?});");
    }
    let body = if body_schema.is_some() && has_input {
        "input.body()"
    } else {
        "null"
    };
    match response {
        ResponseSurface::Json(schema) => {
            let _ = writeln!(
                output,
                "        var response = requestWithRetry({:?}, path, query, headers, {body}, {idempotency_header});",
                operation.method.as_str()
            );
            let _ = writeln!(
                output,
                "        return decode(response, {}.class);",
                response_class(schema)
            );
        }
        ResponseSurface::Empty => {
            let _ = writeln!(
                output,
                "        requestWithRetry({:?}, path, query, headers, {body}, {idempotency_header});",
                operation.method.as_str()
            );
        }
        ResponseSurface::Binary => {
            let _ = writeln!(
                output,
                "        return requestBinary({:?}, path, query, headers, {body});",
                operation.method.as_str()
            );
        }
        ResponseSurface::Sse => {
            let _ = writeln!(
                output,
                "        return requestEventStream({:?}, path, query, headers, {body});",
                operation.method.as_str()
            );
        }
    }
    if maps_declared_errors {
        let map = java_error_mapper_name(operation);
        let _ = writeln!(
            output,
            "        }} catch (ApiException error) {{\n            throw {map}(error);\n        }}"
        );
    }
    output.push_str("    }\n\n");
}
