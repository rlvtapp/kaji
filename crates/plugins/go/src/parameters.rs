use super::*;

pub(super) fn parameter_field_name(
    operation: &Operation,
    parameter: &OperationParameter,
) -> String {
    let preferred: Vec<_> = operation
        .parameters
        .iter()
        .map(|parameter| go_type_name(&parameter.name))
        .collect();
    let mut reserved: BTreeSet<_> = preferred.iter().cloned().collect();
    let mut used = BTreeSet::new();
    if operation.request_body.is_some() {
        reserved.insert("Body".into());
        used.insert("Body".into());
    }
    for (index, candidate) in operation.parameters.iter().enumerate() {
        let base = &preferred[index];
        let collision =
            preferred.iter().filter(|name| *name == base).count() > 1 || used.contains(base);
        let mut name = if collision {
            format!("{}{base}", go_type_name(&candidate.location))
        } else {
            base.clone()
        };
        let seed = name.clone();
        let mut suffix = 2;
        while used.contains(&name) || (name != *base && reserved.contains(&name)) {
            name = format!("{seed}{suffix}");
            suffix += 1;
        }
        used.insert(name.clone());
        if std::ptr::eq(candidate, parameter) {
            return name;
        }
    }
    unreachable!("parameter belongs to the operation")
}

pub(super) fn render_parameter_field(
    output: &mut String,
    parameter: &OperationParameter,
    name: &str,
) {
    let mut value_type = parameter
        .schema
        .as_ref()
        .map(go_type)
        .unwrap_or_else(|| "any".into());
    if !parameter.required && parameter.location != "path" && !value_type.starts_with('*') {
        value_type = format!("*{value_type}");
    }
    let _ = writeln!(
        output,
        "\t{name} {value_type} `json:\"{},omitempty\"`",
        parameter.name
    );
}

pub(super) fn render_parameter_use(
    output: &mut String,
    parameter: &OperationParameter,
    name: &str,
    response_kind: &GoResponseKind,
) {
    let field = format!("input.{name}");
    if parameter.location != "querystring" {
        if let Some(content) = poolster_core::openapi32::parameter_content(parameter)
            .expect("validated parameter content")
            .first()
        {
            let value = format!("poolsterContent{name}");
            let present = format!("poolsterPresent{name}");
            let error = format!("poolsterError{name}");
            let _ = writeln!(
                output,
                "\t{value}, {present}, {error} := poolsterParameterContent({field}, {:?})\n\tif {error} != nil {{",
                content.content_type
            );
            render_error_return(output, response_kind, &error);
            output.push_str("\t}\n");
            let _ = writeln!(output, "\tif {present} {{");
            match parameter.location.as_str() {
                "query" => {
                    let _ = writeln!(output, "\t\tquery.Add({:?}, {value})", parameter.name);
                }
                "path" => {
                    let _ = writeln!(
                        output,
                        "\t\tpath = strings.ReplaceAll(path, {:?}, url.PathEscape({value}))",
                        format!("{{{}}}", parameter.name)
                    );
                }
                "header" => {
                    let _ = writeln!(output, "\t\theaders.Set({:?}, {value})", parameter.name);
                }
                "cookie" => {
                    let _ = writeln!(
                        output,
                        "\t\theaders.Add(\"Cookie\", {:?} + \"=\" + strings.ReplaceAll(url.QueryEscape({value}), \"+\", \"%20\"))",
                        parameter.name
                    );
                }
                _ => {}
            }
            output.push_str("\t}\n");
            return;
        }
    }
    match parameter.location.as_str() {
        "path" => {
            let _ = writeln!(
                output,
                "\tpath = strings.ReplaceAll(path, {:?}, url.PathEscape(fmt.Sprint({field})))",
                format!("{{{}}}", parameter.name)
            );
        }
        "querystring" => {
            let content = poolster_core::openapi32::parameter_content(parameter)
                .expect("validated content metadata")
                .into_iter()
                .next()
                .unwrap_or_else(|| poolster_core::openapi32::ContentDefinition {
                    content_type: "text/plain".into(),
                    ..Default::default()
                });
            let content_type = content.content_type.clone();
            let metadata = serde_json::to_string(&content).expect("typed content metadata");
            let _ = writeln!(
                output,
                "\tencodedQuery, queryError := poolsterWholeQuery({field}, {content_type:?}, {metadata:?})\n\tif queryError != nil {{"
            );
            render_error_return(output, response_kind, "queryError");
            output.push_str("\t}\n\twholeQuery = encodedQuery\n");
        }
        "query" => {
            let _ = writeln!(
                output,
                "\tif err := addQuery(query, {:?}, {field}); err != nil {{",
                parameter.name
            );
            render_error_return(output, response_kind, "err");
            output.push_str("\t}\n");
        }
        "header" => {
            if parameter.required {
                let _ = writeln!(
                    output,
                    "\theaders.Set({:?}, fmt.Sprint({field}))",
                    parameter.name
                );
            } else {
                let _ = writeln!(
                    output,
                    "\tif {field} != nil {{ headers.Set({:?}, fmt.Sprint(*{field})) }}",
                    parameter.name
                );
            }
        }
        _ => {}
    }
}
