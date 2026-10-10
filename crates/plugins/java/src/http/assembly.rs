//! Assembly emission for the java HTTP SDK.
use crate::*;

/// Generates a Java 17+ SDK below `output_dir`.
///
/// `package_name` is a Java package name, such as `com.poolster.email`. When
/// absent, one is deterministically derived under `io.poolster`. The returned
/// tree is not written automatically, allowing profiles to compose multiple
/// language targets safely before materializing them.
#[cfg(test)]
pub(crate) fn render_test_sdk(
    api: &Api,
    output_dir: &str,
    package_name: Option<&str>,
) -> Result<GeneratedTree> {
    render_sdk(api, output_dir, package_name, SdkClientStyle::Flat)
}

/// Generates a Java 17+ SDK with either the original direct-operation client
/// or a resource-namespaced facade. In namespaced mode the direct operations
/// remain available as direct entry points.
#[cfg(test)]
pub(crate) fn render_sdk(
    api: &Api,
    output_dir: &str,
    package_name: Option<&str>,
    style: SdkClientStyle,
) -> Result<GeneratedTree> {
    render_sdk_with_policy(api, output_dir, package_name, style, false)
}

pub(crate) fn render_sdk_with_policy(
    api: &Api,
    output_dir: &str,
    package_name: Option<&str>,
    style: SdkClientStyle,
    open_enums: bool,
) -> Result<GeneratedTree> {
    let prepared = prepare_api(api);
    let api = &prepared;
    multipart::validate(api)?;

    for operation in &api.operations {
        let extension = poolster_core::poolster_extension(&operation.annotations, "pagination")
            .or_else(|| operation.annotations.get("x-speakeasy-pagination"));
        if extension
            .and_then(|extension| extension.get("type"))
            .and_then(Value::as_str)
            == Some("page")
        {
            poolster_core::pagination::normalize_pagination(api, operation, None)?;
        }
    }
    let root = normalized_output_dir(output_dir)?;
    let package = package_name
        .filter(|name| !name.trim().is_empty())
        .map(java_package_name)
        .unwrap_or_else(|| format!("io.poolster.{}", package_segment(&api.name)));
    if package.is_empty() {
        bail!("a Java package name could not be derived from the API name");
    }
    let package_path = package.replace('.', "/");
    let artifact = format!("{}-sdk", package_segment(&api.name));
    let version = package_version(&api.version);
    let mut tree = GeneratedTree::default();

    insert(
        &mut tree,
        &root,
        "settings.gradle",
        settings_gradle(&artifact),
    )?;
    insert(
        &mut tree,
        &root,
        "build.gradle",
        build_gradle(&package, &artifact, &version),
    )?;
    insert(
        &mut tree,
        &root,
        "pom.xml",
        pom_xml(&package, &artifact, &version),
    )?;
    insert(
        &mut tree,
        &root,
        "README.md",
        readme(api, &package, &artifact, style),
    )?;
    insert(
        &mut tree,
        &root,
        "STYLE_GUIDE.md",
        style_guide(api, &package, style),
    )?;
    insert(
        &mut tree,
        &root,
        &format!("src/main/java/{package_path}/ApiException.java"),
        api_exception(&package),
    )?;
    insert(
        &mut tree,
        &root,
        &format!("src/main/java/{package_path}/ClientConfig.java"),
        client_config(&package),
    )?;
    insert(
        &mut tree,
        &root,
        &format!("src/main/java/{package_path}/RetryConfig.java"),
        retry_config(&package),
    )?;
    insert(
        &mut tree,
        &root,
        &format!("src/main/java/{package_path}/ClientHooks.java"),
        client_hooks(&package),
    )?;
    insert(
        &mut tree,
        &root,
        &format!("src/main/java/{package_path}/ClientCallOptions.java"),
        include_str!("../../templates/call_options.java.tmpl").replace("__PACKAGE__", &package),
    )?;
    multipart::emit(api, &root, &package, &mut tree)?;
    for (index, schema) in api.schemas.iter().enumerate() {
        for (filename, source) in render_model_parts(schema, &package, open_enums, index)? {
            insert(
                &mut tree,
                &root,
                &format!("src/main/java/{package_path}/model/{filename}"),
                source,
            )?;
        }
    }
    insert(
        &mut tree,
        &root,
        &format!("src/main/java/{package_path}/ClientBase.java"),
        render_client_base(api, &package),
    )?;
    let operation_overhead = render_operation_chunk(api, &[], &package, 0).len() + 64;
    let units = api
        .operations
        .iter()
        .map(|operation| poolster_core::source_layout::SourceUnit {
            bytes: render_operation_chunk(api, std::slice::from_ref(operation), &package, 0)
                .len()
                .saturating_sub(operation_overhead - 64),
            resource: None,
        })
        .collect::<Vec<_>>();
    let groups = poolster_core::source_layout::SourceLayout::Chunked {
        max_file_bytes: 128 * 1024,
        max_declarations: Some(OPERATIONS_PER_FILE),
    }
    .groups(&units, operation_overhead)?;
    let operation_chunks = groups
        .iter()
        .map(|indices| {
            indices
                .iter()
                .map(|index| api.operations[*index].clone())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for (index, operations) in operation_chunks.iter().enumerate() {
        insert(
            &mut tree,
            &root,
            &format!("src/main/java/{package_path}/internal/Operations{index:03}.java"),
            render_operation_chunk(api, operations, &package, index),
        )?;
    }
    insert(
        &mut tree,
        &root,
        &format!("src/main/java/{package_path}/Client.java"),
        render_client_facade(api, &package, style, operation_chunks.len()),
    )?;
    if style == SdkClientStyle::Namespaced {
        for (resource, operations) in resource_operations(api) {
            let overhead = render_resource_chunk(&resource, &[], &package, 0).len() + 64;
            let units = operations
                .iter()
                .map(|operation| poolster_core::source_layout::SourceUnit {
                    bytes: render_resource_chunk(
                        &resource,
                        std::slice::from_ref(operation),
                        &package,
                        0,
                    )
                    .len()
                    .saturating_sub(overhead - 64),
                    resource: None,
                })
                .collect::<Vec<_>>();
            let groups = poolster_core::source_layout::SourceLayout::Chunked {
                max_file_bytes: 128 * 1024,
                max_declarations: Some(100),
            }
            .groups(&units, overhead)?;
            for (index, indices) in groups.iter().enumerate() {
                let operations = indices
                    .iter()
                    .map(|index| operations[*index].clone())
                    .collect::<Vec<_>>();
                insert(
                    &mut tree,
                    &root,
                    &format!(
                        "src/main/java/{package_path}/internal/resources/{resource}ResourcePart{index:03}.java"
                    ),
                    render_resource_chunk(&resource, &operations, &package, index),
                )?;
            }
            insert(
                &mut tree,
                &root,
                &format!("src/main/java/{package_path}/{resource}Resource.java"),
                render_resource_facade(&resource, &package, groups.len()),
            )?;
        }
    }
    if api.schemas.is_empty() {
        let imports = tree
            .iter()
            .filter(|(path, _)| path.extension().is_some_and(|ext| ext == "java"))
            .map(|(path, source)| {
                (
                    path.to_path_buf(),
                    source.replace(&format!("import {package}.model.*;\n"), ""),
                )
            })
            .collect::<Vec<_>>();
        for (path, source) in imports {
            tree.replace(GeneratedFile::new(path, source)?)?;
        }
    }
    let oversized = tree.iter().filter_map(|(path, source)| {
        let native = path.extension().and_then(|value| value.to_str()) == Some("java");
        (native && source.len() > 128 * 1024).then(|| serde_json::json!({"path":path,"bytes":source.len(),"max_file_bytes":128*1024,"reason":"Atomic native declaration or public facade exceeds the grouping budget; source was retained intact."}))
    }).collect::<Vec<_>>();
    if !oversized.is_empty() {
        insert(
            &mut tree,
            &root,
            ".poolster/source-layout-diagnostics.json",
            serde_json::to_string_pretty(&oversized)?,
        )?;
    }
    Ok(tree)
}

pub(crate) fn insert(
    tree: &mut GeneratedTree,
    root: &str,
    path: &str,
    contents: String,
) -> Result<()> {
    let path = match root {
        "" | "." => path.to_owned(),
        _ => format!("{root}/{path}"),
    };
    tree.insert(GeneratedFile::new(path, contents)?)
}
