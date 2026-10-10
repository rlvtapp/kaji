use super::*;

pub(crate) fn generate_typescript_sdk(
    api: &Api,
    profile: &SdkConfig,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> Result<GeneratedTree> {
    // Poolster's tag-directory layout otherwise puts every untagged operation in
    // `default`. Poolster's SDK surface uses the first meaningful path segment as
    // a stable resource namespace, matching the native targets.
    let mut sdk_api = crate::symbols::prepare(api);
    if profile.group_by_tag {
        for operation in &mut sdk_api.operations {
            if operation_tag_directory_if_present(operation).is_none() {
                let namespace = sdk_namespace(operation);
                operation
                    .annotations
                    .insert("tags".into(), serde_json::json!([namespace]));
            }
        }
    }
    let root = profile.output_dir.trim_matches('/');
    let models_dir = format!("{root}/models");
    let clients_dir = format!("{root}/clients");
    let type_options = ModelRenderOptions {
        output_dir: models_dir,
        schema_output_dir: None,
        operation_output_dir: None,
        group_by_tag: profile.group_by_tag,
        model: profile.model_options.clone(),
    };
    let client_options = ClientRenderOptions {
        model_options: Some(profile.model_options.clone()),
        output_dir: clients_dir,
        runtime_dir: ".poolster".into(),
        throw_on_error: profile.throw_on_error,
        group_by_tag: profile.group_by_tag,
        group_default_directory: profile.group_by_tag,
        type_import_prefix: Some(
            if profile.group_by_tag {
                "../../models"
            } else {
                "../models"
            }
            .into(),
        ),
        runtime_import_prefix: Some("..".into()),
    };
    let mut tree = GeneratedTree::default();
    for file in ModelRenderer.generate(&sdk_api, &type_options)? {
        tree.insert(file)?;
    }
    let client_files = generate_operations(&sdk_api, &client_options, security_schemes)?;
    for file in client_files {
        tree.insert(file)?;
    }
    tree.insert(GeneratedFile::new(
        format!("{root}/.poolster/client.ts"),
        poolster_runtime(profile.transport, security_schemes),
    )?)?;
    let client_name = (profile.surface == SdkSurface::Client).then(|| {
        profile
            .client_name
            .clone()
            .unwrap_or_else(|| sdk_client_name(&sdk_api.name))
    });
    if let Some(client_name) = &client_name {
        for file in poolster_sdk_client(
            &sdk_api,
            client_name,
            profile.group_by_tag,
            profile.client_style,
            root,
            ".poolster",
        )? {
            tree.insert(file)?;
        }
    }
    tree.insert_custom(GeneratedFile::new(
        format!("{root}/custom/index.ts"),
        "// This module is created once and never overwritten by Poolster.\n// Add stable helpers, exports, or product-specific wrappers here.\nexport {}\n",
    )?)?;
    for file in poolster_barrels(
        &tree,
        root,
        &sdk_api,
        profile.group_by_tag,
        client_name.as_deref(),
        !matches!(profile.model_options.enum_type, crate::EnumType::Literal),
    )? {
        tree.insert(GeneratedFile::new(format!("{root}/{}", file.0), file.1)?)?;
    }
    tree.insert(GeneratedFile::new(
        format!("{root}/package.json"),
        poolster_package(&sdk_api, profile.transport, profile.package_name.as_deref())?,
    )?)?;
    tree.insert(GeneratedFile::new(
        format!("{root}/README.md"),
        poolster_readme(&sdk_api, profile),
    )?)?;
    tree.insert(GeneratedFile::new(
        format!("{root}/tsconfig.json"),
        "{\n  \"compilerOptions\": { \"declaration\": true, \"module\": \"ESNext\", \"moduleResolution\": \"Bundler\", \"outDir\": \"dist\", \"strict\": true, \"skipLibCheck\": true, \"target\": \"ES2022\" },\n  \"include\": [\"**/*.ts\"]\n}\n",
    )?)?;
    Ok(tree)
}
