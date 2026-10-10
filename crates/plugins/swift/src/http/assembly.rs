//! Assembly emission for the Swift HTTP SDK.
use crate::*;

/// Generates a self-contained Swift Package Manager SDK below `output_dir`.
pub(crate) fn render_sdk(
    api: &Api,
    output_dir: &str,
    package_name: Option<&str>,
    style: SdkClientStyle,
) -> Result<GeneratedTree> {
    let prepared = native_api(api);
    let api = &prepared;
    let prepared = multipart::prepare(api)?;
    let api = &prepared;
    let root = normalized_root(output_dir)?;
    let package = package_name
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{}-sdk", kebab_case(&api.name)));
    let module = type_name(&package);
    let pagination = pagination::render(api)?;
    let mut tree = GeneratedTree::default();
    multipart::emit(api, &root, &module, &mut tree)?;
    insert(
        &mut tree,
        &root,
        "Package.swift",
        package_manifest(&package, &module),
    )?;
    insert(
        &mut tree,
        &root,
        "README.md",
        readme(api, &package, &module, style)
            + if pagination.is_empty() {
                ""
            } else {
                "\n## Pagination\n\nDeclared string-cursor operations also expose `<operation>Pages(...)`. Their lazy `PoolsterCursorSequence` preserves the initial caller cursor, yields full responses, stops on missing/null/empty or repeated continuation tokens, and checks cancellation before each request. String query/header/path controls are supported; body and integer cursors fail generation.\n\nDeclared page-number operations expose `<operation>Pages(...)`, an `AsyncSequence` of full decoded pages. Iterate with `for try await page in client.<operation>Pages(...)`. Requests run only when the iterator advances and retain the ordinary operation transport, middleware, headers, and body. Optional page defaults to 1; explicit 0 is preserved. Required page remains a required argument. A positive declared limit stops after a short page; an empty page always stops and is yielded once. Invalid controls, malformed results selectors, integer overflow, and 10,000 pages terminate with `PoolsterPaginationError`. Cancellation is checked before each request. Offset/limit operations use the same lazy sequence, default offset to 0 and advance by the actual result count. URL continuation operations use `PoolsterURLSequence`, require absolute HTTP(S) URLs with the configured origin, preserve authentication and reject credentials, fragments, repeated URLs and relative continuations before sending a request. Page/offset/limit must be nonnullable scalar integer parameter controls; body-bound controls are rejected during generation.\n"
            },
    )?;
    insert(
        &mut tree,
        &root,
        "STYLE_GUIDE.md",
        style_guide(api, &module, style),
    )?;
    insert(
        &mut tree,
        &root,
        &format!("Sources/{module}/JSONValue.swift"),
        json_value(),
    )?;
    insert(
        &mut tree,
        &root,
        &format!("Sources/{module}/PoolsterClient.swift"),
        if api.operations.iter().any(operation_is_sse) {
            client_runtime_with_streaming()
        } else {
            client_runtime()
        },
    )?;
    for schema in &api.schemas {
        insert(
            &mut tree,
            &root,
            &format!(
                "Sources/{module}/Models/{}.swift",
                model_file_name(&schema.name)
            ),
            render_model_for_api(api, schema),
        )?;
    }
    // Split at complete operation boundaries by source bytes and declaration count.
    let mut operation_chunks: Vec<Vec<Operation>> = Vec::new();
    let mut current = Vec::new();
    let mut bytes = 1024;
    for operation in &api.operations {
        let length = render_operation(operation, "    ").len();
        if !current.is_empty() && (current.len() >= 100 || bytes + length > 128 * 1024) {
            operation_chunks.push(std::mem::take(&mut current));
            bytes = 1024;
        }
        current.push(operation.clone());
        bytes += length;
    }
    if !current.is_empty() {
        operation_chunks.push(current);
    }
    for (index, operations) in operation_chunks.into_iter().enumerate() {
        let mut chunk = api.as_ref().clone();
        chunk.operations = operations;
        let pagination = pagination::render(&chunk)?;
        let file = if index == 0 {
            "Operations.swift".to_owned()
        } else {
            format!("PoolsterOperations{index:03}.swift")
        };
        insert(
            &mut tree,
            &root,
            &format!("Sources/{module}/{file}"),
            render_operations(&chunk).replace("\n}\n", &format!("\n{pagination}\n}}\n")),
        )?;
    }
    if api.operations.iter().any(operation_is_sse) {
        insert(
            &mut tree,
            &root,
            &format!("Sources/{module}/Streaming.swift"),
            include_str!("../../templates/sse_runtime.swift.tmpl").into(),
        )?;
    }
    if !pagination.is_empty() {
        insert(
            &mut tree,
            &root,
            &format!("Sources/{module}/Pagination.swift"),
            include_str!("../../templates/page_runtime.swift.tmpl").into(),
        )?;
    }
    if style == SdkClientStyle::Namespaced {
        for (resource, operations) in operation_groups(api) {
            let mut groups: Vec<Vec<&Operation>> = Vec::new();
            let mut current = Vec::new();
            let mut bytes = 1024;
            for operation in operations {
                let length = render_resource(api, &resource, &[operation], true).len();
                if !current.is_empty() && (current.len() >= 100 || bytes + length > 128 * 1024) {
                    groups.push(std::mem::take(&mut current));
                    bytes = 1024;
                }
                current.push(operation);
                bytes += length;
            }
            if !current.is_empty() {
                groups.push(current);
            }
            for (index, operations) in groups.iter().enumerate() {
                let file = if index == 0 {
                    format!("{resource}Resource.swift")
                } else {
                    format!("PoolsterResource_{resource}_{index:03}.swift")
                };
                insert(
                    &mut tree,
                    &root,
                    &format!("Sources/{module}/Resources/{file}"),
                    render_resource(api, &resource, operations, index > 0),
                )?;
            }
        }
    }
    Ok(tree)
}

pub(crate) fn normalized_root(output_dir: &str) -> Result<String> {
    let root = output_dir.trim_matches('/');
    if root.is_empty() {
        bail!("Swift SDK output directory cannot be empty");
    }
    if root.split('/').any(|part| part == "..") {
        bail!("Swift SDK output directory cannot contain parent-directory components");
    }
    Ok(root.to_owned())
}

pub(crate) fn insert(
    tree: &mut GeneratedTree,
    root: &str,
    path: &str,
    contents: String,
) -> Result<()> {
    tree.insert(GeneratedFile::new(format!("{root}/{path}"), contents)?)
}

pub(crate) fn package_manifest(package: &str, module: &str) -> String {
    format!(
        "// swift-tools-version: 5.9\nimport PackageDescription\n\nlet package = Package(\n    name: {package:?},\n    platforms: [.macOS(.v13), .iOS(.v16)],\n    products: [.library(name: {module:?}, targets: [{module:?}])],\n    targets: [.target(name: {module:?})]\n)\n"
    )
}
