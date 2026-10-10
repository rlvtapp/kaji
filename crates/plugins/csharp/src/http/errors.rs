//! Errors emission for the csharp HTTP SDK.
use crate::*;

pub(crate) fn render_api_exception(api: &Api, namespace: &str) -> String {
    let mut output = format!(
        "{NOTICE}\nusing System.Text.Json;\n\nnamespace {namespace};\n\n\n/// <summary>An unsuccessful response returned by the API.</summary>\npublic class ApiException : Exception\n{{\n  public ApiException(int statusCode, string responseBody)\n        : base($\"API request failed with HTTP {{statusCode}}.\")\n    {{\n    StatusCode = statusCode;\n    \n        ResponseBody = responseBody;\n    \n\n  }}\n  \n\n    public int StatusCode {{\n    get;\n\n  }}\n  \n\n    public string ResponseBody {{\n    get;\n\n  }}\n  \n}}\n\n"
    );
    for operation in &api.operations {
        for response in declared_error_responses(operation) {
            let error = dotnet_error_name(operation, response);
            let body = declared_error_type(response);
            let status = &response.status;
            if let Some(body) = body {
                let _ = writeln!(
                    output,
                    "\n/// <summary>Declared {status} response from {}.</summary>\npublic sealed class {error} : ApiException\n{{\n  public {error}(string responseBody, {body}? body) : base({status}, responseBody) => Body = body;\n  \n    public {body}? Body {{\n    get;\n\n  }}\n  \n}}",
                    operation.id
                );
            } else {
                let _ = writeln!(
                    output,
                    "\n/// <summary>Declared {status} response from {}.</summary>\npublic sealed class {error} : ApiException\n{{\n  public {error}(string responseBody) : base({status}, responseBody) {{\n\n  }}\n  \n}}",
                    operation.id
                );
            }
        }
    }
    output
}

/// Emits error selection at the operation boundary, after the common transport
/// has retained the status and raw payload. Only concrete non-2xx statuses are
/// mapped: an OpenAPI `default` response has no stable status-specific class.
pub(crate) fn render_declared_error_mapper(output: &mut String, operation: &Operation) {
    let responses = declared_error_responses(operation);
    if responses.is_empty() {
        return;
    }
    let mapper = dotnet_error_mapper_name(operation);
    let _ = writeln!(
        output,
        "\n    private static ApiException {mapper}(ApiException error)\n    {{\n        switch (error.StatusCode)\n        {{"
    );
    for response in responses {
        let status = &response.status;
        let error_name = dotnet_error_name(operation, response);
        if let Some(body) = declared_error_type(response) {
            let _ = writeln!(
                output,
                "            case {status}: return new {error_name}(error.ResponseBody, TryDeserializeError<{body}>(error.ResponseBody));"
            );
        } else {
            let _ = writeln!(
                output,
                "            case {status}: return new {error_name}(error.ResponseBody);"
            );
        }
    }
    output.push_str("            default: return error;\n        }\n    }\n");
}

pub(crate) fn render_error_deserialization_helper(output: &mut String) {
    output.push_str("\n    private static T? TryDeserializeError<T>(string contents)\n    {\n        if (string.IsNullOrWhiteSpace(contents)) return default;\n        try { return JsonSerializer.Deserialize<T>(contents, JsonOptions); }\n        catch (JsonException) { return default; }\n    }\n");
}

pub(crate) fn declared_error_responses(operation: &Operation) -> Vec<&OperationResponse> {
    operation
        .responses
        .iter()
        .filter(|response| {
            response
                .status
                .parse::<u16>()
                .is_ok_and(|status| !(200..300).contains(&status))
        })
        .collect()
}

pub(crate) fn declared_error_type(response: &OperationResponse) -> Option<String> {
    response
        .media_types
        .iter()
        .find(|media| is_json_media(&media.content_type))
        .and_then(|media| media.schema.as_ref())
        .map(|schema| csharp_type(schema, false))
}

pub(crate) fn dotnet_error_name(operation: &Operation, response: &OperationResponse) -> String {
    format!(
        "{}Status{}Exception",
        pascal_case(&operation.id),
        response.status
    )
}

pub(crate) fn dotnet_error_mapper_name(operation: &Operation) -> String {
    format!("Map{}Error", pascal_case(&operation.id))
}
