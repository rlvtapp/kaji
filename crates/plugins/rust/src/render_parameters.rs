//! HTTP parameters rendering.
use super::*;

pub(crate) fn operation_has_parameters(operation: &Operation) -> bool {
    !operation.parameters.is_empty()
}

pub(crate) fn operation_request_name(operation: &Operation) -> String {
    format!("{}Request", type_name(&operation.id))
}

pub(crate) fn render_operation_request(operation: &Operation) -> String {
    let all_optional = operation
        .parameters
        .iter()
        .all(|parameter| !parameter.required);
    let derive = if all_optional {
        "#[derive(Clone, Debug, Default)]"
    } else {
        "#[derive(Clone, Debug)]"
    };
    let mut output = format!(
        "{derive}\npub struct {} {{\n",
        operation_request_name(operation)
    );
    for parameter in &operation.parameters {
        let name = parameter_name(parameter);
        let mut value_type = if parameter.location == "querystring" {
            "String".into()
        } else {
            parameter
                .schema
                .as_ref()
                .map(rust_type)
                .unwrap_or_else(|| "serde_json::Value".into())
        };
        if !parameter.required && !value_type.starts_with("Option<") {
            value_type = format!("Option<{value_type}>");
        }
        let _ = writeln!(output, "    pub {name}: {value_type},");
    }
    output.push_str("}\n\n");
    output
}

pub(crate) fn parameter_json_content(parameter: &OperationParameter) -> bool {
    poolster_core::openapi32::parameter_content(parameter)
        .ok()
        .and_then(|items| items.into_iter().next())
        .is_some_and(|content| {
            content.content_type == "application/json" || content.content_type.ends_with("+json")
        })
}
pub(crate) fn render_parameter_use(output: &mut String, parameter: &OperationParameter) {
    let field = format!("input.{}", parameter_name(parameter));
    if parameter_json_content(parameter) && matches!(parameter.location.as_str(), "path" | "query")
    {
        let value = if parameter.required {
            format!("Some(&{field})")
        } else {
            format!("{field}.as_ref()")
        };
        let expression = if parameter.location == "path" {
            format!(
                "path=path.replace({:?},&poolster_path_segment(&poolster_parameter_json(value)));",
                format!("{{{}}}", parameter.name)
            )
        } else {
            format!(
                "query.push(({:?}.to_owned(),poolster_parameter_json(value)));",
                parameter.name
            )
        };
        let _ = writeln!(
            output,
            "        if let Some(value)={value} {{{expression}}}"
        );
        return;
    }
    match parameter.location.as_str() {
        "path" => {
            if parameter.required {
                let _ = writeln!(
                    output,
                    "        path = path.replace({:?}, &poolster_path_segment(&poolster_query_value(&{field})));",
                    format!("{{{}}}", parameter.name),
                );
            } else {
                let _ = writeln!(
                    output,
                    "        if let Some(value) = {field}.as_ref() {{ path = path.replace({:?}, &poolster_path_segment(&poolster_query_value(value))); }}",
                    format!("{{{}}}", parameter.name),
                );
            }
        }
        "query" => {
            let style = parameter
                .annotations
                .get("style")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("form");
            let explode = parameter
                .annotations
                .get("explode")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(style == "form");
            if parameter.required {
                let _ = writeln!(
                    output,
                    "        query.extend(poolster_query_pairs({:?}, &{field}, {style:?}, {explode}));",
                    parameter.name,
                );
            } else {
                let _ = writeln!(
                    output,
                    "        if let Some(value) = {field}.as_ref() {{ query.extend(poolster_query_pairs({:?}, value, {style:?}, {explode})); }}",
                    parameter.name,
                );
            }
        }
        _ => {}
    }
}

pub(crate) fn render_header_use(output: &mut String, parameter: &OperationParameter) {
    let field = format!("input.{}", parameter_name(parameter));
    if parameter.location == "cookie" {
        let serializer = if parameter_json_content(parameter) {
            "poolster_parameter_json"
        } else {
            "poolster_query_value"
        };
        let value = if parameter.required {
            format!("Some(&{field})")
        } else {
            format!("{field}.as_ref()")
        };
        let _ = writeln!(
            output,
            "        if let Some(value)={value} {{request=request.header(reqwest::header::COOKIE,format!(\"{{}}={{}}\",{:?},poolster_path_segment(&{serializer}(value))));}}",
            parameter.name
        );
        return;
    }
    let serializer = if parameter_json_content(parameter) {
        "poolster_parameter_json"
    } else {
        "poolster_query_value"
    };
    if parameter.required {
        let _ = writeln!(
            output,
            "        request = request.header({:?}, {serializer}(&{field}));",
            parameter.name,
        );
    } else {
        let _ = writeln!(
            output,
            "        if let Some(value) = {field}.as_ref() {{ request = request.header({:?}, {serializer}(value)); }}",
            parameter.name,
        );
    }
}
