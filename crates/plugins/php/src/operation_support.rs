use super::*;

pub(super) fn php_source_ranges(
    sizes: &[usize],
    overhead: usize,
    count: usize,
) -> Vec<std::ops::Range<usize>> {
    let units = sizes
        .iter()
        .map(|bytes| poolster_core::source_layout::SourceUnit {
            bytes: *bytes,
            resource: None,
        })
        .collect::<Vec<_>>();
    poolster_core::source_layout::SourceLayout::Chunked {
        max_file_bytes: 128 * 1024,
        max_declarations: Some(count),
    }
    .groups(&units, overhead)
    .expect("valid bounded source layout")
    .into_iter()
    .map(|group| group[0]..group[group.len() - 1] + 1)
    .collect()
}

pub(super) fn php_operation_groups(
    api: &Api,
    namespace: &str,
    named_types: &NamedTypes,
) -> Vec<std::ops::Range<usize>> {
    let overhead = render_operation_trait(api, &[], 0, namespace, named_types).len();
    let sizes = api
        .operations
        .iter()
        .map(|operation| {
            render_operation_trait(
                api,
                std::slice::from_ref(operation),
                0,
                namespace,
                named_types,
            )
            .len()
            .saturating_sub(overhead)
        })
        .collect::<Vec<_>>();
    php_source_ranges(&sizes, overhead + 64, 100)
}

pub(super) fn php_resource_groups(
    api: &Api,
    resource: &str,
    operations: &[(&Operation, String)],
    namespace: &str,
    named_types: &NamedTypes,
) -> Vec<std::ops::Range<usize>> {
    let overhead = render_resource_trait(api, resource, &[], 0, namespace, named_types).len();
    let sizes = operations
        .iter()
        .map(|operation| {
            render_resource_trait(
                api,
                resource,
                std::slice::from_ref(operation),
                0,
                namespace,
                named_types,
            )
            .len()
            .saturating_sub(overhead)
        })
        .collect::<Vec<_>>();
    php_source_ranges(&sizes, overhead + 64, RESOURCE_METHODS_PER_FILE)
}

pub(super) fn render_operation_trait(
    api: &Api,
    operations: &[Operation],
    part: usize,
    namespace: &str,
    named_types: &NamedTypes,
) -> String {
    let mut output = format!(
        "<?php\n\ndeclare(strict_types=1);\n\nnamespace {namespace};\n\nuse {namespace}\\Exceptions\\ApiException;\nuse Psr\\Http\\Message\\StreamInterface;\n"
    );
    for name in operation_model_imports(operations, named_types) {
        let _ = writeln!(output, "use {namespace}\\Models\\{name};");
    }
    let _ = writeln!(
        output,
        "\n{NOTICE}\n/** Bounded generated operation slice; composed into Client. */\ntrait ClientOperations{part:03}\n{{"
    );
    for operation in operations {
        output.push_str(&render_operation(operation, namespace, named_types));
        if let Some(page) = page_pagination::render(api, operation, named_types) {
            output.push_str(&page);
        } else if let Some(pagination) = cursor_pagination(api, operation) {
            output.push_str(&render_cursor_paginator(
                operation,
                named_types,
                &pagination,
            ));
        } else if let Some(pagination) = offset_pagination(operation) {
            output.push_str(&render_offset_paginator(
                operation,
                named_types,
                &pagination,
            ));
        } else if let Some(pagination) = url_pagination(operation) {
            output.push_str(&render_url_paginator(operation, named_types, &pagination));
        }
    }
    output.push_str("}\n");
    output
}

pub(super) fn json_content_allows_null(parameter: &poolster_core::OperationParameter) -> bool {
    fn allows(value: &SchemaValue) -> bool {
        value.nullable
            || matches!(value.kind, SchemaKind::Any | SchemaKind::Null)
            || match &value.kind {
                SchemaKind::OneOf { variants } | SchemaKind::AnyOf { variants } => {
                    variants.iter().any(allows)
                }
                _ => false,
            }
    }
    parameter.schema.as_ref().is_none_or(allows)
}

pub(super) fn is_json_parameter_content(parameter: &poolster_core::OperationParameter) -> bool {
    poolster_core::openapi32::parameter_content(parameter)
        .ok()
        .and_then(|items| items.into_iter().next())
        .is_some_and(|content| {
            content.content_type == "application/json" || content.content_type.ends_with("+json")
        })
}

pub(super) fn operation_is_binary_response(operation: &Operation) -> bool {
    matches!(
        analyze_operation(operation, None).streaming,
        Some(StreamingKind::Binary)
    )
}

pub(super) fn operation_is_sse_response(operation: &Operation) -> bool {
    matches!(
        analyze_operation(operation, None).streaming,
        Some(StreamingKind::ServerSentEvents)
    )
}

pub(super) fn operation_has_multipart(operation: &Operation) -> bool {
    operation.request_body.as_ref().is_some_and(|body| {
        body.media_types
            .iter()
            .any(|media| media.content_type.starts_with("multipart/"))
    })
}

pub(super) fn operation_body_kind(operation: &Operation) -> &'static str {
    if operation_has_multipart(operation) {
        return if operation.request_body.as_ref().is_some_and(|body| {
            body.media_types.iter().any(|media| {
                media.content_type == "application/json" || media.content_type.ends_with("+json")
            })
        }) {
            "multipart-json"
        } else {
            "multipart"
        };
    }

    operation
        .request_body
        .as_ref()
        .and_then(|body| body.media_types.first())
        .map(|media| match media.content_type.as_str() {
            "application/octet-stream" => "binary",
            "application/x-www-form-urlencoded" => "form",
            _ => "json",
        })
        .unwrap_or("json")
}
