//! Assembly emission for the csharp HTTP SDK.
use crate::*;

/// Generates an idiomatic .NET 8 C# SDK below `output_dir`.
///
/// The returned tree is intentionally not written to disk: a profile can
/// combine this output with other language targets before atomically
/// materializing it. `package_name` controls the NuGet package identity; when
/// omitted, a stable `<api>-sdk` name is derived from the API title.
#[cfg(test)]
pub(crate) fn render_test_sdk(
    api: &Api,
    output_dir: &str,
    package_name: Option<&str>,
) -> Result<GeneratedTree> {
    render_sdk(api, output_dir, package_name, SdkClientStyle::Flat)
}

/// Generates a .NET SDK with either a flat or resource-namespaced client.
///
/// [`SdkClientStyle::Flat`] exports the direct `PoolsterClient` API such as
/// `client.CreateContactAsync(...)`. [`SdkClientStyle::Namespaced`] additionally
/// exports resource properties, for example `client.Contacts.CreateAsync(...)`.
/// The direct methods remain available in both modes, so choosing the facade is
/// a non-breaking additive change for generated consumers.
#[cfg(test)]
pub(crate) fn render_sdk(
    api: &Api,
    output_dir: &str,
    package_name: Option<&str>,
    client_style: SdkClientStyle,
) -> Result<GeneratedTree> {
    render_sdk_with_policy(api, output_dir, package_name, client_style, false)
}

pub(crate) fn render_sdk_with_policy(
    api: &Api,
    output_dir: &str,
    package_name: Option<&str>,
    client_style: SdkClientStyle,
    open_enums: bool,
) -> Result<GeneratedTree> {
    let prepared = prepare_api(api);
    let api = &prepared;
    multipart::validate(api)?;

    for operation in &api.operations {
        let extension = poolster_core::poolster_extension(&operation.annotations, "pagination")
            .or_else(|| operation.annotations.get("x-speakeasy-pagination"));
        if matches!(
            extension
                .and_then(|extension| extension.get("type"))
                .and_then(serde_json::Value::as_str),
            Some("page" | "offsetLimit" | "url")
        ) {
            pagination::validate(api, operation)?;
        }
    }
    let root = normalized_root(output_dir)?;
    let package = package_name
        .filter(|name| !name.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{}-sdk", kebab_case(&api.name)));
    let namespace = dotnet_namespace(&package);
    let project = format!("{}.csproj", pascal_case(&package));
    let mut tree = GeneratedTree::default();
    tree.insert(GeneratedFile::new(
        output_path(&root, &project),
        render_project(api, &package),
    )?)?;
    multipart::emit(api, &root, &namespace, &mut tree)?;
    for (index, schema) in api.schemas.iter().enumerate() {
        for (filename, source) in render_model_parts(schema, &namespace, open_enums, index)? {
            tree.insert(GeneratedFile::new(
                output_path(&root, &format!("Models/{filename}")),
                source,
            )?)?;
        }
    }
    tree.insert(GeneratedFile::new(
        output_path(&root, "ApiException.cs"),
        render_api_exception(&Api::default(), &namespace),
    )?)?;
    let error_header = format!("{NOTICE}\nusing System.Text.Json;\nnamespace {namespace};\n");
    let errors = api
        .operations
        .iter()
        .map(|operation| {
            let mut only = Api::default();
            only.operations.push(operation.clone());
            render_api_exception(&only, &namespace)
                .strip_prefix(&render_api_exception(&Api::default(), &namespace))
                .unwrap()
                .to_owned()
        })
        .filter(|source| !source.is_empty())
        .collect::<Vec<_>>();
    let units = errors
        .iter()
        .map(|error| poolster_core::source_layout::SourceUnit {
            bytes: error.len(),
            resource: None,
        })
        .collect::<Vec<_>>();
    for (part, indices) in poolster_core::source_layout::SourceLayout::default()
        .groups(&units, error_header.len())?
        .iter()
        .enumerate()
    {
        let source = error_header.clone()
            + &indices
                .iter()
                .map(|index| errors[*index].as_str())
                .collect::<String>();
        tree.insert(GeneratedFile::new(
            output_path(&root, &format!("Errors/DeclaredErrors{part:03}.cs")),
            source,
        )?)?;
    }
    tree.insert(GeneratedFile::new(
        output_path(&root, "PoolsterClient.cs"),
        render_client(api, &namespace, client_style),
    )?)?;
    let overhead = render_operation_chunk(api, &[], &namespace).len();
    let units = api
        .operations
        .iter()
        .map(|operation| poolster_core::source_layout::SourceUnit {
            bytes: render_operation_chunk(api, std::slice::from_ref(operation), &namespace)
                .len()
                .saturating_sub(overhead),
            resource: None,
        })
        .collect::<Vec<_>>();
    let groups = poolster_core::source_layout::SourceLayout::Chunked {
        max_file_bytes: 128 * 1024,
        max_declarations: Some(100),
    }
    .groups(&units, overhead)?;
    for (part, indices) in groups.iter().enumerate() {
        let operations = indices
            .iter()
            .map(|index| api.operations[*index].clone())
            .collect::<Vec<_>>();
        tree.insert(GeneratedFile::new(
            output_path(
                &root,
                &format!("Operations/PoolsterClientOperations{part:03}.cs"),
            ),
            render_operation_chunk(api, &operations, &namespace),
        )?)?;
    }
    if client_style == SdkClientStyle::Namespaced {
        for resource in operation_groups(api) {
            let operations = api
                .operations
                .iter()
                .filter(|operation| operation_resource_name(operation) == resource)
                .collect::<Vec<_>>();
            let overhead = render_resource_chunk(&resource, &[], &namespace, true).len();
            let units = operations
                .iter()
                .map(|operation| poolster_core::source_layout::SourceUnit {
                    bytes: render_resource_chunk(
                        &resource,
                        std::slice::from_ref(operation),
                        &namespace,
                        false,
                    )
                    .len()
                    .saturating_sub(render_resource_chunk(&resource, &[], &namespace, false).len()),
                    resource: None,
                })
                .collect::<Vec<_>>();
            let groups = poolster_core::source_layout::SourceLayout::Chunked {
                max_file_bytes: 128 * 1024,
                max_declarations: Some(100),
            }
            .groups(&units, overhead)?;
            for (part, indices) in groups.iter().enumerate() {
                let operations = indices
                    .iter()
                    .map(|index| operations[*index])
                    .collect::<Vec<_>>();
                tree.insert(GeneratedFile::new(
                    output_path(&root, &format!("Resources/{resource}Resource{part:03}.cs")),
                    render_resource_chunk(&resource, &operations, &namespace, part == 0),
                )?)?;
            }
        }
    }
    tree.insert(GeneratedFile::new(
        output_path(&root, "README.md"),
        render_readme(api, &package, &namespace, client_style),
    )?)?;
    tree.insert(GeneratedFile::new(
        output_path(&root, "STYLE_GUIDE.md"),
        render_style_guide(api, &namespace, client_style),
    )?)?;
    let oversized = tree.iter().filter_map(|(path, source)| {
        let native = path.extension().and_then(|value| value.to_str()) == Some("cs");
        (native && source.len() > 128 * 1024).then(|| serde_json::json!({"path":path,"bytes":source.len(),"max_file_bytes":128*1024,"reason":"Atomic native declaration or public facade exceeds the grouping budget; source was retained intact."}))
    }).collect::<Vec<_>>();
    if !oversized.is_empty() {
        tree.insert(GeneratedFile::new(
            output_path(&root, ".poolster/source-layout-diagnostics.json"),
            serde_json::to_string_pretty(&oversized)?,
        )?)?;
    }
    Ok(tree)
}

pub(crate) fn normalized_root(output_dir: &str) -> Result<String> {
    let root = output_dir.trim_matches('/');
    if root.is_empty() {
        bail!(".NET SDK output directory cannot be empty");
    }
    if root.split('/').any(|component| component == "..") {
        bail!(".NET SDK output directory cannot contain parent-directory components");
    }
    Ok(root.to_owned())
}

pub(crate) fn output_path(root: &str, file: &str) -> String {
    format!("{root}/{file}")
}

pub(crate) fn render_project(api: &Api, package: &str) -> String {
    format!(
        "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    <TargetFramework>net8.0</TargetFramework>\n    <ImplicitUsings>enable</ImplicitUsings>\n    <Nullable>enable</Nullable>\n    <LangVersion>latest</LangVersion>\n    <GeneratePackageOnBuild>false</GeneratePackageOnBuild>\n    <PackageId>{}</PackageId>\n    <Version>{}</Version>\n    <Description>Generated .NET client for {}</Description>\n  </PropertyGroup>\n</Project>\n",
        xml_escape(package),
        dotnet_version(&api.version),
        xml_escape(&api.name),
    )
}
