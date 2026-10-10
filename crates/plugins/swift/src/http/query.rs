//! Query emission for the Swift HTTP SDK.
use crate::*;

pub(crate) fn render_query_parameters(
    operation: &Operation,
    query_binding: &str,
    indent: &str,
) -> String {
    let mut output = String::new();
    for parameter in operation
        .parameters
        .iter()
        .filter(|parameter| parameter.location == "query")
    {
        let value = parameter_name(parameter);
        if parameter_json_content(parameter) {
            let value = parameter_name(parameter);
            let expression = if parameter.required {
                format!(
                    "{indent}    {query_binding}.append(URLQueryItem(name:{:?},value:try jsonParameter({value})))",
                    parameter.name
                )
            } else {
                format!(
                    "{indent}    if let value={value} {{{query_binding}.append(URLQueryItem(name:{:?},value:try jsonParameter(value)))}}",
                    parameter.name
                )
            };
            let _ = writeln!(output, "{expression}");
            continue;
        }
        let required = parameter.required
            && !parameter
                .schema
                .as_ref()
                .is_some_and(|schema| schema.nullable);
        let array = parameter
            .schema
            .as_ref()
            .is_some_and(|schema| matches!(schema.kind, SchemaKind::Array { .. }));
        let local = if required {
            value.clone()
        } else {
            "value".into()
        };
        let nested = if required {
            format!("{indent}    ")
        } else {
            format!("{indent}        ")
        };
        if !required {
            let _ = writeln!(output, "{indent}    if let value = {value} {{");
        }
        if array {
            if parameter
                .annotations
                .get("explode")
                .and_then(serde_json::Value::as_bool)
                == Some(false)
            {
                let mut joined = "__poolsterJoinedValues".to_owned();
                while operation
                    .parameters
                    .iter()
                    .any(|p| parameter_name(p) == joined)
                {
                    joined.push_str("Values");
                }
                let _ = writeln!(output, "{nested}do {{");
                let _ = writeln!(output, "{nested}    var {joined}: [String] = []");
                let _ = writeln!(
                    output,
                    "{nested}    for item in {local} {{ {joined}.append(String(describing: item)) }}"
                );
                let _ = writeln!(
                    output,
                    r#"{nested}    {query_binding}.append(URLQueryItem(name: {:?}, value: {joined}.joined(separator: ",")))"#,
                    parameter.name
                );
                let _ = writeln!(output, "{nested}}}");
            } else {
                let _ = writeln!(
                    output,
                    "{nested}for item in {local} {{ {query_binding}.append(URLQueryItem(name: {:?}, value: String(describing: item))) }}",
                    parameter.name
                );
            }
        } else {
            let _ = writeln!(
                output,
                "{nested}{query_binding}.append(URLQueryItem(name: {:?}, value: String(describing: {local})))",
                parameter.name
            );
        }
        if !required {
            let _ = writeln!(output, "{indent}    }}");
        }
    }
    output
}
