use super::*;
use poolster_core::pagination::{PaginationKind, SelectorSegment, normalize_pagination};

pub(super) fn render(api: &Api) -> Result<String> {
    let mut output = super::cursor_pagination::render(api)?;
    for operation in &api.operations {
        let extension = poolster_core::poolster_extension(&operation.annotations, "pagination")
            .or_else(|| operation.annotations.get("x-speakeasy-pagination"));
        let kind = extension
            .and_then(|value| value.get("type"))
            .and_then(serde_json::Value::as_str);
        if !matches!(kind, Some("page" | "offsetLimit" | "url")) {
            continue;
        }
        let Some(plan) = normalize_pagination(api, operation, None)? else {
            continue;
        };
        if plan.kind == PaginationKind::Url {
            render_url(&mut output, operation, &plan)?;
            continue;
        }
        let offset = plan.kind == PaginationKind::OffsetLimit;
        let page = plan
            .inputs
            .iter()
            .find(|input| input.role == if offset { "offset" } else { "page" })
            .unwrap();
        let limit = plan.inputs.iter().find(|input| input.role == "limit");
        // Body bindings need immutable model copying, which this native API does not provide.
        if plan
            .inputs
            .iter()
            .any(|input| input.location == "requestBody")
        {
            bail!(
                "Swift page pagination for {} requires parameter controls",
                operation.id
            );
        }
        for input in &plan.inputs {
            let parameter = operation
                .parameters
                .iter()
                .find(|parameter| parameter.name == input.name)
                .unwrap();
            let value = parameter.schema.as_ref().unwrap();
            if value.nullable
                || value.optional
                || value.nullish
                || !value.enum_values.is_empty()
                || !matches!(value.kind, SchemaKind::Integer)
            {
                bail!(
                    "Swift page controls must be nonnullable inline integer scalars: {}",
                    input.name
                );
            }
        }
        let page_name = identifier(&page.name);
        let initial = if page.required {
            page_name.clone()
        } else {
            format!("{page_name} ?? {}", if offset { 0 } else { 1 })
        };
        let limit_expr = limit
            .map(|input| identifier(&input.name))
            .unwrap_or_else(|| "nil".into());
        let parameters = operation_parameters(operation);
        let signature = parameters
            .iter()
            .map(|parameter| parameter.signature.clone())
            .collect::<Vec<_>>()
            .join(", ");
        let args = parameters
            .iter()
            .map(|parameter| {
                let name = parameter.signature.split(':').next().unwrap();
                format!(
                    "{name}: {}",
                    if name == page_name { "current" } else { name }
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let response = swift_type(operation.success_schema().unwrap(), false);
        let selectors = plan
            .results
            .unwrap()
            .segments
            .iter()
            .map(|segment| match segment {
                SelectorSegment::Field(field) => {
                    format!(".field({})", swift_literal(field))
                }
                SelectorSegment::Index(index) => format!(".index({index})"),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let name = function_name(&operation.id);
        writeln!(
            output,
            "    func {name}Pages({signature}) -> PoolsterPageSequence<{response}> {{\n        PoolsterPageSequence(page: {initial}, limit: {limit_expr}, offset: {offset}) {{ current in\n            let response = try await self.{name}({args})\n            return (response, try poolsterPageCount(response, [{selectors}]))\n        }}\n    }}"
        )?;
    }
    Ok(output)
}

fn render_url(
    output: &mut String,
    operation: &Operation,
    plan: &poolster_core::pagination::PaginationPlan,
) -> Result<()> {
    anyhow::ensure!(
        operation
            .responses
            .iter()
            .filter(|response| response.status.starts_with('2'))
            .flat_map(|response| &response.media_types)
            .all(|media| media.content_type == "application/json"
                || media.content_type.ends_with("+json")),
        "Swift URL pagination requires buffered JSON responses"
    );
    let parameters = operation_parameters(operation);
    let signature = parameters
        .iter()
        .map(|parameter| parameter.signature.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let args = parameters
        .iter()
        .map(|parameter| {
            let name = parameter.signature.split(':').next().unwrap();
            format!("{name}: {name}")
        })
        .collect::<Vec<_>>()
        .join(", ");
    let args = if args.is_empty() {
        String::new()
    } else {
        format!("{args}, ")
    };
    let response = swift_type(operation.success_schema().unwrap(), false);
    let name = function_name(&operation.id);
    let selectors = plan
        .continuation
        .as_ref()
        .unwrap()
        .segments
        .iter()
        .map(|segment| match segment {
            SelectorSegment::Field(field) => format!(".field({})", swift_literal(field)),
            SelectorSegment::Index(index) => format!(".index({index})"),
        })
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(
        output,
        "    func {name}Pages({signature}) -> PoolsterURLSequence<{response}> {{\n        PoolsterURLSequence {{ nextURL in\n            let response = try await self.{name}PoolsterURL({args}_poolsterURL: nextURL)\n            return (response, try poolsterNextURL(response, [{selectors}]))\n        }}\n    }}"
    )?;
    Ok(())
}

pub(super) fn swift_literal(value: &str) -> String {
    let mut result = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            ch if ch.is_control() => {
                let _ = write!(result, "\\u{{{:x}}}", ch as u32);
            }
            ch => result.push(ch),
        }
    }
    result.push('"');
    result
}

#[cfg(test)]
#[path = "pagination/tests.rs"]
mod tests;
