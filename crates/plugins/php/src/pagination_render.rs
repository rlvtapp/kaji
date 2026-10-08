use super::*;

pub(super) fn render_cursor_paginator(
    operation: &Operation,
    named_types: &NamedTypes,
    pagination: &CursorPagination,
) -> String {
    let response = response_schema(operation)
        .map(|schema| php_type(schema, named_types))
        .unwrap_or_else(|| "void".into());
    let arguments = facade_arguments(operation, named_types);
    let declaration = arguments
        .iter()
        .map(|(_, declaration)| declaration.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let (state, invocation, update) = match &pagination.input {
        CursorInput::Parameter(parameter_name) => {
            let invocation = arguments
                .iter()
                .map(|(variable, _)| {
                    if variable == parameter_name {
                        "$poolsterCursor".to_owned()
                    } else {
                        format!("${variable}")
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            (
                format!("        $poolsterCursor = ${parameter_name};\n"),
                invocation,
                "            $poolsterCursor = $nextCursor;\n".to_owned(),
            )
        }
        CursorInput::Body(wire_name) => {
            let invocation = arguments
                .iter()
                .map(|(variable, _)| {
                    if variable == "body" {
                        "$poolsterBody".to_owned()
                    } else {
                        format!("${variable}")
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            (
                "        $poolsterBody = $body;\n".to_owned(),
                invocation,
                format!(
                    "            $poolsterBody = self::poolsterWithBodyValue($poolsterBody, {}, $nextCursor);\n",
                    php_string(wire_name)
                ),
            )
        }
    };
    format!(
        "    /** @return \\Generator<int, {response}> */\n    public function {}Pages({declaration}): \\Generator\n    {{\n{state}        while (true) {{\n            $response = $this->{}({invocation});\n            yield $response;\n            $nextCursor = self::poolsterJsonPath($response, {});\n            if ($nextCursor === null || $nextCursor === '') {{\n                return;\n            }}\n{update}        }}\n    }}\n\n",
        method_name(&operation.id),
        method_name(&operation.id),
        php_string(&pagination.next_cursor_path),
    )
}

pub(super) fn render_offset_paginator(
    operation: &Operation,
    named_types: &NamedTypes,
    pagination: &OffsetPagination,
) -> String {
    let response = response_schema(operation)
        .map(|schema| php_type(schema, named_types))
        .unwrap_or_else(|| "void".into());
    let arguments = facade_arguments(operation, named_types);
    let declaration = arguments
        .iter()
        .map(|(_, declaration)| declaration.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let invocation = arguments
        .iter()
        .map(|(variable, _)| {
            if variable == &pagination.offset_name {
                "$poolsterOffset".to_owned()
            } else if variable == &pagination.limit_name {
                "$poolsterLimit".to_owned()
            } else {
                format!("${variable}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "    /** @return \\Generator<int, {response}> */\n    public function {}Pages({declaration}): \\Generator\n    {{\n        $poolsterOffset = ${} ?? 0;\n        $poolsterLimit = ${};\n        while (true) {{\n            $response = $this->{}({invocation});\n            yield $response;\n            $results = self::poolsterJsonPath($response, {});\n            if (!is_array($results) || $results === []) {{\n                return;\n            }}\n            $poolsterOffset += count($results);\n            if ($poolsterLimit !== null && count($results) < $poolsterLimit) {{\n                return;\n            }}\n        }}\n    }}\n\n",
        method_name(&operation.id),
        pagination.offset_name,
        pagination.limit_name,
        method_name(&operation.id),
        php_string(&pagination.results_path),
    )
}

pub(super) fn render_url_paginator(
    operation: &Operation,
    named_types: &NamedTypes,
    pagination: &UrlPagination,
) -> String {
    let response = response_schema(operation)
        .map(|schema| php_type(schema, named_types))
        .unwrap_or_else(|| "void".into());
    let arguments = facade_arguments(operation, named_types);
    let declaration = arguments
        .iter()
        .map(|(_, declaration)| declaration.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let invocation = arguments
        .iter()
        .map(|(variable, _)| format!("${variable}"))
        .collect::<Vec<_>>()
        .join(", ");
    let call = if invocation.is_empty() {
        format!(
            "$this->{}(_poolsterPaginationUrl: $poolsterUrl)",
            method_name(&operation.id)
        )
    } else {
        format!(
            "$this->{}({invocation}, _poolsterPaginationUrl: $poolsterUrl)",
            method_name(&operation.id)
        )
    };
    format!(
        "    /** @return \\Generator<int, {response}> */\n    public function {}Pages({declaration}): \\Generator\n    {{\n        $poolsterUrl = null;\n        while (true) {{\n            $response = {call};\n            yield $response;\n            $nextUrl = self::poolsterJsonPath($response, {});\n            if (!is_string($nextUrl) || $nextUrl === '') {{\n                return;\n            }}\n            $poolsterUrl = $nextUrl;\n        }}\n    }}\n\n",
        method_name(&operation.id),
        php_string(&pagination.next_url_path),
    )
}

pub(super) fn render_pagination_helper() -> &'static str {
    include_str!("../templates/pagination_helpers.php.tmpl")
}
