//! Declared integer query pagination; all requests use the normal operation API.
use super::*;
use kaji_core::pagination::{PaginationKind, SelectorSegment, normalize_pagination};

/// Validate the renderer ABI in addition to the shared wire-level plan.
pub(super) fn validate(api: &Api, operation: &Operation) -> Result<()> {
    let Some(plan) = normalize_pagination(api, operation, None)? else {
        return Ok(());
    };
    if plan.kind == PaginationKind::Url {
        if !matches!(
            operation_response_surface(operation),
            DotnetResponseSurface::Json(_)
        ) {
            bail!("C# URL pagination requires buffered JSON response")
        }
        for parameter in &operation.parameters {
            if matches!(
                camel_case(&parameter.name).as_str(),
                "kajiURL"
                    | "paginationURL"
                    | "nextURL"
                    | "seenURLs"
                    | "responsePage"
                    | "pageIndex"
                    | "cancellationToken"
            ) {
                bail!("C# URL pagination parameter collides with native helper locals")
            }
        }
        return Ok(());
    }
    if !matches!(
        plan.kind,
        PaginationKind::Page | PaginationKind::OffsetLimit
    ) {
        return Ok(());
    }
    if !matches!(
        operation_response_surface(operation),
        DotnetResponseSurface::Json(_)
    ) {
        bail!(
            "C# pagination for {} requires a JSON response",
            operation.id
        );
    }
    for input in &plan.inputs {
        if input.location != "query" {
            bail!(
                "C# pagination for {} supports query inputs only; {} is {}",
                operation.id,
                input.name,
                input.location
            );
        }
        let parameter = operation
            .parameters
            .iter()
            .find(|parameter| parameter.name == input.name && parameter.location == "query")
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "C# pagination input {} is not a query parameter",
                    input.name
                )
            })?;
        let schema = parameter
            .schema
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("C# pagination input {} has no schema", input.name))?;
        if !matches!(schema.kind, SchemaKind::Integer)
            || !matches!(schema.format.as_deref(), None | Some("int32" | "int64"))
        {
            bail!(
                "C# pagination input {} requires a direct int32/int64 integer schema; references and custom formats are unsupported",
                input.name
            );
        }
        if parameter.required && (schema.nullable || schema.optional || schema.nullish) {
            bail!(
                "C# pagination input {} cannot be required and nullable/optional",
                input.name
            );
        }
    }
    for parameter in &operation.parameters {
        let name = camel_case(&parameter.name);
        if matches!(
            name.as_str(),
            "currentValue"
                | "pageLimit"
                | "responsePage"
                | "results"
                | "count"
                | "pageIndex"
                | "cancellationToken"
        ) || name.strip_prefix("index").is_some_and(|suffix| {
            !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
        }) {
            bail!(
                "C# pagination for {} has an unsupported control-name collision: {}",
                operation.id,
                parameter.name
            );
        }
    }
    Ok(())
}

pub(super) fn render(output: &mut String, api: &Api, operation: &Operation) {
    let Ok(Some(plan)) = normalize_pagination(api, operation, None) else {
        return;
    };
    if plan.kind == PaginationKind::Url {
        render_url(output, api, operation, &plan);
        return;
    }
    if !matches!(
        plan.kind,
        PaginationKind::Page | PaginationKind::OffsetLimit
    ) {
        return;
    }
    let role = if plan.kind == PaginationKind::Page {
        "page"
    } else {
        "offset"
    };
    let Some(input) = plan.inputs.iter().find(|input| input.role == role) else {
        return;
    };
    if plan.inputs.iter().any(|input| input.location != "query") {
        return;
    }
    let Some(selector) = plan.results else { return };
    let DotnetResponseSurface::Json(response) = operation_response_surface(operation) else {
        return;
    };
    if validate(api, operation).is_err() {
        return;
    }
    let parameter = operation
        .parameters
        .iter()
        .find(|parameter| parameter.name == input.name && parameter.location == "query")
        .expect("validated query input");
    let int32 = parameter
        .schema
        .as_ref()
        .is_some_and(|schema| schema.format.as_deref() == Some("int32"));
    let max_value = if int32 {
        "int.MaxValue"
    } else {
        "long.MaxValue"
    };
    let field = camel_case(&input.name);
    let name = pascal_case(&operation.id);
    let parameters = facade_parameters(operation);
    let parameters = parameters[..parameters.len() - 1].join(", ");
    let mut arguments = facade_arguments(operation);
    for argument in &mut arguments {
        if *argument == field {
            *argument = if int32 {
                "checked((int)currentValue)".into()
            } else {
                "currentValue".into()
            };
        }
    }
    let arguments = arguments.join(", ");
    let initial = if input.required {
        field.clone()
    } else {
        format!("{field} ?? {}", if role == "page" { 1 } else { 0 })
    };
    let limit = plan
        .inputs
        .iter()
        .find(|input| input.role == "limit")
        .map(|input| camel_case(&input.name));
    let limit = limit.unwrap_or_else(|| "null".into());
    let mut traversal = String::new();
    for segment in selector.segments {
        match segment {
            SelectorSegment::Field(field) => {
                let array_index = if (field == "0" || !field.starts_with('0'))
                    && field.bytes().all(|byte| byte.is_ascii_digit())
                {
                    field.parse::<i32>().ok()
                } else {
                    None
                };
                let field = serde_json::to_string(&field).unwrap();
                if let Some(index) = array_index {
                    let _ = writeln!(
                        traversal,
                        "            if (results.ValueKind == JsonValueKind.Array) {{ if ({index} >= results.GetArrayLength()) yield break; results = results[{index}]; }}\n            else if (results.ValueKind != JsonValueKind.Object || !results.TryGetProperty({field}, out results)) yield break;"
                    );
                    continue;
                }
                let _ = writeln!(
                    traversal,
                    "            if (results.ValueKind != JsonValueKind.Object || !results.TryGetProperty({field}, out results)) yield break;"
                );
            }
            SelectorSegment::Index(index) => {
                let _ = writeln!(
                    traversal,
                    "            if (results.ValueKind != JsonValueKind.Array) yield break;\n            var index{} = {}L < 0 ? results.GetArrayLength() + {}L : {}L;\n            if (index{} < 0 || index{} >= results.GetArrayLength()) yield break;\n            results = results[(int)index{}];",
                    traversal.len(),
                    index,
                    index,
                    index,
                    traversal.len(),
                    traversal.len(),
                    traversal.len()
                );
            }
        }
    }
    let increment = if role == "page" { "1" } else { "count" };
    let _ = writeln!(
        output,
        "    public async IAsyncEnumerable<{response}> {name}PagesAsync({parameters}, [System.Runtime.CompilerServices.EnumeratorCancellation] CancellationToken cancellationToken = default)\n    {{\n        long currentValue = {initial};\n        long? pageLimit = {limit};\n        if (currentValue < 0 || pageLimit <= 0) throw new ArgumentOutOfRangeException(\"pagination\");\n        for (var pageIndex = 0; pageIndex < 10000; pageIndex++)\n        {{\n            cancellationToken.ThrowIfCancellationRequested();\n            var responsePage = await {name}Async({arguments}).ConfigureAwait(false);\n            yield return responsePage;\n            var results = JsonSerializer.SerializeToElement(responsePage, JsonOptions);\n{traversal}            if (results.ValueKind != JsonValueKind.Array) yield break;\n            var count = results.GetArrayLength();\n            if (count == 0 || (pageLimit.HasValue && count < pageLimit.Value)) yield break;\n            if (currentValue > {max_value} - {increment}) yield break;\n            currentValue += {increment};\n        }}\n    }}\n"
    );
}

fn render_url(
    output: &mut String,
    api: &Api,
    operation: &Operation,
    plan: &kaji_core::pagination::PaginationPlan,
) {
    if validate(api, operation).is_err() {
        return;
    }
    let DotnetResponseSurface::Json(response) = operation_response_surface(operation) else {
        return;
    };
    let name = pascal_case(&operation.id);
    let parameters = facade_parameters(operation);
    let signature = parameters[..parameters.len() - 1].join(", ");
    let signature = if signature.is_empty() {
        String::new()
    } else {
        format!("{signature}, ")
    };
    let args = facade_arguments(operation).join(", ");
    let args = if args.is_empty() {
        String::new()
    } else {
        format!("{args}, ")
    };
    let path = &plan.continuation.as_ref().unwrap().expression;
    let _ = writeln!(
        output,
        r#"    /// <summary>Lazy absolute same-origin URL pages; relative URLs are rejected.</summary>
    public async IAsyncEnumerable<{response}> {name}PagesAsync({signature}[System.Runtime.CompilerServices.EnumeratorCancellation] CancellationToken cancellationToken=default) {{
        string? nextURL=null;
        var seenURLs=new HashSet<string>(StringComparer.Ordinal);
        for (var pageIndex=0;pageIndex<10000;pageIndex++) {{
            cancellationToken.ThrowIfCancellationRequested();
            if(nextURL is not null && !seenURLs.Add(nextURL)) throw new InvalidOperationException("Repeated continuation URL");
            var responsePage=await {name}KajiURLAsync({args}nextURL).ConfigureAwait(false);
            yield return responsePage;
            nextURL=KajiJsonPath(JsonSerializer.SerializeToElement(responsePage,JsonOptions),{path:?});
            if(string.IsNullOrEmpty(nextURL)) yield break;
        }}
        throw new InvalidOperationException("Pagination exceeded 10000 pages");
    }}
    private Uri {name}KajiURLTarget(string next) {{
        if(!Uri.TryCreate(next,UriKind.Absolute,out var uri) || (uri.Scheme!="http" && uri.Scheme!="https") || uri.Scheme!=_baseUri.Scheme || !string.Equals(uri.IdnHost,_baseUri.IdnHost,StringComparison.OrdinalIgnoreCase) || uri.Port!=_baseUri.Port || uri.UserInfo.Length!=0 || uri.Fragment.Length!=0) throw new InvalidOperationException("Unsafe continuation URL");
        return uri;
    }}
"#
    );
}
#[cfg(test)]
mod url_tests {
    use super::*;
    use kaji_core::{HttpMethod, OperationParameter, OperationResponse};
    fn api() -> Api {
        let mut operation = Operation {
            id: "listLinks".into(),
            method: HttpMethod::Get,
            path: "/links".into(),
            parameters: vec![OperationParameter {
                name: "tenant".into(),
                location: "query".into(),
                required: false,
                schema: Some(SchemaValue::new(SchemaKind::String)),
                description: None,
                annotations: Default::default(),
            }],
            responses: vec![OperationResponse::json(
                "200",
                SchemaValue::new(SchemaKind::Object {
                    fields: vec![kaji_core::Field {
                        name: "next".into(),
                        value: SchemaValue::new(SchemaKind::String),
                        required: false,
                        annotations: Default::default(),
                    }],
                    additional_properties: kaji_core::AdditionalProperties::Forbidden,
                }),
            )],
            ..Default::default()
        };
        operation.annotations.insert(
            "x-kaji-pagination".into(),
            serde_json::json!({"type":"url","outputs":{"nextUrl":"/next"}}),
        );
        Api {
            name: "Probe".into(),
            version: "1.0.0".into(),
            operations: vec![operation],
            ..Default::default()
        }
    }
    #[test]
    fn url_helpers_use_private_guarded_operation_and_keep_public_signature() {
        let api = api();
        let tree = render_sdk(&api, "sdk", Some("Probe"), SdkClientStyle::Flat).unwrap();
        let source = tree
            .get("sdk/ClientOperations000.cs")
            .or_else(|| tree.get("sdk/Operations0000.cs"));
        // File layout is independently bounded; inspect the actual renderer body.
        let source = source
            .map(str::to_owned)
            .unwrap_or_else(|| render_operation_chunk(&api, &api.operations, "Probe"));
        assert!(source.contains("ListLinksPagesAsync"));
        assert!(source.contains("ListLinksKajiURLAsync"));
        assert!(source.contains("uri.IdnHost"));
        assert!(source.contains("uri.UserInfo.Length!=0"));
        assert!(source.contains("uri.Port!=_baseUri.Port"));
        assert!(
            source.find("var paginationURL =").unwrap()
                < source.find("var request = CreateRequest").unwrap()
        );
        assert!(source.contains("seenURLs.Add"));
    }
    #[test]
    #[ignore = "Requires .NET8; real generated URL operations with in-memory HttpMessageHandler"]
    fn native_url_pages_reject_unsafe_origins_preserve_auth_and_cancel_lazily() {
        let root = tempfile::tempdir().unwrap();
        render_sdk(&api(), "sdk", Some("Probe"), SdkClientStyle::Flat)
            .unwrap()
            .write_to(root.path())
            .unwrap();
        let sdk = root.path().join("sdk");
        let project = std::fs::read_dir(&sdk)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.extension().is_some_and(|ext| ext == "csproj"))
            .unwrap();
        let contents = std::fs::read_to_string(&project).unwrap().replace(
            "<TargetFramework>",
            "<OutputType>Exe</OutputType><TargetFramework>",
        );
        std::fs::write(&project, contents).unwrap();
        std::fs::write(
            sdk.join("Program.cs"),
            include_str!("url_pagination_probe.cs"),
        )
        .unwrap();
        let output = std::process::Command::new("dotnet")
            .args(["run", "--project"])
            .arg(project)
            .current_dir(sdk)
            .output()
            .expect(".NET8 toolchain unavailable");
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
