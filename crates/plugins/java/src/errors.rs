use super::*;

pub(super) fn declared_error_responses(operation: &Operation) -> Vec<&OperationResponse> {
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

pub(super) fn declared_error_schema(response: &OperationResponse) -> Option<&SchemaValue> {
    response
        .media_types
        .iter()
        .find(|media| is_json_media(&media.content_type))
        .and_then(|media| media.schema.as_ref())
}

pub(super) fn java_error_name(operation: &Operation, response: &OperationResponse) -> String {
    format!(
        "{}Status{}Exception",
        type_name(&operation.id),
        response.status
    )
}

pub(super) fn java_error_mapper_name(operation: &Operation) -> String {
    format!("map{}Error", type_name(&operation.id))
}

/// Error types are nested under Client so the package retains one public
/// transport exception file while callers can catch operation/status-specific
/// classes. A malformed declared error body remains available as raw text.
pub(super) fn render_declared_error_types(output: &mut String, api: &Api) {
    for operation in &api.operations {
        let responses = declared_error_responses(operation);
        if responses.is_empty() {
            continue;
        }
        let mapper = java_error_mapper_name(operation);
        let _ = writeln!(
            output,
            "    private RuntimeException {mapper}(ApiException error) {{\n        return switch (error.statusCode()) {{"
        );
        for response in &responses {
            let status = &response.status;
            let error = java_error_name(operation, response);
            if let Some(schema) = declared_error_schema(response) {
                let class = response_class(schema);
                let _ = writeln!(
                    output,
                    "            case {status} -> new {error}(error, decodeDeclaredError(error.responseBody(), {class}.class));"
                );
            } else {
                let _ = writeln!(output, "            case {status} -> new {error}(error);");
            }
        }
        output.push_str("            default -> error;\n        };\n    }\n\n");
        for response in responses {
            let error = java_error_name(operation, response);
            if let Some(schema) = declared_error_schema(response) {
                let body = operation_response_type(schema);
                let _ = writeln!(
                    output,
                    "    public static final class {error} extends ApiException {{\n  private final {body} body;\n  \n        private {error}(ApiException source, {body} body) {{\n    super(source.statusCode(), source.responseBody(), source.retryAfter(), source.retryAfterMillis());\n    this.body = body;\n\n  }}\n  \n        public {body} body() {{\n    return body;\n\n  }}\n  \n\n}}\n\n"
                );
            } else {
                let _ = writeln!(
                    output,
                    "    public static final class {error} extends ApiException {{\n  private {error}(ApiException source) {{\n    super(source.statusCode(), source.responseBody(), source.retryAfter(), source.retryAfterMillis());\n\n  }}\n  \n\n}}\n\n"
                );
            }
        }
    }
    output.push_str("    private <T> T decodeDeclaredError(String response, Class<T> type) {\n        if (response == null || response.isBlank()) return null;\n        try { return mapper.readValue(response, type); } catch (JsonProcessingException ignored) { return null; }\n    }\n\n");
}
