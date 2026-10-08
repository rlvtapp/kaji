use super::*;

pub(super) fn resource_operations(api: &Api) -> BTreeMap<String, Vec<&Operation>> {
    let mut resources = BTreeMap::<String, Vec<&Operation>>::new();
    for operation in &api.operations {
        resources
            .entry(resource_name(operation))
            .or_default()
            .push(operation);
    }
    resources
}

pub(super) fn resource_name(operation: &Operation) -> String {
    operation
        .annotations
        .get("tags")
        .and_then(|tags| tags.as_array())
        .and_then(|tags| tags.first())
        .and_then(|tag| tag.as_str())
        .filter(|tag| !tag.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| {
            operation
                .path
                .split('/')
                .filter(|segment| !segment.is_empty() && !segment.starts_with('{'))
                .find(|segment| {
                    !matches!(*segment, "api" | "email") && !is_version_segment(segment)
                })
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "api".into())
}

pub(super) fn is_version_segment(segment: &str) -> bool {
    let mut characters = segment.chars();
    matches!(characters.next(), Some('v' | 'V'))
        && characters
            .next()
            .is_some_and(|character| character.is_ascii_digit())
}

pub(super) fn render_resource_chunk(
    api: &Api,
    resource: &str,
    all_operations: &[&Operation],
    operations: &[&Operation],
    index: usize,
) -> String {
    let class = format!("{}Resource", pascal_case(resource));
    let attribute = resource_attribute(resource, operations);
    let parent = if index == 0 {
        String::new()
    } else {
        format!("({class}Part{:03})", index - 1)
    };
    let mut output = format!(
        "{NOTICE}from __future__ import annotations\n\nfrom typing import Any, Iterator\nfrom ..multipart import MultipartBody\n\n{}\n\nclass {class}Part{index:03}{parent}:\n    \"\"\"Bounded typed {attribute} resource operations.\"\"\"\n\n    def __init__(self, client: Any) -> None:\n        self._client = client\n",
        if index == 0 {
            String::new()
        } else {
            format!(
                "from .{}_part_{:03} import {class}Part{:03}",
                schema_file_name(resource),
                index - 1,
                index - 1
            )
        },
    );
    // Resource methods must remain unique across parts. Seed this partition's
    // name allocator with every preceding operation exactly as the original
    // single-file renderer did.
    let mut used_methods = BTreeMap::<String, usize>::new();
    let start = operations
        .first()
        .and_then(|first| {
            all_operations
                .iter()
                .position(|item| std::ptr::eq(*item, *first))
        })
        .unwrap_or(0);
    for operation in all_operations.iter().take(start) {
        let direct = python_identifier(&snake_case(&operation.id));
        let preferred = resource_method_name(&direct, resource);
        let previously_seen = *used_methods.get(&preferred).unwrap_or(&0);
        if previously_seen != 0 {
            used_methods.entry(direct).or_default();
        }
        *used_methods.entry(preferred).or_default() += 1;
    }
    for operation in operations {
        let direct = python_identifier(&snake_case(&operation.id));
        let preferred = resource_method_name(&direct, &attribute);
        let seen = used_methods.entry(preferred.clone()).or_default();
        let method = if *seen == 0 {
            preferred
        } else {
            direct.clone()
        };
        *seen += 1;
        let response = response_type(operation);
        let (signature, arguments) = operation_signature(operation);
        let forwarded = arguments
            .iter()
            .map(|argument| format!("{argument}={argument}"))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            output,
            "    def {method}(self{signature}) -> {response}:\n        return self._client.{direct}({forwarded})\n"
        );
        if render_page_paginator(api, operation).is_some()
            || cursor_pagination(api, operation).is_some()
            || offset_pagination(operation).is_some()
            || url_pagination(operation).is_some()
        {
            let _ = writeln!(
                output,
                "    def {method}_pages(self{signature}) -> Iterator[{response}]:\n        return self._client.{direct}_pages({forwarded})\n"
            );
        }
    }
    output
}

pub(super) fn render_resource_facade(resource: &str, chunks: usize) -> String {
    let class = format!("{}Resource", pascal_case(resource));
    let file = schema_file_name(resource);
    let last = chunks.saturating_sub(1);
    format!(
        "{NOTICE}from __future__ import annotations\n\nfrom .{file}_part_{last:03} import {class}Part{last:03}\n\n\nclass {class}({class}Part{last:03}):\n    \"\"\"Typed resource namespace exposed by :class:`Client`.\"\"\"\n\n    pass\n"
    )
}

pub(super) fn resource_method_name(operation: &str, resource: &str) -> String {
    let singular = resource.strip_suffix('s').unwrap_or(resource);
    let method = operation
        .strip_suffix(resource)
        .or_else(|| operation.strip_suffix(singular))
        .or_else(|| operation.strip_prefix(resource))
        .unwrap_or(operation)
        .trim_matches('_');
    if method.is_empty() {
        operation.into()
    } else {
        python_identifier(method)
    }
}

/// Uses the resource name unless it would hide one of Client's direct
/// generated methods. This keeps the flat surface available in namespaced
/// mode: an `auth-check` resource with `authCheck` becomes
/// `client.auth_check_resource`, while `client.auth_check(...)` still works.
pub(super) fn resource_attribute(resource: &str, operations: &[&Operation]) -> String {
    let direct_methods = operations
        .iter()
        .map(|operation| python_identifier(&snake_case(&operation.id)))
        .collect::<std::collections::BTreeSet<_>>();
    let mut attribute = python_identifier(resource);
    while direct_methods.contains(&attribute) || matches!(attribute.as_str(), "for_call" | "aclose")
    {
        attribute.push_str("_resource");
    }
    attribute
}
