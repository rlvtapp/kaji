//! Native page-number iteration from core's validated pagination plan.
use super::*;
#[derive(Clone)]
struct Input {
    field: String,
    required: bool,
}
fn input(
    api: &Api,
    operation: &Operation,
    plan: &kaji_core::pagination::PaginationPlan,
    role: &str,
) -> Option<Input> {
    let input = plan
        .inputs
        .iter()
        .find(|input| input.role == role && input.location != "requestBody")?;
    let parameter = operation
        .parameters
        .iter()
        .find(|parameter| parameter.name == input.name)?;
    let mut value = parameter.schema.as_ref()?;
    let mut seen = BTreeSet::new();
    loop {
        if value.nullable || value.nullish {
            return None;
        }
        let Some(name) = value.kind.reference_name() else {
            break;
        };
        if !seen.insert(name) {
            return None;
        }
        value = &api.schemas.iter().find(|schema| schema.name == name)?.value;
    }
    if !matches!(value.kind, SchemaKind::Integer) {
        return None;
    }
    Some(Input {
        field: parameter_field_name(operation, parameter),
        required: input.required,
    })
}
fn plan(api: &Api, operation: &Operation) -> Option<(Input, Option<Input>, String)> {
    let plan = kaji_core::pagination::normalize_pagination(api, operation, None)
        .ok()
        .flatten()?;
    if plan.kind != kaji_core::pagination::PaginationKind::Page {
        return None;
    }
    let page = input(api, operation, &plan, "page")?;
    let limit = if plan.inputs.iter().any(|input| input.role == "limit") {
        Some(input(api, operation, &plan, "limit")?)
    } else {
        None
    };
    Some((page, limit, native_selector(plan.results.as_ref()?)?))
}
pub(super) fn supported(api: &Api, operation: &Operation) -> bool {
    matches!(operation_response_kind(operation), GoResponseKind::Json(_))
        && plan(api, operation).is_some()
}
pub(super) fn render(output: &mut String, api: &Api, operation: &Operation) {
    let Some((page, limit, selector)) = plan(api, operation) else {
        return;
    };
    let GoResponseKind::Json(response_type) = operation_response_kind(operation) else {
        return;
    };
    let name = go_type_name(&operation.id);
    let request = format!("{name}Request");
    let pager = format!("{name}Pager");
    let page_start = if page.required {
        format!("page = copyInput.{}", page.field)
    } else {
        format!(
            "if copyInput.{} != nil {{ page = *copyInput.{} }}",
            page.field, page.field
        )
    };
    let limit_start = match &limit {
        None => String::new(),
        Some(input) if input.required => format!(
            "limitValue := copyInput.{}; limit = &limitValue",
            input.field
        ),
        Some(input) => format!(
            "if copyInput.{} != nil {{ limitValue := *copyInput.{}; limit = &limitValue; copyInput.{} = &limitValue }}",
            input.field, input.field, input.field
        ),
    };
    let page_update = if page.required {
        format!("pager.input.{} = pager.page", page.field)
    } else {
        format!("pager.input.{} = &pager.page", page.field)
    };
    let _ = writeln!(
        output,
        r#"
// {pager} yields declared page-number response pages. Optional page defaults to 1.
type {pager} struct {{ client *Client; input *{request}; page int64; limit *int64; done bool; nextError error }}
// {name}Pages copies the caller's inputs. Without a limit it stops on an empty page.
func (client *Client) {name}Pages(input *{request}) *{pager} {{
    copyInput := &{request}{{}}; if input != nil {{ *copyInput = *input }}
    page := int64(1); {page_start}
    var limit *int64; {limit_start}
    return &{pager}{{client:client,input:copyInput,page:page,limit:limit}}
}}
// Next returns a full page, then io.EOF after an empty or shorter-than-limit page.
func (pager *{pager}) Next(ctx context.Context) (*{response_type},error) {{
    if pager.nextError != nil {{ return nil,pager.nextError }}
    if pager.done {{ return nil,io.EOF }}
    if pager.limit != nil && *pager.limit <= 0 {{ return nil,fmt.Errorf("kaji: page pagination limit must be positive") }}
    {page_update}
    response,err := pager.client.{name}(ctx,pager.input); if err != nil {{ return nil,err }}
    count,ok := kajiPaginationArrayLen(response,{selector:?})
    if !ok {{ return nil,fmt.Errorf("kaji: page pagination results must be an array") }}
    if count == 0 || (pager.limit != nil && int64(count) < *pager.limit) {{ pager.done = true; return response,nil }}
    if pager.page == int64(^uint64(0)>>1) {{ pager.nextError = fmt.Errorf("kaji: page pagination number overflow"); return response,nil }}
    pager.page++
    return response,nil
}}
"#
    );
}

pub(super) fn documentation(api: &Api) -> String {
    let mut output = String::new();
    for operation in &api.operations {
        let Ok(Some(normalized)) =
            kaji_core::pagination::normalize_pagination(api, operation, None)
        else {
            continue;
        };
        if normalized.kind != kaji_core::pagination::PaginationKind::Page {
            continue;
        }
        if supported(api, operation) {
            let name = go_type_name(&operation.id);
            let _ = writeln!(
                output,
                "- `{name}Pages(input)` returns a pager; `Next(ctx)` yields each full response, then `io.EOF`. Optional page starts at 1; an explicit start is preserved. An empty result array ends iteration. With a positive limit, a shorter result array also ends iteration. Inputs are copied, and pagination controls do not mutate the caller's values. Missing/non-array results and counter overflow return errors."
            );
        } else {
            let _ = writeln!(
                output,
                "- `{}` has a validated page declaration, but no native page helper is emitted: this binding currently requires a JSON response and nonnullable integer controls in query/header/path parameters. Request-body controls and nullable integer aliases need a separate native binding. The direct operation remains available.",
                operation.id.replace('`', "")
            );
        }
    }
    if output.is_empty() {
        output
    } else {
        format!("\n## Page-number pagination\n\n{output}")
    }
}
