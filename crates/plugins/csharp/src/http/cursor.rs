//! Cursor emission for the csharp HTTP SDK.
use crate::*;

pub(crate) fn dotnet_cursor_pagination(operation: &Operation) -> Option<String> {
    let extension = poolster_core::poolster_extension(&operation.annotations, "pagination")
        .or_else(|| operation.annotations.get("x-speakeasy-pagination"))?
        .as_object()?;
    if extension.get("type")?.as_str()? != "cursor"
        || !matches!(
            operation_response_surface(operation),
            DotnetResponseSurface::Json(_)
        )
    {
        return None;
    }
    let input =
        extension.get("inputs")?.as_array()?.iter().find(|input| {
            input.get("type").and_then(serde_json::Value::as_str) == Some("cursor")
        })?;
    let name = input.get("name")?.as_str()?;
    let parameter = operation.parameters.iter().find(|parameter| {
        parameter.name == name
            && matches!(parameter.location.as_str(), "query" | "header" | "path")
            && (parameter.location != "path" || parameter.required)
    })?;
    if !matches!(parameter.schema.as_ref()?.kind, SchemaKind::String) {
        return None;
    }
    let selector = extension.get("outputs")?.get("nextCursor")?.as_str()?;
    poolster_core::pagination::Selector::parse(selector).ok()?;
    Some(selector.to_owned())
}

pub(crate) fn render_cursor_pager(
    output: &mut String,
    operation: &Operation,
    next_cursor_path: &str,
) {
    let name = pascal_case(&operation.id);
    let DotnetResponseSurface::Json(response) = operation_response_surface(operation) else {
        return;
    };
    let cursor_name = poolster_core::poolster_extension(&operation.annotations, "pagination")
        .or_else(|| operation.annotations.get("x-speakeasy-pagination"))
        .and_then(serde_json::Value::as_object)
        .and_then(|extension| extension.get("inputs"))
        .and_then(serde_json::Value::as_array)
        .and_then(|inputs| {
            inputs.iter().find(|input| {
                input.get("type").and_then(serde_json::Value::as_str) == Some("cursor")
            })
        })
        .and_then(|input| input.get("name"))
        .and_then(serde_json::Value::as_str)
        .map(camel_case)
        .expect("validated cursor pagination");
    let parameters = facade_parameters(operation);
    let mut arguments = facade_arguments(operation);
    if let Some(argument) = arguments
        .iter_mut()
        .find(|argument| *argument == &cursor_name)
    {
        *argument = "currentCursor".into();
    }
    let operation_id = &operation.id;
    let parameters = parameters[..parameters.len() - 1].join(", ");
    let arguments = arguments.join(", ");
    let _ = writeln!(
        output,
        "    /// <summary>Lazily follows the declared cursor pagination contract for {operation_id}.</summary>\n    public async IAsyncEnumerable<{response}> {name}PagesAsync({parameters}, [System.Runtime.CompilerServices.EnumeratorCancellation] CancellationToken cancellationToken = default)\n    {{\n  var currentCursor = {cursor_name};\n  \n        for (var pageIndex = 0; pageIndex < 10000; pageIndex++)\n        {{\n    cancellationToken.ThrowIfCancellationRequested();\n    \n            var page = await {name}Async({arguments}).ConfigureAwait(false);\n    \n            yield return page;\n    \n            var nextCursor = PoolsterJsonPath(JsonSerializer.SerializeToElement(page, JsonOptions), {next_cursor_path:?});\n    \n            if (string.IsNullOrEmpty(nextCursor) || nextCursor == currentCursor) yield break;\n    \n            currentCursor = nextCursor;\n    \n\n  }}\n  \n\n}}\n\n"
    );
}
