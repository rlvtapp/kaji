//! HTTP operations rendering.
use super::*;

pub(crate) fn direct_method_name(operation: &Operation, options: &RenderOptions) -> String {
    rust_field_name(
        &options
            .operation_prefix
            .as_ref()
            .map(|prefix| format!("{prefix}_{}", operation.id))
            .unwrap_or_else(|| operation.id.clone()),
    )
}

pub(crate) fn render_operation(operation: &Operation, config: &RenderOptions) -> String {
    let method_name = direct_method_name(operation, config);
    let response = operation_response_type(operation);
    let error = operation_error_name(operation);
    let pagination = rust_pagination(operation);
    let parameters = if operation_has_parameters(operation) {
        format!(
            ", input: crate::client::operations::{}",
            operation_request_name(operation)
        )
    } else {
        String::new()
    };
    let request_body = request_body_type(operation)
        .map(|request| format!(", body: &{request}"))
        .unwrap_or_default();
    let path_binding = if operation
        .parameters
        .iter()
        .any(|parameter| matches!(parameter.location.as_str(), "path" | "querystring"))
    {
        "mut "
    } else {
        ""
    };
    let query_binding = if operation
        .parameters
        .iter()
        .any(|parameter| parameter.location == "query")
    {
        "mut "
    } else {
        ""
    };
    let mut output = format!(
        "    /// Invokes {} {}.\n    pub async fn {method_name}(&self{parameters}{request_body}) -> Result<{response}, {error}> {{\n        let {path_binding}path = {:?}.to_owned();\n        let {query_binding}query: Vec<(String, String)> = Vec::new();\n",
        operation.method.as_str(),
        operation.path,
        operation.path,
    );
    if let Some(policy) =
        poolster_core::idempotency::resolved(operation).filter(|policy| policy.auto_generate)
    {
        let field = operation
            .parameters
            .iter()
            .find(|parameter| {
                parameter.location == "header"
                    && parameter.name.eq_ignore_ascii_case(&policy.parameter_name)
            })
            .map(parameter_name)
            .unwrap_or_else(|| rust_field_name(&policy.parameter_name));
        let _ = writeln!(
            output,
            "        let mut input = input;\n        if input.{field}.is_none() {{ input.{field} = Some(uuid::Uuid::new_v4().to_string()); }}"
        );
    }
    for parameter in &operation.parameters {
        render_parameter_use(&mut output, parameter);
    }
    for parameter in operation
        .parameters
        .iter()
        .filter(|p| p.location == "querystring")
    {
        let field = parameter_name(parameter);
        let value = if parameter.required {
            format!("Some(input.{field}.as_str())")
        } else {
            format!("input.{field}.as_deref()")
        };
        let _ = writeln!(
            output,
            "        if let Some(raw_query) = {value} {{ if !poolster_valid_raw_query(raw_query) {{ return Err({error}::UnsupportedRequestMedia(\"invalid serialized whole query\")); }} if !raw_query.is_empty() {{ path.push('?'); path.push_str(raw_query); }} }}"
        );
    }
    let retry_allowed = rust_retry_allowed(operation);
    let native_method = match &operation.method {
        poolster_core::HttpMethod::Query => {
            "Method::from_bytes(b\"QUERY\").expect(\"static HTTP method\")".to_owned()
        }
        poolster_core::HttpMethod::Custom(_) => format!(
            "Method::from_bytes({:?}.as_bytes()).expect(\"validated HTTP method\")",
            operation.method.as_str()
        ),
        _ => format!("Method::{}", operation.method.as_str()),
    };
    if matches!(request_media_kind(operation), RequestMediaKind::Multipart) {
        let plan = poolster_core::openapi32::request_content(operation)
            .ok()
            .and_then(|items| {
                items
                    .into_iter()
                    .find(|item| item.content_type.starts_with("multipart/"))
            })
            .and_then(|item| serde_json::to_string(&item).ok());
        if let Some(plan) = plan {
            let _ = writeln!(
                output,
                "        let plan:serde_json::Value=serde_json::from_str({plan:?}).expect(\"generated multipart plan\");\n        let (multipart_content_type,multipart_bytes)=body.encoded_with_plan(&plan).map_err(|_|{error}::UnsupportedRequestMedia(\"invalid multipart input or encoding plan\"))?;"
            );
        } else {
            output.push_str(
                "        let (multipart_content_type, multipart_bytes) = body.encoded();\n",
            );
        }
    }
    output.push_str(&format!(
        "        let request_url = format!(\"{{}}{{}}\", self.base_url, path);\n        let request_info = RequestInfo {{ method: {}, url: request_url.clone() }};\n        self.poolster_before_request(&request_info);\n        let retry_allowed = {retry_allowed};\n        let max_attempts = self.retry.max_attempts.max(1);\n        let mut attempt = 0_usize;\n        let mut oauth_replayed = false;\n        #[allow(unused_mut)]\n        let mut oauth_token: Option<String> = None;\n        loop {{\n            attempt += 1;\n            let mut request = self.http.request({}, request_url.clone()).headers(self.call_options.headers.clone());\n            if let Some(timeout) = self.call_options.timeout {{ request = request.timeout(timeout); }}\n            if !query.is_empty() {{ request = request.query(&query); }}\n",
        native_method,
        native_method,
    ));
    for parameter in &operation.parameters {
        if matches!(parameter.location.as_str(), "header" | "cookie") {
            render_header_use(&mut output, parameter);
        }
    }
    if !operation.security.is_empty() {
        output.push_str("        if !self.call_options.headers.contains_key(reqwest::header::AUTHORIZATION) { if let Some(token) = &self.bearer_token { request = request.bearer_auth(token); } }\n");
    }
    if operation.request_body.is_some() {
        match request_media_kind(operation) {
            RequestMediaKind::Json | RequestMediaKind::Unknown => {
                let media = operation.request_body.as_ref().and_then(|body|body.media_types.first()).map(|media|media.content_type.as_str()).unwrap_or("");
                if matches!(media,"application/x-ndjson"|"application/ndjson"|"application/jsonl"|"application/json-seq") {
                    let _ = writeln!(output,"        request = request.header(reqwest::header::CONTENT_TYPE,{media:?}).body(poolster_encode_sequence(body,{media:?}).map_err(|_|{error}::UnsupportedRequestMedia(\"invalid sequential JSON request\"))?);");
                } else { output.push_str("        request = request.json(body);\n"); }
            }
            RequestMediaKind::Multipart => output.push_str("        request = request.header(reqwest::header::CONTENT_TYPE, multipart_content_type.clone()).body(multipart_bytes.clone());\n"),
            RequestMediaKind::Unsupported(content_type) => {
                let _ = writeln!(
                    output,
                    "        return Err({error}::UnsupportedRequestMedia({content_type:?}));"
                );
            }
        }
    }
    output.push_str(&format!(
        "            #[allow(unused_mut)]\n            let mut request = request.build().map_err({error}::Transport)?;\n"
    ));
    if !operation.security.is_empty() {
        output.push_str(&format!("            if !request.headers().contains_key(reqwest::header::AUTHORIZATION) {{ if let Some(provider) = &self.token_provider {{ let token = provider.token().await.map_err({error}::TokenProvider)?; let header = reqwest::header::HeaderValue::from_str(&format!(\"Bearer {{token}}\")).map_err(|_| {error}::TokenProvider(TokenProviderError))?; request.headers_mut().insert(reqwest::header::AUTHORIZATION, header); oauth_token = Some(token); }} }}\n"));
    }
    output.push_str(&format!(
        "            let response = match self.transport.execute(request).await {{\n                Ok(response) => response,\n                Err(source) => {{\n                    if retry_allowed && attempt < max_attempts {{\n                        tokio::time::sleep(self.poolster_retry_delay(attempt, None)).await;\n                        continue;\n                    }}\n                    self.poolster_on_error(&request_info, &source.to_string());\n                    return Err({error}::Transport(source));\n                }}\n            }};\n            let status = response.status();\n            if status.as_u16() == 401 && retry_allowed && !oauth_replayed {{ if let (Some(provider), Some(token)) = (&self.token_provider, oauth_token.take()) {{ provider.invalidate(&token); oauth_replayed = true; continue; }} }}\n            let retry_after = poolster_retry_after(response.headers());\n            if retry_allowed && attempt < max_attempts && matches!(status.as_u16(), 408 | 429 | 500 | 502 | 503 | 504) {{\n                tokio::time::sleep(self.poolster_retry_delay(attempt, retry_after)).await;\n                continue;\n            }}\n            self.poolster_after_response(&request_info, &response);\n            if !status.is_success() {{\n                let headers = response.headers().clone();\n                let body = response.bytes().await.map_err({error}::Transport)?.to_vec();\n                self.poolster_on_error(&request_info, &format!(\"HTTP {{}}\", status));\n                return Err({});\n            }}\n",
        render_error_response(operation, &error),
    ));
    match response_kind(operation) {
        ResponseKind::Empty => output.push_str("            return Ok(());\n"),
        ResponseKind::Binary => output.push_str(&format!(
            "            return response.bytes().await.map(|body| body.to_vec()).map_err({error}::Transport);\n"
        )),
        ResponseKind::ServerSentEvents => output.push_str("            return Ok(response);\n"),
        ResponseKind::Text => output.push_str(&format!(
            "            return response.bytes().await.map(|body| String::from_utf8_lossy(&body).into_owned()).map_err({error}::Transport);\n"
        )),
        ResponseKind::Json => output.push_str(&format!(
            "            let headers = response.headers().clone();\n            let body = response.bytes().await.map_err({error}::Transport)?.to_vec();\n            return match poolster_decode_json::<{response}>(&body, headers.get(reqwest::header::CONTENT_TYPE).and_then(|value|value.to_str().ok()).unwrap_or(\"\")) {{\n                Ok(value) => Ok(value),\n                Err(source) => {{\n                    self.poolster_on_error(&request_info, &source.to_string());\n                    Err({error}::Decode {{ source, response: ApiResponse {{ status, headers, body }} }})\n                }}\n            }};\n"
        )),
    }
    output.push_str("        }\n    }\n\n");
    if matches!(pagination, Some(RustPagination::Url { .. })) {
        let original = output.clone();
        let end = original.find(" -> Result<").unwrap();
        let mut helper = original.clone();
        helper.insert_str(end - 1, ", poolster_url: Option<&str>");
        helper = helper.replacen(
            &format!("pub async fn {method_name}("),
            &format!("async fn {method_name}_poolster_url("),
            1,
        );
        helper=helper.replace("let request_url = format!(\"{}{}\", self.base_url, path);",&format!("let request_url = if let Some(url)=poolster_url {{ poolster_same_origin_url(&self.base_url,url).map_err({error}::Pagination)? }} else {{ format!(\"{{}}{{}}\",self.base_url,path) }};"));
        helper = helper.replace(
            "if !query.is_empty() {",
            "if poolster_url.is_none() && !query.is_empty() {",
        );
        let args = if operation_has_parameters(operation) {
            "input, "
        } else {
            ""
        };
        output = format!(
            "{} {{\n        self.{method_name}_poolster_url({args}None).await\n    }}\n\n{helper}",
            &original[..original.find(" {\n").unwrap()]
        );
    }
    if let Some(pagination) = pagination {
        output.push_str(&render_rust_pagination_iterator(
            &method_name,
            operation,
            &pagination,
        ));
    }
    output
}
