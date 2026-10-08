use super::*;

pub(super) fn render_resource(resource: &str, parts: usize, namespace: &str) -> String {
    let mut output = format!(
        "<?php\n\ndeclare(strict_types=1);\n\nnamespace {namespace}\\Resources;\n\nuse {namespace}\\Client;\n\n{NOTICE}\nfinal class {resource}Resource\n{{\n"
    );
    for part in 0..parts {
        let _ = writeln!(output, "    use {resource}ResourceOperations{part:03};");
    }
    output.push_str(
        "\n    public function __construct(private readonly Client $client)\n    {\n    }\n}\n",
    );
    output
}

pub(super) fn render_resource_trait(
    api: &Api,
    resource: &str,
    operations: &[(&Operation, String)],
    part: usize,
    namespace: &str,
    named_types: &NamedTypes,
) -> String {
    let mut output = format!(
        "<?php\n\ndeclare(strict_types=1);\n\nnamespace {namespace}\\Resources;\n\nuse {namespace}\\Client;\n"
    );
    for name in resource_model_imports(operations, named_types) {
        let _ = writeln!(output, "use {namespace}\\Models\\{name};");
    }
    let _ = writeln!(
        output,
        "\n{NOTICE}\n/** Bounded resource facade slice; composed into {resource}Resource. */\ntrait {resource}ResourceOperations{part:03}\n{{"
    );
    for (operation, facade_method) in operations {
        let response = response_schema(operation);
        let return_type = if operation_is_sse_response(operation) {
            "\\Psr\\Http\\Message\\StreamInterface".into()
        } else if operation_is_binary_response(operation) {
            "string".into()
        } else {
            response
                .map(|schema| php_type(schema, named_types))
                .unwrap_or_else(|| "void".into())
        };
        let return_type = if return_type == "null" {
            "void"
        } else {
            &return_type
        };
        let arguments = facade_arguments(operation, named_types);
        let declaration = arguments
            .iter()
            .map(|(_, declaration)| declaration.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let invocation = arguments
            .iter()
            .map(|(variable, _)| format!("${variable}"))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            output,
            "    public function {facade_method}({declaration}): {return_type}\n    {{"
        );
        if return_type == "void" {
            let _ = writeln!(
                output,
                "        $this->client->{}({invocation});",
                method_name(&operation.id)
            );
            output.push_str("    }\n\n");
        } else {
            let _ = writeln!(
                output,
                "        return $this->client->{}({invocation});",
                method_name(&operation.id)
            );
            output.push_str("    }\n\n");
        }
        if page_pagination::render(api, operation, named_types).is_some()
            || cursor_pagination(api, operation).is_some()
            || offset_pagination(operation).is_some()
            || url_pagination(operation).is_some()
        {
            let paginator_method = facade_method
                .strip_suffix("Operation")
                .unwrap_or(facade_method)
                .to_owned()
                + "Pages";
            let _ = writeln!(
                output,
                "    /** @return \\Generator<int, {return_type}> */\n    public function {paginator_method}({declaration}): \\Generator\n    {{\n        return $this->client->{}Pages({invocation});\n    }}\n",
                method_name(&operation.id)
            );
        }
    }
    output.push_str("}\n");
    output
}

pub(super) fn facade_arguments(
    operation: &Operation,
    named_types: &NamedTypes,
) -> Vec<(String, String)> {
    let body_schema = operation
        .request_body
        .as_ref()
        .and_then(|body| body.media_types.first())
        .and_then(|media| media.schema.as_ref());
    let mut parameters = operation.parameters.clone();
    parameters.sort_by_key(|parameter| !parameter.required);
    let mut arguments = Vec::new();
    let mut used = BTreeSet::new();
    if operation.request_body.is_some() {
        used.insert("body".to_owned());
    }
    for parameter in &parameters {
        let variable = unique_name(property_name(&parameter.name), &mut used);
        let type_name = parameter
            .schema
            .as_ref()
            .map(|schema| php_type(schema, named_types))
            .unwrap_or_else(|| "mixed".into());
        let type_name = nullable_type(&type_name, !parameter.required);
        let default = if parameter.required { "" } else { " = null" };
        arguments.push((
            variable.clone(),
            format!("{type_name} ${variable}{default}"),
        ));
    }
    if let Some(schema) = body_schema {
        let type_name = if operation_has_multipart(operation) {
            "mixed".into()
        } else {
            php_type(schema, named_types)
        };
        let required = operation
            .request_body
            .as_ref()
            .is_some_and(|body| body.required);
        let default = if required { "" } else { " = null" };
        let type_name = nullable_type(&type_name, !required);
        let body = ("body".into(), format!("{type_name} $body{default}"));
        if required {
            let first_optional = operation
                .parameters
                .iter()
                .filter(|parameter| parameter.required)
                .count();
            arguments.insert(first_optional, body);
        } else {
            arguments.push(body);
        }
    }
    arguments
}
