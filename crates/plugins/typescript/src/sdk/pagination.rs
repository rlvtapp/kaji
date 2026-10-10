use super::*;

#[derive(Clone, Debug)]
pub(crate) struct CursorPagination {
    pub(crate) input_name: String,
    pub(crate) input_location: String,
    pub(crate) input_body_path: Option<String>,
    pub(crate) next_cursor_path: String,
}

#[derive(Clone, Debug)]
pub(crate) struct PaginationInput {
    pub(crate) name: String,
    pub(crate) location: String,
    /// An RFC 6901 JSON Pointer into a JSON request body. This is intentionally
    /// a separate, opt-in field: an input name is a wire name, not a safe way
    /// to infer where a nested value lives in a request model.
    pub(crate) body_path: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct OffsetPagination {
    pub(crate) step: OffsetStep,
    pub(crate) limit: Option<PaginationInput>,
    pub(crate) results_path: Option<String>,
    pub(crate) num_pages_path: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct UrlPagination {
    pub(crate) next_url_path: String,
}

#[derive(Clone, Debug)]
pub(crate) enum OffsetStep {
    Page(PaginationInput),
    Offset(PaginationInput),
}

#[derive(Clone, Debug)]
pub(crate) enum Pagination {
    Cursor(CursorPagination),
    OffsetLimit(OffsetPagination),
    Url(UrlPagination),
}

pub(crate) fn pagination(operation: &Operation) -> Option<Pagination> {
    let extension = poolster_core::poolster_extension(&operation.annotations, "pagination")
        .or_else(|| operation.annotations.get("x-speakeasy-pagination"))?
        .as_object()?;
    let outputs = extension.get("outputs")?.as_object()?;
    match extension.get("type").and_then(Value::as_str) {
        Some("cursor") => {
            let inputs = extension.get("inputs").and_then(Value::as_array)?;
            let input = pagination_input(operation, inputs, "cursor")?;
            Some(Pagination::Cursor(CursorPagination {
                input_name: input.name,
                input_location: input.location,
                input_body_path: input.body_path,
                next_cursor_path: outputs.get("nextCursor")?.as_str()?.to_owned(),
            }))
        }
        Some("offsetLimit" | "page") => {
            let inputs = extension.get("inputs").and_then(Value::as_array)?;
            let input = |kind| pagination_input(operation, inputs, kind);
            let page = input("page");
            let offset = input("offset");
            let step = match (page, offset) {
                (Some(page), _) => OffsetStep::Page(page),
                (None, Some(offset)) => OffsetStep::Offset(offset),
                (None, None) => return None,
            };
            let results_path = outputs
                .get("results")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let num_pages_path = outputs
                .get("numPages")
                .and_then(Value::as_str)
                .map(str::to_owned);
            match step {
                OffsetStep::Page(_) if results_path.is_none() && num_pages_path.is_none() => None,
                OffsetStep::Offset(_) if results_path.is_none() => None,
                _ => Some(Pagination::OffsetLimit(OffsetPagination {
                    step,
                    limit: input("limit"),
                    results_path,
                    num_pages_path,
                })),
            }
        }
        Some("url") => Some(Pagination::Url(UrlPagination {
            next_url_path: outputs.get("nextUrl")?.as_str()?.to_owned(),
        })),
        _ => None,
    }
}

pub(crate) fn pagination_input(
    operation: &Operation,
    inputs: &[Value],
    kind: &str,
) -> Option<PaginationInput> {
    let input = inputs
        .iter()
        .find(|input| input.get("type").and_then(Value::as_str) == Some(kind))?
        .as_object()?;
    let name = input.get("name")?.as_str()?.to_owned();
    let location = match input.get("in").and_then(Value::as_str) {
        Some("requestBody") => "body".into(),
        Some("parameters") | None => operation
            .parameters
            .iter()
            .find(|parameter| parameter.name == name)
            .map(|parameter| typescript_options_location(&parameter.location))
            .unwrap_or_else(|| "query".into()),
        Some(location) => typescript_options_location(location),
    };
    let body_path = if location == "body" {
        let path = input
            .get("bodyPath")
            .and_then(Value::as_str)
            .map(str::to_owned)
            // Preserve the original top-level request-body convention. Nested
            // body values must opt in with `bodyPath`; there is no schema-path
            // inference hidden behind this fallback.
            .unwrap_or_else(|| format!("/{}", json_pointer_escape(&name)));
        json_pointer_targets_name(&path, &name).then_some(path)
    } else {
        None
    };
    if location == "body" && (body_path.is_none() || !operation_has_json_request_body(operation)) {
        return None;
    }
    Some(PaginationInput {
        name,
        location,
        body_path,
    })
}

pub(crate) fn operation_has_json_request_body(operation: &Operation) -> bool {
    operation.request_body.as_ref().is_some_and(|body| {
        body.media_types.iter().any(|media| {
            media.content_type.eq_ignore_ascii_case("application/json")
                || media.content_type.to_ascii_lowercase().ends_with("+json")
        })
    })
}

pub(crate) fn json_pointer_escape(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

/// Confirms the explicit pointer is structurally valid and ends at the input's
/// declared wire name. This keeps an accidental `bodyPath` typo from updating
/// a different body field while still allowing optional intermediate objects.
pub(crate) fn json_pointer_targets_name(pointer: &str, name: &str) -> bool {
    let Some(last) = pointer
        .strip_prefix('/')
        .and_then(|path| path.rsplit('/').next())
    else {
        return false;
    };
    let unescaped = last.replace("~1", "/").replace("~0", "~");
    !pointer.contains("//") && unescaped == name
}

pub(crate) fn typescript_options_location(location: &str) -> String {
    match location {
        "header" => "headers".into(),
        "requestBody" => "body".into(),
        location => location.into(),
    }
}

pub(crate) fn has_pagination(api: &Api) -> bool {
    api.operations
        .iter()
        .any(|operation| pagination(operation).is_some())
}

pub(crate) fn render_pagination_iterator(
    public_name: &str,
    function: &str,
    pagination: &Pagination,
) -> String {
    render_pagination_iterator_with_client(public_name, function, pagination, "this.transport")
}

pub(crate) fn render_pagination_iterator_with_client(
    public_name: &str,
    function: &str,
    pagination: &Pagination,
    client_expression: &str,
) -> String {
    if let Pagination::Url(pagination) = pagination {
        return render_url_pagination_iterator(
            public_name,
            function,
            pagination,
            client_expression,
        );
    }
    let initial = if let Pagination::OffsetLimit(OffsetPagination {
        step: OffsetStep::Page(input),
        ..
    }) = pagination
    {
        let value = pagination_input_value(input);
        let update = pagination_input_update(input, "initialPage");
        let validate_limit = if let Pagination::OffsetLimit(OffsetPagination {
            limit: Some(limit),
            ..
        }) = pagination
        {
            let value = pagination_input_value(limit);
            format!(
                "        const initialLimit = {value}\n        if (initialLimit !== undefined && initialLimit !== null && (!Number.isSafeInteger(initialLimit) || Number(initialLimit) <= 0)) throw new Error('limit must be a positive safe integer')\n"
            )
        } else {
            String::new()
        };
        format!(
            "{validate_limit}        const initialPage = {value} ?? 1\n        if (!Number.isSafeInteger(initialPage) || Number(initialPage) < 0) throw new Error('page must be a nonnegative safe integer')\n        current = {update}\n        if (current === undefined) throw new Error('invalid pagination request body')\n"
        )
    } else {
        String::new()
    };
    let header = format!(
        "    {{\n      const paginationClient = {client_expression}\n      this.{public_name}Pages = async function* (options: Parameters<typeof {function}>[0]) {{\n        let current: unknown = options\n{initial}        for (let pageCount = 0; pageCount < 10000; pageCount++) {{\n          const response = await {function}({{ ...(current as Record<string, unknown>), client: paginationClient }} as Parameters<typeof {function}>[0])\n          yield response\n"
    );
    let footer =
        "        }\n        throw new Error('pagination exceeded 10000 pages')\n      }\n    }\n";
    let body = match pagination {
        Pagination::Cursor(pagination) => {
            let update = if pagination.input_location == "body" {
                format!(
                    "poolsterWithBodyValue(current, {:?}, cursor)",
                    pagination
                        .input_body_path
                        .as_deref()
                        .expect("validated request body cursor")
                )
            } else {
                format!(
                    "poolsterWithValue(current, {:?}, {:?}, cursor)",
                    pagination.input_location, pagination.input_name
                )
            };
            format!(
                "          const cursor = poolsterJsonPath(response, {:?})\n          if (cursor === undefined || cursor === null || cursor === '') return\n          const next = {update}\n          if (next === undefined) return\n          current = next\n",
                pagination.next_cursor_path
            )
        }
        Pagination::OffsetLimit(pagination) => render_offset_pagination(pagination),
        Pagination::Url(_) => unreachable!("URL pagination has its own iterator renderer"),
    };
    format!("{header}{body}{footer}")
}

/// URL pagination deliberately re-enters the generated operation through a
/// small `ClientInstance` wrapper instead of making a bare request. This is
/// important: the operation continues to supply its declared method,
/// security descriptor, serialization settings, and error policy. The
/// runtime treats `paginationUrl` as an internal, same-origin continuation
/// target; it is not a general-purpose URL override on public SDK methods.
pub(crate) fn render_url_pagination_iterator(
    public_name: &str,
    function: &str,
    pagination: &UrlPagination,
    client_expression: &str,
) -> String {
    format!(
        "    {{\n      const paginationClient = {client_expression}\n      this.{public_name}Pages = async function* (options: Parameters<typeof {function}>[0]) {{\n        let current: unknown = options\n        let nextUrl: string | undefined\n        while (true) {{\n          const request: ClientInstance = nextUrl === undefined\n            ? paginationClient\n            : (operation) => paginationClient({{ ...operation, paginationUrl: nextUrl }})\n          const requestOptions = nextUrl === undefined\n            ? (current as Record<string, unknown>)\n            : {{ ...(current as Record<string, unknown>), query: undefined }}\n          const response = await {function}({{ ...requestOptions, client: request }} as Parameters<typeof {function}>[0])\n          yield response\n          nextUrl = poolsterPaginationUrl(response, {:?})\n          if (nextUrl === undefined) return\n        }}\n      }}\n    }}\n",
        pagination.next_url_path,
    )
}

pub(crate) fn render_offset_pagination(pagination: &OffsetPagination) -> String {
    let (input, increment) = match &pagination.step {
        OffsetStep::Page(input) => (input, "currentValue + 1"),
        OffsetStep::Offset(input) => (input, "currentValue + items.length"),
    };
    let mut output = String::new();
    if let (OffsetStep::Page(_), Some(num_pages_path)) =
        (&pagination.step, &pagination.num_pages_path)
    {
        let current_value = pagination_input_value(input);
        let update = pagination_input_update(input, "nextValue");
        output.push_str(&format!(
            "          const currentValue = Number({current_value})\n          const nextValue = currentValue + 1\n          const numPages = Number(poolsterJsonPath(response, {:?}))\n          if (!Number.isSafeInteger(currentValue) || !Number.isSafeInteger(nextValue) || !Number.isSafeInteger(numPages) || numPages < 0 || nextValue > numPages) return\n          const next = {update}\n          if (next === undefined) return\n          current = next\n",
            num_pages_path,
        ));
        return output;
    }

    let results_path = pagination
        .results_path
        .as_deref()
        .expect("validated offset pagination has a results path");
    let limit = pagination.limit.as_ref().map_or_else(
        || "NaN".to_owned(),
        |input| format!("Number({})", pagination_input_value(input)),
    );
    let current_value = pagination_input_value(input);
    let update = pagination_input_update(input, "nextValue");
    output.push_str(&format!(
        "          const items = poolsterJsonPath(response, {:?})\n          if (!Array.isArray(items)) return\n          const configuredLimit = {limit}\n          if (items.length === 0 || (Number.isFinite(configuredLimit) && configuredLimit > 0 && items.length < configuredLimit)) return\n          const currentValue = Number({current_value})\n          const nextValue = {increment}\n          if (!Number.isSafeInteger(currentValue) || currentValue < 0 || !Number.isSafeInteger(nextValue)) return\n          const next = {update}\n          if (next === undefined) return\n          current = next\n",
        results_path,
    ));
    output
}

pub(crate) fn pagination_input_value(input: &PaginationInput) -> String {
    if input.location == "body" {
        format!(
            "poolsterBodyValue(current, {:?})",
            input
                .body_path
                .as_deref()
                .expect("validated request body pagination input")
        )
    } else {
        format!(
            "poolsterOptionValue(current, {:?}, {:?})",
            input.location, input.name
        )
    }
}

pub(crate) fn pagination_input_update(input: &PaginationInput, value: &str) -> String {
    if input.location == "body" {
        format!(
            "poolsterWithBodyValue(current, {:?}, {value})",
            input
                .body_path
                .as_deref()
                .expect("validated request body pagination input")
        )
    } else {
        format!(
            "poolsterWithValue(current, {:?}, {:?}, {value})",
            input.location, input.name
        )
    }
}

pub(crate) fn pagination_helpers() -> &'static str {
    "\nconst poolsterJsonPath = (value: unknown, path: string): unknown => {\n  const segments: (string | number)[] = []\n  if (path.startsWith('/')) {\n    for (const part of path.slice(1).split('/')) {\n      if (/~(?![01])/.test(part)) return undefined\n      segments.push(part.replace(/~1/g, '/').replace(/~0/g, '~'))\n    }\n  } else if (path.startsWith('$')) {\n    let rest = path.slice(1)\n    while (rest) {\n      const match = /^(?:\\.([^.[\\]]+)|\\[(-?\\d+)\\])/.exec(rest)\n      if (!match) return undefined\n      segments.push(match[1] ?? Number(match[2]))\n      rest = rest.slice(match[0].length)\n    }\n  } else return undefined\n  let current: unknown = value\n  for (const segment of segments) {\n    if (Array.isArray(current)) {\n      if (typeof segment === 'string' && !/^(?:0|[1-9]\\d*)$/.test(segment)) return undefined\n      const index = Number(segment)\n      if (!Number.isSafeInteger(index)) return undefined\n      const position = index < 0 ? current.length + index : index\n      if (!Object.hasOwn(current, position)) return undefined\n      current = current[position]\n    } else if (current && typeof current === 'object' && Object.hasOwn(current, segment)) current = (current as Record<string, unknown>)[segment]\n    else return undefined\n  }\n  return current\n}\nconst poolsterPaginationUrl = (response: unknown, path: string): string | undefined => {\n  const value = poolsterJsonPath(response, path)\n  return typeof value === 'string' && value.trim() ? value : undefined\n}\nconst poolsterOptionValue = (options: unknown, location: string, name: string): unknown => {\n  const current = (options ?? {}) as Record<string, unknown>\n  const section = current[location] as Record<string, unknown> | undefined\n  return section?.[name]\n}\nconst poolsterWithValue = (options: unknown, location: string, name: string, value: unknown): Record<string, unknown> => {\n  const current = (options ?? {}) as Record<string, unknown>\n  const section = (current[location] ?? {}) as Record<string, unknown>\n  return { ...current, [location]: { ...section, [name]: value } }\n}\n// `bodyPath` is an explicit RFC 6901 JSON Pointer. This helper creates new\n// objects/arrays only along that declared path; it never mutates caller input\n// and refuses to invent a missing array shape.\nconst poolsterJsonPointer = (path: string): string[] | undefined => {\n  if (!path.startsWith('/') || path.includes('//')) return undefined\n  return path.slice(1).split('/').map((part) => part.split('~1').join('/').split('~0').join('~'))\n}\nconst poolsterBodyValue = (options: unknown, path: string): unknown => {\n  const current = (options ?? {}) as Record<string, unknown>\n  const pointer = poolsterJsonPointer(path)\n  if (!pointer) return undefined\n  let value: unknown = current.body\n  for (const key of pointer) {\n    if (value === null || typeof value !== 'object') return undefined\n    value = Array.isArray(value) ? value[Number(key)] : (value as Record<string, unknown>)[key]\n  }\n  return value\n}\nconst poolsterWithBodyValue = (options: unknown, path: string, value: unknown): Record<string, unknown> | undefined => {\n  const current = (options ?? {}) as Record<string, unknown>\n  const pointer = poolsterJsonPointer(path)\n  if (!pointer || pointer.length === 0) return undefined\n  const update = (node: unknown, index: number): unknown | undefined => {\n    const key = pointer[index]\n    if (Array.isArray(node)) {\n      if (!/^\\d+$/.test(key)) return undefined\n      const position = Number(key)\n      if (!Number.isSafeInteger(position) || position < 0 || position >= node.length) return undefined\n      const copy = node.slice()\n      const next = index + 1 === pointer.length ? value : update(node[position], index + 1)\n      if (next === undefined) return undefined\n      copy[position] = next\n      return copy\n    }\n    if (node !== null && typeof node === 'object') {\n      const record = node as Record<string, unknown>\n      const next = index + 1 === pointer.length ? value : update(record[key] ?? {}, index + 1)\n      if (next === undefined) return undefined\n      return { ...record, [key]: next }\n    }\n    // An absent optional object can be created, but scalar and array shapes\n    // remain unrepresentable without a schema-directed declaration.\n    if (node === undefined || node === null) return update({}, index)\n    return undefined\n  }\n  const body = update(current.body ?? {}, 0)\n  return body === undefined ? undefined : { ...current, body }\n}\n"
}
