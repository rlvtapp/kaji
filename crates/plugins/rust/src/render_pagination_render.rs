//! HTTP pagination render rendering.
use super::*;

pub(crate) fn render_pagination_runtime() -> &'static str {
    r#"
#[allow(dead_code)]
fn poolster_same_origin_url(base:&str,next:&str)->Result<String,serde_json::Error> {
    let base=reqwest::Url::parse(base).map_err(|_|poolster_pagination_error("invalid API origin"))?;
    let next=reqwest::Url::parse(next).map_err(|_|poolster_pagination_error("invalid absolute continuation URL"))?;
    if !matches!(next.scheme(),"http"|"https") || next.scheme()!=base.scheme() || next.host_str()!=base.host_str() || next.port_or_known_default()!=base.port_or_known_default() || !next.username().is_empty() || next.password().is_some() || next.fragment().is_some() {return Err(poolster_pagination_error("unsafe continuation URL"))}
    Ok(next.to_string())
}

/// Read declared JSONPath fields/indices or RFC 6901 pointers without evaluating code.
fn poolster_json_path<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    if path.starts_with('/') {
        let mut chars = path.chars();
        while let Some(c) = chars.next() {
            if c == '~' && !matches!(chars.next(), Some('0' | '1')) { return None; }
        }
        return value.pointer(path);
    }
    let mut rest = path.strip_prefix('$')?;
    let mut current = value;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('.') {
            let end = after.find(['.', '[']).unwrap_or(after.len());
            if end == 0 { return None; }
            current = current.as_object()?.get(&after[..end])?;
            rest = &after[end..];
        } else if let Some(after) = rest.strip_prefix('[') {
            let end = after.find(']')?;
            let token = &after[..end];
            let digits = token.strip_prefix('-').unwrap_or(token);
            if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) { return None; }
            let index = token.parse::<isize>().ok()?;
            let values = current.as_array()?;
            let index = if index < 0 { values.len().checked_add_signed(index)? } else { index as usize };
            current = values.get(index)?;
            rest = &after[end + 1..];
        } else { return None; }
    }
    Some(current)
}

fn poolster_pagination_error(message: &'static str) -> serde_json::Error {
    serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::InvalidData, message))
}

"#
}

pub(crate) fn render_rust_pagination_iterator(
    method_name: &str,
    operation: &Operation,
    pagination: &RustPagination,
) -> String {
    if let RustPagination::Url { next_url_path } = pagination {
        return render_rust_url_iterator(method_name, operation, next_url_path);
    }
    let request = operation_request_name(operation);
    let response = operation_response_type(operation);
    let error = operation_error_name(operation);
    let pages_method = format!("{method_name}_pages");
    let mut initial = String::new();
    let mut before = String::new();
    let (state, next) = match pagination {
        RustPagination::Url { .. } => unreachable!(),
        RustPagination::Page {
            field,
            limit,
            results_path,
        } => {
            let name = &field.name;
            if field.optional {
                initial = format!(
                    "        let mut input = input;\n        if input.{name}.is_none() {{ input.{name} = Some(1); }}\n"
                );
            }
            let current = if field.optional {
                format!("input.{name}.unwrap_or(1)")
            } else {
                format!("input.{name}")
            };
            before = format!(
                "            if {current} < 0 {{ return Err({error}::Pagination(poolster_pagination_error(\"page must be nonnegative\"))); }}\n"
            );
            let limit = limit.as_ref().map_or_else(
                || "None::<i64>".into(),
                |field| {
                    if field.optional {
                        format!("input.{}", field.name)
                    } else {
                        format!("Some(input.{})", field.name)
                    }
                },
            );
            before.push_str(&format!("            if ({limit}).is_some_and(|limit| limit <= 0) {{ return Err({error}::Pagination(poolster_pagination_error(\"limit must be positive\"))); }}\n"));
            let assignment = if field.optional {
                format!("input.{name} = Some(next_page);")
            } else {
                format!("input.{name} = next_page;")
            };
            (
                "true".into(),
                format!(
                    "            let next = serde_json::to_value(&response).map_err({error}::Pagination)?;\n            let items = poolster_json_path(&next, {results_path:?}).and_then(serde_json::Value::as_array).ok_or_else(|| {error}::Pagination(poolster_pagination_error(\"pagination results must be an array\")))?;\n            let limit = {limit};\n            if items.is_empty() || limit.is_some_and(|limit| limit > 0 && (items.len() as i64) < limit) {{ return Ok(Some((response, (client, input, false, page_count + 1)))); }}\n            let Some(next_page) = ({current}).checked_add(1) else {{ return Ok(Some((response, (client, input, false, page_count + 1)))); }};\n            {assignment}\n            Ok(Some((response, (client, input, true, page_count + 1))))"
                ),
            )
        }
        RustPagination::Cursor {
            field,
            next_cursor_path,
        } => {
            let assignment = if field.optional {
                format!("input.{} = Some(cursor.to_owned());", field.name)
            } else {
                format!("input.{} = cursor.to_owned();", field.name)
            };
            (
                "true".to_owned(),
                format!(
                    "            let next = serde_json::to_value(&response).map_err({error}::Pagination)?;\n            let Some(cursor) = poolster_json_path(&next, {next_cursor_path:?}).and_then(serde_json::Value::as_str).filter(|cursor| !cursor.is_empty()) else {{\n                return Ok(Some((response, (client, input, false, page_count + 1))));\n            }};\n            {assignment}\n            Ok(Some((response, (client, input, true, page_count + 1))))"
                ),
            )
        }
        RustPagination::OffsetLimit {
            step,
            limit_field,
            results_path,
            num_pages_path,
        } => {
            let field = match step {
                RustOffsetStep::Page { field } | RustOffsetStep::Offset { field } => field,
            };
            let default = if matches!(step, RustOffsetStep::Page { .. }) {
                1
            } else {
                0
            };
            initial = format!(
                "        let mut input = input;\n        if input.{field}.is_none() {{ input.{field} = Some({default}); }}\n"
            );
            let continue_condition = match step {
                RustOffsetStep::Page { .. } => format!(
                    "            let Some(current) = input.{field} else {{ return Ok(Some((response, (client, input, false, page_count + 1)))); }};\n            let next = serde_json::to_value(&response).map_err({error}::Pagination)?;\n            let Some(num_pages) = poolster_json_path(&next, {path:?}).and_then(serde_json::Value::as_i64) else {{ return Ok(Some((response, (client, input, false, page_count + 1)))); }};\n            let Some(next_page) = current.checked_add(1) else {{ return Ok(Some((response, (client, input, false, page_count + 1)))); }};\n            if next_page > num_pages {{ return Ok(Some((response, (client, input, false, page_count + 1)))); }};\n            input.{field} = Some(next_page);\n            Ok(Some((response, (client, input, true, page_count + 1))))",
                    path = num_pages_path
                        .as_deref()
                        .expect("validated page pagination"),
                ),
                RustOffsetStep::Offset { .. } => {
                    let results_path = results_path
                        .as_deref()
                        .expect("validated offset pagination");
                    let limit = limit_field
                        .as_ref()
                        .map_or_else(|| "None".to_owned(), |field| format!("input.{field}"));
                    format!(
                        "            let Some(current) = input.{field} else {{ return Ok(Some((response, (client, input, false, page_count + 1)))); }};\n            let next = serde_json::to_value(&response).map_err({error}::Pagination)?;\n            let Some(items) = poolster_json_path(&next, {results_path:?}).and_then(serde_json::Value::as_array) else {{ return Ok(Some((response, (client, input, false, page_count + 1)))); }};\n            let item_count = items.len() as i64;\n            let limit = {limit};\n            if item_count == 0 || limit.is_some_and(|limit| item_count < limit) {{ return Ok(Some((response, (client, input, false, page_count + 1)))); }};\n            let Some(next_offset) = current.checked_add(item_count) else {{ return Ok(Some((response, (client, input, false, page_count + 1)))); }};\n            input.{field} = Some(next_offset);\n            Ok(Some((response, (client, input, true, page_count + 1))))"
                    )
                }
            };
            ("true".to_owned(), continue_condition)
        }
    };
    format!(
        "    /// Lazily fetches every page using this operation's declared pagination contract.\n    pub fn {pages_method}(&self, input: {request}) -> impl futures_util::Stream<Item = Result<{response}, {error}>> {{\n        let client = self.clone();\n{initial}        futures_util::stream::try_unfold((client, input, {state}, 0usize), |(client, mut input, has_next, page_count)| async move {{\n            if !has_next {{ return Ok(None); }}\n            if page_count >= 10000 {{ return Err({error}::Pagination(poolster_pagination_error(\"pagination exceeded 10000 pages\"))); }}\n            {before}            let response = client.{method_name}(input.clone()).await?;\n{next}\n        }})\n    }}\n\n"
    )
}

pub(crate) fn render_rust_url_iterator(method: &str, operation: &Operation, path: &str) -> String {
    let request = operation_request_name(operation);
    let response = operation_response_type(operation);
    let error = operation_error_name(operation);
    let (parameters, input, args) = if operation_has_parameters(operation) {
        (
            format!(", input: crate::client::operations::{request}"),
            "input",
            "input.clone(), ",
        )
    } else {
        (String::new(), "()", "")
    };
    format!(
        r#"    /// Lazy absolute same-origin next-URL pages. Relative URLs are rejected.
    pub fn {method}_pages(&self{parameters}) -> impl futures_util::Stream<Item=Result<{response},{error}>> {{
        let client=self.clone();
        futures_util::stream::try_unfold((client,{input},None::<String>,true,std::collections::HashSet::<String>::new(),0usize),|(client,input,url,has_next,mut seen,count)|async move {{
            if !has_next {{return Ok(None)}}
            if count>=10000 {{return Err({error}::Pagination(poolster_pagination_error("pagination exceeded 10000 pages")))}}
            if let Some(url)=&url {{if !seen.insert(url.clone()) {{return Err({error}::Pagination(poolster_pagination_error("repeated continuation URL")))}}}}
            let response=client.{method}_poolster_url({args}url.as_deref()).await?;
            let value=serde_json::to_value(&response).map_err({error}::Pagination)?;
            let next=match poolster_json_path(&value,{path:?}) {{
                None|Some(serde_json::Value::Null)=>None,
                Some(serde_json::Value::String(next)) if next.is_empty()=>None,
                Some(serde_json::Value::String(next))=>Some(next.clone()),
                _=>return Err({error}::Pagination(poolster_pagination_error("continuation URL must be a string"))),
            }};
            let has_next=next.is_some();Ok(Some((response,(client,input,next,has_next,seen,count+1))))
        }})
    }}
"#
    )
}
