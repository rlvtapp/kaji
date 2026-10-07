use super::*;
use kaji_core::pagination::{PaginationPlan, normalize_pagination};
pub(crate) fn plan(api: &Api, operation: &Operation) -> anyhow::Result<Option<PaginationPlan>> {
    let Some(raw) = operation
        .annotations
        .get("x-kaji-pagination")
        .or_else(|| operation.annotations.get("x-speakeasy-pagination"))
    else {
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
fn variable(operation: &Operation, name: &str) -> String {
    let mut parameters = operation.parameters.clone();
    parameters.sort_by_key(|p| !p.required);
    let mut used = BTreeSet::new();
    for parameter in &parameters {
        let variable = unique_name(property_name(&parameter.name), &mut used);
        if parameter.name == name {
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
    let page_var = variable(operation, &page.name);
    let state = if page.location == "requestBody" {
        format!(
            "        $_kajiPage = self::kajiJsonPath($body, {}) ?? 1;\n        $_kajiBody = self::kajiWithBodyValue($body, {}, $_kajiPage);\n",
            php_string(&format!(
                "/{}",
                page.name.replace('~', "~0").replace('/', "~1")
            )),
            php_string(&page.name)
        )
    } else {
        format!("        $_kajiPage = ${page_var} ?? 1;\n")
    };
    let invocation = arguments
        .iter()
        .map(|(variable, _)| {
            if page.location == "requestBody" && variable == "body" {
                "$_kajiBody".into()
            } else if page.location != "requestBody" && variable == &page_var {
                "$_kajiPage".into()
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
                    "self::kajiJsonPath($_kajiBody, {})",
                    php_string(&format!(
                        "/{}",
                        input.name.replace('~', "~0").replace('/', "~1")
                    ))
                )
            } else {
                format!("${}", variable(operation, &input.name))
            }
        })
        .unwrap_or("null".into());
    let update = if page.location == "requestBody" {
        format!(
            "            $_kajiBody = self::kajiWithBodyValue($_kajiBody, {}, $_kajiPage);\n",
            php_string(&page.name)
        )
    } else {
        String::new()
    };
    Some(format!(
        "    /** Yield declared page responses without mutating caller input. */\n    public function {name}Pages({declaration}): \\Generator\n    {{\n{state}        if (!is_int($_kajiPage) || $_kajiPage < 0) {{ throw new \\InvalidArgumentException('page must be a nonnegative integer'); }}\n        for ($_kajiCount = 0; $_kajiCount < 10000; $_kajiCount++) {{\n            $_kajiResponse = $this->{name}({invocation});\n            $_kajiResults = self::kajiJsonPath($_kajiResponse, {selector});\n            if (!is_array($_kajiResults) || !array_is_list($_kajiResults)) {{ throw new \\TypeError('pagination results must be an array'); }}\n            yield $_kajiResponse;\n            $_kajiLimit = {limit};\n            if ($_kajiResults === [] || (is_int($_kajiLimit) && $_kajiLimit > 0 && count($_kajiResults) < $_kajiLimit)) {{ return; }}\n            if ($_kajiPage === PHP_INT_MAX) {{ throw new \\OverflowException('page exceeds the integer range'); }}\n            $_kajiPage++;\n{update}        }}\n        throw new \\RuntimeException('pagination exceeded 10000 pages');\n    }}\n\n",
        name = method_name(&operation.id),
        selector = php_string(selector)
    ))
}
