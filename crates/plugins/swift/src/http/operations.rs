//! Operations emission for the Swift HTTP SDK.
use crate::*;

pub(crate) fn operation_is_multipart_response(operation: &Operation) -> bool {
    operation
        .responses
        .iter()
        .find(|response| response.status.starts_with('2'))
        .and_then(|response| response.media_types.first())
        .is_some_and(|media| {
            media
                .content_type
                .to_ascii_lowercase()
                .starts_with("multipart/")
        })
}
pub(crate) fn operation_is_sse(operation: &Operation) -> bool {
    operation
        .responses
        .iter()
        .find(|response| response.status.starts_with('2'))
        .is_some_and(|response| {
            response.media_types.iter().any(|media| {
                media
                    .content_type
                    .split(';')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .eq_ignore_ascii_case("text/event-stream")
            })
        })
}
pub(crate) fn render_operation(operation: &Operation, indent: &str) -> String {
    let name = function_name(&operation.id);
    let parameters = operation_parameters(operation);
    let sse = operation_is_sse(operation);
    let response = if sse {
        "PoolsterByteStream".to_owned()
    } else if operation_is_multipart_response(operation) {
        "Data".to_owned()
    } else {
        operation
            .success_schema()
            .map(|schema| swift_type(schema, false))
            .unwrap_or_else(|| "Void".to_owned())
    };
    let mut output = String::new();
    let _ = write!(output, "{indent}func {name}(");
    for (index, parameter) in parameters.iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }
        output.push_str(&parameter.signature);
    }
    let _ = writeln!(output, ") async throws -> {response} {{");
    let mut path = operation.path.clone();
    for (index, parameter) in operation
        .parameters
        .iter()
        .filter(|parameter| parameter.location == "path")
        .enumerate()
    {
        let value = parameter_name(parameter);
        let serialized = if parameter_json_content(parameter) {
            format!("try jsonParameter({value})")
        } else {
            format!("String(describing: {value})")
        };
        let path_value = format!("_poolsterPath{index}");
        let _ = writeln!(
            output,
            "{indent}    let {path_value} = ({serialized}).poolsterPathComponent"
        );
        path = path.replace(
            &format!("{{{}}}", parameter.name),
            &format!("\\({path_value})"),
        );
    }
    let url_pagination = poolster_core::poolster_extension(&operation.annotations, "pagination")
        .or_else(|| operation.annotations.get("x-speakeasy-pagination"))
        .and_then(|value| value.get("type"))
        .and_then(serde_json::Value::as_str)
        == Some("url");
    let request_binding = if operation
        .parameters
        .iter()
        .any(|p| p.location == "querystring")
        || sse
        || url_pagination
        || operation.request_body.is_some()
        || operation
            .parameters
            .iter()
            .any(|parameter| parameter.location == "header")
    {
        "var"
    } else {
        "let"
    };
    let mut query_binding = "__poolsterQuery".to_owned();
    while operation
        .parameters
        .iter()
        .any(|parameter| parameter_name(parameter) == query_binding)
    {
        query_binding.push_str("Items");
    }
    let query_mutability = if operation
        .parameters
        .iter()
        .any(|parameter| parameter.location == "query")
    {
        "var"
    } else {
        "let"
    };
    let _ = writeln!(
        output,
        "{indent}    {query_mutability} {query_binding}: [URLQueryItem] = []"
    );
    let query_parameters = operation
        .parameters
        .iter()
        .filter(|p| p.location == "query")
        .collect::<Vec<_>>();
    let mut query_helpers = String::new();
    // Hundreds of serialization branches in one async method make Swift's
    // debug LLVM code generation disproportionately expensive. Keep the public
    // arguments intact and build their query items in bounded sync functions.
    if query_parameters.len() > 100 {
        for (index, chunk) in query_parameters.chunks(50).enumerate() {
            let helper = format!("__poolsterQuery_{name}_{index}");
            let local = Operation {
                parameters: chunk.iter().map(|p| (*p).clone()).collect(),
                ..Default::default()
            };
            let rendered = operation_parameters(&local);
            let signature = rendered
                .iter()
                .map(|p| p.signature.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let arguments = rendered
                .iter()
                .map(|p| {
                    let name = p.signature.split(':').next().unwrap();
                    format!("{name}: {name}")
                })
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(
                output,
                "{indent}    {query_binding}.append(contentsOf: try self.{helper}({arguments}))"
            );
            let _ = writeln!(
                query_helpers,
                "{indent}private func {helper}({signature}) throws -> [URLQueryItem] {{\n{indent}    var {query_binding}: [URLQueryItem] = []"
            );
            query_helpers.push_str(&render_query_parameters(&local, &query_binding, indent));
            let _ = writeln!(
                query_helpers,
                "{indent}    return {query_binding}\n{indent}}}\n"
            );
        }
    } else {
        output.push_str(&render_query_parameters(operation, &query_binding, indent));
    }
    let _ = writeln!(
        output,
        "{indent}    {request_binding} request = try self.makeRequest(method: {:?}, path: \"{}\", query: {query_binding})",
        operation.method.as_str(),
        swift_path_literal(&path)
    );
    for parameter in operation
        .parameters
        .iter()
        .filter(|p| p.location == "querystring")
    {
        let value = parameter_name(parameter);
        let expression = if parameter.required {
            format!("Optional({value})")
        } else {
            value
        };
        let _ = writeln!(
            output,
            "{indent}    if let rawQuery = {expression} {{ try applyWholeQuery(&request, raw:rawQuery) }}"
        );
    }
    for parameter in operation
        .parameters
        .iter()
        .filter(|parameter| parameter.location == "header")
    {
        let value = parameter_name(parameter);
        let serialized = if parameter_json_content(parameter) {
            format!("try jsonParameter({value})")
        } else {
            format!("String(describing: {value})")
        };
        if parameter.required {
            let _ = writeln!(
                output,
                "{indent}    request.setValue({serialized}, forHTTPHeaderField: {:?})",
                parameter.name
            );
        } else {
            let _ = writeln!(
                output,
                "{indent}    if let {value} {{ request.setValue({serialized}, forHTTPHeaderField: {:?}) }}",
                parameter.name
            );
        }
    }
    for parameter in operation
        .parameters
        .iter()
        .filter(|p| p.location == "cookie")
    {
        let value = parameter_name(parameter);
        let expression = if parameter.required {
            format!("Optional({value})")
        } else {
            value
        };
        let serialized = if parameter_json_content(parameter) {
            "try jsonParameter(value)"
        } else {
            "String(describing:value)"
        };
        let _ = writeln!(
            output,
            "{indent}    if let value={expression} {{let cookie={:?}+\"=\"+({serialized}).poolsterPathComponent;request.setValue(request.value(forHTTPHeaderField: \"Cookie\").map{{$0+\"; \"+cookie}} ?? cookie,forHTTPHeaderField: \"Cookie\")}}",
            parameter.name
        );
    }
    if let Some(policy) =
        poolster_core::idempotency::resolved(operation).filter(|policy| policy.auto_generate)
    {
        let value = operation
            .parameters
            .iter()
            .find(|parameter| {
                parameter.location == "header"
                    && (parameter.name == policy.parameter_name
                        || parameter.name.eq_ignore_ascii_case(&policy.header))
            })
            .map(parameter_name)
            .unwrap_or_else(|| identifier(&policy.parameter_name));
        let _ = writeln!(
            output,
            "{indent}    if {value} == nil {{ request.setValue(UUID().uuidString.lowercased(), forHTTPHeaderField: {:?}) }}",
            policy.header
        );
    }
    if let Some(body) = &operation.request_body {
        if multipart::body_type(operation).is_some() {
            if body.required {
                let _ = writeln!(
                    output,
                    "{indent}    let encoded = try body.poolsterEncoded()\n{indent}    request.httpBody = encoded.body\n{indent}    request.setValue(encoded.contentType, forHTTPHeaderField: \"Content-Type\")"
                );
            } else {
                let _ = writeln!(
                    output,
                    "{indent}    if let body {{\n{indent}        let encoded = try body.poolsterEncoded()\n{indent}        request.httpBody = encoded.body\n{indent}        request.setValue(encoded.contentType, forHTTPHeaderField: \"Content-Type\")\n{indent}    }}"
                );
            }
        } else {
            let content_type = body
                .media_types
                .first()
                .map(|media| media.content_type.as_str())
                .unwrap_or("application/json");
            let _ = writeln!(
                output,
                "{indent}    request.setValue({content_type:?}, forHTTPHeaderField: \"Content-Type\")"
            );
            let encoding = if matches!(
                content_type,
                "application/x-ndjson"
                    | "application/ndjson"
                    | "application/jsonl"
                    | "application/json-seq"
            ) {
                format!("encodeSequentialJSON(body,media:{content_type:?})")
            } else {
                "self.encode(body)".into()
            };
            if body.required {
                let _ = writeln!(output, "{indent}    request.httpBody = try {encoding}");
            } else {
                let _ = writeln!(
                    output,
                    "{indent}    if let body {{ request.httpBody = try {encoding} }}"
                );
            }
        }
    }
    let retry_header = poolster_core::idempotency::resolved(operation)
        .map(|p| format!("{:?}", p.header))
        .unwrap_or_else(|| "nil".into());
    if sse {
        let _ = writeln!(
            output,
            "{indent}    request.setValue(\"text/event-stream\", forHTTPHeaderField: \"Accept\")\n{indent}    return try await streamEvents(request)"
        );
    } else if operation_is_multipart_response(operation) {
        let _ = writeln!(
            output,
            "{indent}    return try await sendBytes(request, idempotencyHeader: {retry_header})"
        );
    } else if response == "Void" {
        let _ = writeln!(
            output,
            "{indent}    try await self.sendVoid(request, idempotencyHeader: {retry_header})"
        );
    } else {
        let _ = writeln!(
            output,
            "{indent}    return try await self.send(request, as: {response}.self, idempotencyHeader: {retry_header})"
        );
    }
    let _ = writeln!(output, "{indent}}}\n");
    if sse {
        output = output.replace(
            ") async throws -> PoolsterByteStream {",
            ") -> PoolsterEventSequence {\n        PoolsterEventSequence { [self] in",
        );
        let end = output.rfind(&format!("{indent}}}")).unwrap();
        output.insert_str(end, &format!("{indent}    }}\n"));
    }
    if url_pagination {
        let original = output.clone();
        let signature_end = original.find(") async throws").unwrap();
        let mut helper = original.clone();
        helper.insert_str(
            signature_end,
            &format!(
                "{}_poolsterURL: URL?",
                if parameters.is_empty() { "" } else { ", " }
            ),
        );
        helper = helper.replacen(
            &format!("func {name}("),
            &format!("internal func {name}PoolsterURL("),
            1,
        );
        let brace = helper.find(" {\n").unwrap() + 3;
        helper.insert_str(
            brace,
            &format!(
                "{indent}    let _poolsterValidatedURL = try self.poolsterContinuationURL(_poolsterURL)\n"
            ),
        );
        let send = helper
            .rfind(&format!("{indent}    return try await self.send"))
            .or_else(|| helper.rfind(&format!("{indent}    try await self.send")))
            .unwrap();
        helper.insert_str(
            send,
            &format!(
                "{indent}    if let _poolsterValidatedURL {{ request.url = _poolsterValidatedURL }}\n"
            ),
        );
        let args = parameters
            .iter()
            .map(|parameter| {
                let label = parameter.signature.split(':').next().unwrap();
                format!("{label}: {label}")
            })
            .collect::<Vec<_>>()
            .join(", ");
        let args = if args.is_empty() {
            String::new()
        } else {
            format!("{args}, ")
        };
        output = format!(
            "{} {{\n{indent}    return try await {name}PoolsterURL({args}_poolsterURL: nil)\n{indent}}}\n\n{helper}",
            &original[..signature_end + format!(") async throws -> {response}").len()]
        );
    }
    output.push_str(&query_helpers);
    output
}
