use super::*;

/// Generates an installable, typed Python SDK below `output_dir`.
///
/// The package uses only the Python standard library at runtime. `package_name`
/// controls both the distribution name in `pyproject.toml` and the import
/// package (with hyphens normalized to underscores). When omitted it is
/// derived from the API title.
#[cfg(test)]
pub(crate) fn render_test_sdk(
    api: &Api,
    output_dir: &str,
    package_name: Option<&str>,
) -> Result<GeneratedTree> {
    render_sdk(api, output_dir, package_name, SdkClientStyle::Flat)
}

/// Generates a Python SDK with either direct operations or resource namespaces.
pub(crate) fn render_sdk(
    api: &Api,
    output_dir: &str,
    package_name: Option<&str>,
    client_style: SdkClientStyle,
) -> Result<GeneratedTree> {
    let prepared = symbols::prepare(api);
    let api = prepared.as_ref();
    for operation in &api.operations {
        if pagination_annotation(operation)
            .and_then(|v| v.get("type"))
            .and_then(Value::as_str)
            == Some("page")
        {
            poolster_core::pagination::normalize_pagination(api, operation, None)?;
        }
    }
    let root = output_dir.trim_matches('/');
    if root.is_empty() {
        bail!("Python SDK output directory cannot be empty");
    }

    let distribution = package_name
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{}-sdk", kebab_case(&api.name)));
    let module = python_module_name(&distribution);
    let mut tree = GeneratedTree::default();
    tree.insert(GeneratedFile::new(
        format!("{root}/pyproject.toml"),
        render_pyproject(&distribution, &api.version),
    )?)?;
    // Keep source files comfortably navigable for very large descriptions.
    // `models` remains a normal import package, so `from sdk.models import X`
    // stays exactly the same for callers.
    tree.insert(GeneratedFile::new(
        format!("{root}/src/{module}/models/__init__.py"),
        render_models_init(api),
    )?)?;
    tree.insert(GeneratedFile::new(
        format!("{root}/src/{module}/models/chunks/__init__.py"),
        format!("{NOTICE}# Bounded model re-export partitions.\n"),
    )?)?;
    for (index, schemas) in api.schemas.chunks(100).enumerate() {
        tree.insert(GeneratedFile::new(
            format!("{root}/src/{module}/models/chunks/exports_{index:03}.py"),
            render_model_exports(schemas),
        )?)?;
    }
    tree.insert(GeneratedFile::new(
        format!("{root}/src/{module}/models/_model_codec.py"),
        include_str!("../../templates/model_codec.py"),
    )?)?;
    for schema in &api.schemas {
        tree.insert(GeneratedFile::new(
            format!(
                "{root}/src/{module}/models/{}.py",
                schema_file_name(&schema.name)
            ),
            render_model(schema),
        )?)?;
    }
    tree.insert(GeneratedFile::new(
        format!("{root}/src/{module}/runtime.py"),
        render_runtime(api),
    )?)?;
    tree.insert(GeneratedFile::new(
        format!("{root}/src/{module}/presence.py"),
        include_str!("../../templates/presence.py"),
    )?)?;
    for (path, contents) in render_partitioned_errors(api) {
        tree.insert(GeneratedFile::new(
            format!("{root}/src/{module}/{path}"),
            contents,
        )?)?;
    }
    for (path, contents) in response_validation::render_partitioned(api) {
        tree.insert(GeneratedFile::new(
            format!("{root}/src/{module}/{path}"),
            contents,
        )?)?;
    }
    tree.insert(GeneratedFile::new(
        format!("{root}/src/{module}/multipart.py"),
        include_str!("../../templates/multipart.py"),
    )?)?;
    if api
        .operations
        .iter()
        .any(|operation| operation_body_kind(operation).ends_with("multipart"))
    {
        tree.insert(GeneratedFile::new(
            format!("{root}/MULTIPART.md"),
            include_str!("../../templates/multipart_readme.md"),
        )?)?;
    }
    tree.insert(GeneratedFile::new(
        format!("{root}/src/{module}/oauth.py"),
        include_str!("../../templates/oauth.py"),
    )?)?;
    for (index, range) in python_operation_groups(api).into_iter().enumerate() {
        let operations = &api.operations[range];
        tree.insert(GeneratedFile::new(
            format!("{root}/src/{module}/operations_{index:03}.py"),
            render_operation_chunk(api, operations, index),
        )?)?;
    }
    if client_style == SdkClientStyle::Namespaced {
        let resource_names = resource_operations(api).keys().cloned().collect::<Vec<_>>();
        tree.insert(GeneratedFile::new(
            format!("{root}/src/{module}/resources/__init__.py"),
            render_resources_init(resource_names.len()),
        )?)?;
        tree.insert(GeneratedFile::new(
            format!("{root}/src/{module}/resources/chunks/__init__.py"),
            format!("{NOTICE}# Bounded resource re-export partitions.\n"),
        )?)?;
        for (index, resources) in resource_names.chunks(100).enumerate() {
            tree.insert(GeneratedFile::new(
                format!("{root}/src/{module}/resources/chunks/exports_{index:03}.py"),
                render_resource_exports(resources),
            )?)?;
        }
        for (resource, resource_operations) in resource_operations(api) {
            let file = schema_file_name(&resource);
            let groups = python_resource_groups(api, &resource, &resource_operations);
            let group_count = groups.len();
            for (index, range) in groups.into_iter().enumerate() {
                let operations = &resource_operations[range];
                tree.insert(GeneratedFile::new(
                    format!("{root}/src/{module}/resources/{file}_part_{index:03}.py"),
                    render_resource_chunk(api, &resource, &resource_operations, operations, index),
                )?)?;
            }
            tree.insert(GeneratedFile::new(
                format!("{root}/src/{module}/resources/{file}.py"),
                render_resource_facade(&resource, group_count),
            )?)?;
        }
    }
    tree.insert(GeneratedFile::new(
        format!("{root}/src/{module}/client.py"),
        render_client_facade(api, client_style),
    )?)?;
    tree.insert(GeneratedFile::new(
        format!("{root}/src/{module}/__init__.py"),
        render_init(api, client_style),
    )?)?;
    tree.insert(GeneratedFile::new(
        format!("{root}/README.md"),
        render_readme(api, &distribution, &module, client_style),
    )?)?;
    tree.insert(GeneratedFile::new(
        format!("{root}/api.md"),
        render_api_reference(api),
    )?)?;
    if client_style == SdkClientStyle::Namespaced {
        tree.insert(GeneratedFile::new(
            format!("{root}/STYLE_GUIDE.md"),
            render_style_guide(api, &module),
        )?)?;
    }
    Ok(tree)
}
