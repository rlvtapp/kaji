use super::*;
use poolster_core::pagination::{PaginationPlan, normalize_pagination};
pub(crate) fn plan(api: &Api, operation: &Operation) -> anyhow::Result<Option<PaginationPlan>> {
    let Some(raw) = pagination_annotation(operation) else {
        return Ok(None);
    };
    if raw.get("type").and_then(Value::as_str) != Some("page") {
        anyhow::ensure!(
            matches!(
                raw.get("type").and_then(Value::as_str),
                Some("cursor" | "offsetLimit" | "url")
            ),
            "unsupported PHP pagination type"
        );
        return Ok(None);
    }
    let plan = normalize_pagination(api, operation, None)?;
    if let Some(plan) = &plan {
        if plan
            .inputs
            .iter()
            .any(|input| input.location == "requestBody")
        {
            anyhow::ensure!(
                operation
                    .request_body
                    .as_ref()
                    .is_some_and(|body| body.required),
                "body pagination requires a required JSON body"
            );
        }
    }
    Ok(plan)
}
fn variable(operation: &Operation, name: &str, location: &str) -> String {
    let mut parameters = operation.parameters.clone();
    parameters.sort_by_key(|p| !p.required);
    let mut used = BTreeSet::new();
    if operation.request_body.is_some() {
        used.insert("body".to_owned());
    }
    for parameter in &parameters {
        let variable = unique_name(property_name(&parameter.name), &mut used);
        if parameter.name == name && parameter.location == location {
            return variable;
        }
    }
    property_name(name)
}
pub(crate) fn render(api: &Api, operation: &Operation, named: &NamedTypes) -> Option<String> {
    let plan = plan(api, operation).ok()??;
    let page = plan.inputs.iter().find(|input| input.role == "page")?;
    let limit = plan.inputs.iter().find(|input| input.role == "limit");
    let selector = &plan.results?.expression;
    let arguments = facade_arguments(operation, named);
    let declaration = arguments
        .iter()
        .map(|(_, declaration)| declaration.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let page_var = variable(operation, &page.name, &page.location);
    let state = if page.location == "requestBody" {
        format!(
            "        $_poolsterPage = self::poolsterJsonPath($body, {}) ?? 1;\n        $_poolsterBody = self::poolsterWithBodyValue($body, {}, $_poolsterPage);\n",
            php_string(&format!(
                "/{}",
                page.name.replace('~', "~0").replace('/', "~1")
            )),
            php_string(&page.name)
        )
    } else {
        format!("        $_poolsterPage = ${page_var} ?? 1;\n")
    };
    let invocation = arguments
        .iter()
        .map(|(variable, _)| {
            if page.location == "requestBody" && variable == "body" {
                "$_poolsterBody".into()
            } else if page.location != "requestBody" && variable == &page_var {
                "$_poolsterPage".into()
            } else {
                format!("${variable}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let limit = limit
        .map(|input| {
            if input.location == "requestBody" {
                format!(
                    "self::poolsterJsonPath($_poolsterBody, {})",
                    php_string(&format!(
                        "/{}",
                        input.name.replace('~', "~0").replace('/', "~1")
                    ))
                )
            } else {
                format!("${}", variable(operation, &input.name, &input.location))
            }
        })
        .unwrap_or("null".into());
    let update = if page.location == "requestBody" {
        format!(
            "            $_poolsterBody = self::poolsterWithBodyValue($_poolsterBody, {}, $_poolsterPage);\n",
            php_string(&page.name)
        )
    } else {
        String::new()
    };
    Some(format!(
        "    /** Yield declared page responses without mutating caller input. */\n    public function {name}Pages({declaration}): \\Generator\n    {{\n{state}        if (!is_int($_poolsterPage) || $_poolsterPage < 0) {{ throw new \\InvalidArgumentException('page must be a nonnegative integer'); }}\n        for ($_poolsterCount = 0; $_poolsterCount < 10000; $_poolsterCount++) {{\n            $_poolsterResponse = $this->{name}({invocation});\n            $_poolsterResults = self::poolsterJsonPath($_poolsterResponse, {selector});\n            if (!is_array($_poolsterResults) || !array_is_list($_poolsterResults)) {{ throw new \\TypeError('pagination results must be an array'); }}\n            yield $_poolsterResponse;\n            $_poolsterLimit = {limit};\n            if ($_poolsterResults === [] || (is_int($_poolsterLimit) && $_poolsterLimit > 0 && count($_poolsterResults) < $_poolsterLimit)) {{ return; }}\n            if ($_poolsterPage === PHP_INT_MAX) {{ throw new \\OverflowException('page exceeds the integer range'); }}\n            $_poolsterPage++;\n{update}        }}\n        throw new \\RuntimeException('pagination exceeded 10000 pages');\n    }}\n\n",
        name = method_name(&operation.id),
        selector = php_string(selector)
    ))
}
