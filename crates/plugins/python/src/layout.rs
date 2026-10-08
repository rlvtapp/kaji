use super::*;

pub(super) fn bounded_source_ranges(
    sizes: &[usize],
    overhead: usize,
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
        max_declarations: Some(100),
    }
    .groups(&units, overhead)
    .expect("valid bounded source layout")
    .into_iter()
    .map(|group| group[0]..group[group.len() - 1] + 1)
    .collect()
}

pub(super) fn python_operation_groups(api: &Api) -> Vec<std::ops::Range<usize>> {
    let overhead = render_operation_chunk(api, &[], 0).len();
    let sizes = api
        .operations
        .iter()
        .map(|operation| {
            render_operation_chunk(api, std::slice::from_ref(operation), 0)
                .len()
                .saturating_sub(overhead)
        })
        .collect::<Vec<_>>();
    bounded_source_ranges(&sizes, overhead + 64)
}

pub(super) fn python_resource_groups(
    api: &Api,
    resource: &str,
    operations: &[&Operation],
) -> Vec<std::ops::Range<usize>> {
    let overhead = render_resource_chunk(api, resource, operations, &[], 0).len();
    let sizes = operations
        .iter()
        .map(|operation| {
            render_resource_chunk(
                api,
                resource,
                operations,
                std::slice::from_ref(operation),
                0,
            )
            .len()
            .saturating_sub(overhead)
        })
        .collect::<Vec<_>>();
    bounded_source_ranges(&sizes, overhead + 512)
}

pub(super) fn render_operation_chunk(api: &Api, operations: &[Operation], index: usize) -> String {
    let mut output = format!(
        "{NOTICE}from __future__ import annotations\n\nfrom typing import Any, Iterator, cast\nfrom uuid import uuid4\nfrom urllib.parse import quote\n\nfrom .runtime import ApiError, _poolster_json_path, _poolster_with_body_value, to_wire\nfrom .multipart import MultipartBody\nfrom .models import *\n\n\nclass Operations{index:03}:\n"
    );
    for operation in operations {
        output.push_str(&render_operation(api, operation));
        if let Some(page) = render_page_paginator(api, operation) {
            output.push_str(&page);
        } else if let Some(pagination) = cursor_pagination(api, operation) {
            output.push_str(&render_cursor_paginator(api, operation, &pagination));
        } else if let Some(pagination) = offset_pagination(operation) {
            output.push_str(&render_offset_paginator(operation, &pagination));
        } else if let Some(pagination) = url_pagination(operation) {
            output.push_str(&render_url_paginator(operation, &pagination));
        }
    }
    output
}

pub(super) fn render_client_facade(api: &Api, client_style: SdkClientStyle) -> String {
    let operation_imports = (0..python_operation_groups(api).len())
        .map(|index| format!("from .operations_{index:03} import Operations{index:03}"))
        .collect::<Vec<_>>();
    let resources = resource_operations(api);
    let mut output = format!(
        "{NOTICE}from __future__ import annotations\n\nfrom .runtime import BaseClient\n{}\n",
        operation_imports.join("\n")
    );
    if client_style == SdkClientStyle::Namespaced {
        for resource in resources.keys() {
            let class = format!("{}Resource", pascal_case(resource));
            let file = schema_file_name(resource);
            let _ = writeln!(output, "from .resources.{file} import {class}");
        }
    }
    let bases = operation_imports
        .iter()
        .enumerate()
        .map(|(index, _)| format!("Operations{index:03}"))
        .chain(std::iter::once("BaseClient".to_owned()))
        .collect::<Vec<_>>();
    output.push_str(&format!("\n\nclass Client({}):\n", bases.join(", ")));
    if client_style == SdkClientStyle::Namespaced {
        output.push_str("    def __init__(self, *args, **kwargs) -> None:\n        super().__init__(*args, **kwargs)\n");
        for (resource, operations) in &resources {
            let _ = writeln!(
                output,
                "        self.{} = {}Resource(self)",
                resource_attribute(resource, operations),
                pascal_case(resource)
            );
        }
    } else {
        output.push_str("    pass\n");
    }
    output
}
