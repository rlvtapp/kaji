//! HTTP operation layout rendering.
use super::*;

pub(crate) fn render_operation_files(
    api: &Api,
    config: &RenderOptions,
) -> Result<Vec<GeneratedFile>> {
    let mut files = Vec::new();
    let mut diagnostics = Vec::new();
    for operation in &api.operations {
        let Some(extension) =
            poolster_core::poolster_extension(&operation.annotations, "pagination")
                .or_else(|| operation.annotations.get("x-speakeasy-pagination"))
        else {
            continue;
        };
        if extension.get("type").and_then(Value::as_str) == Some("page") {
            poolster_core::pagination::normalize_pagination(api, operation, None)?;
        }
        if rust_pagination(operation).is_none() {
            diagnostics.push(serde_json::json!({"operationId":operation.id,"type":extension.get("type"),"reason":"Native Rust helper does not support this declaration; page controls require scalar integer parameters, cursor controls require scalar string parameters, and responses must be buffered JSON."}));
        }
    }
    if !diagnostics.is_empty() {
        files.push(GeneratedFile::new(
            ".poolster/pagination-diagnostics.json",
            serde_json::to_string_pretty(&diagnostics)?,
        )?);
    }
    let mut module_index = String::new();
    let operation_units = api
        .operations
        .iter()
        .map(|operation| {
            let mut bytes =
                render_operation_error(operation).len() + render_operation(operation, config).len();
            if operation_has_parameters(operation) {
                bytes += render_operation_request(operation).len();
            }
            if let Some(form) = multipart_alternative(operation) {
                bytes += render_operation(&form, config).len() + 128;
            }
            poolster_core::source_layout::SourceUnit {
                bytes,
                resource: None,
            }
        })
        .collect::<Vec<_>>();
    let overhead = NOTICE.len() + 256;
    let groups = poolster_core::source_layout::SourceLayout::Chunked {
        max_file_bytes: 128 * 1024,
        max_declarations: Some(OPERATIONS_PER_FILE),
    }
    .groups(&operation_units, overhead)?;
    for (index, indices) in groups.iter().enumerate() {
        let operations = indices
            .iter()
            .map(|index| &api.operations[*index])
            .collect::<Vec<_>>();
        let module = format!("chunk_{:04}", index + 1);
        let _ = writeln!(module_index, "mod {module};");
        let _ = writeln!(module_index, "pub use {module}::*;");
        let mut contents = format!(
            "{NOTICE}\n\nuse super::super::*;\n#[allow(unused_imports)]\nuse crate::models::*;\n\n"
        );
        for operation in &operations {
            contents.push_str(&render_operation_error(operation));
            if operation_has_parameters(operation) {
                contents.push_str(&render_operation_request(operation));
            }
        }
        contents.push_str("impl Client {\n");
        for operation in &operations {
            contents.push_str(&render_operation(operation, config));
            if let Some(form) = multipart_alternative(operation) {
                let direct = direct_method_name(operation, config);
                let name = multipart_method_name(api, operation, config);
                contents.push_str(&render_operation(&form, config).replacen(
                    &format!("pub async fn {direct}("),
                    &format!("pub async fn {name}("),
                    1,
                ));
            }
        }
        contents.push_str("}\n");
        files.push(GeneratedFile::new(
            format!("src/client/operations/{module}.rs"),
            contents,
        )?);
    }
    files.insert(
        0,
        GeneratedFile::new(
            "src/client/operations/mod.rs",
            format!("{NOTICE}\n{module_index}"),
        )?,
    );
    Ok(files)
}

pub(crate) fn render_resource_files(
    api: &Api,
    options: &RenderOptions,
) -> Result<Vec<GeneratedFile>> {
    let resources = resource_operations(api);
    let direct_methods = api
        .operations
        .iter()
        .map(|operation| direct_method_name(operation, options))
        .collect::<Vec<_>>();
    let mut files = Vec::new();
    let mut module_index = String::new();
    for (resource_index, (resource, operations)) in resources.iter().enumerate() {
        let resource_module = format!("{}_{}", bounded_module_stem(resource), resource_index + 1);
        let mut resource_index_source = String::new();
        let accessor = resource_accessor_name(resource, &direct_methods);
        let client_type = format!("{}Client", type_name(resource));
        let mut used_methods = BTreeMap::<String, usize>::new();
        let methods = operations
            .iter()
            .map(|operation| {
                let direct = rust_field_name(&operation.id);
                let preferred = resource_method_name(&direct, resource);
                let seen = used_methods.entry(preferred.clone()).or_default();
                let method = if *seen == 0 { preferred } else { direct };
                *seen += 1;
                (*operation, method)
            })
            .collect::<Vec<_>>();
        let declarations = methods
            .iter()
            .map(|(operation, method)| {
                let mut contents = render_resource_operation(method, operation, options);
                if let Some(form) = multipart_alternative(operation) {
                    let mut facade = format!("{method}_multipart");
                    while methods.iter().any(|(_, existing)| existing == &facade) {
                        facade.push_str("_body");
                    }
                    let direct = direct_method_name(operation, options);
                    let companion = multipart_method_name(api, operation, options);
                    contents.push_str(&render_resource_operation(&facade, &form, options).replace(
                        &format!("self.client.{direct}("),
                        &format!("self.client.{companion}("),
                    ));
                }
                if rust_pagination(operation).is_some() {
                    contents.push_str(&render_resource_pagination_operation(
                        method, operation, options,
                    ));
                }
                contents
            })
            .collect::<Vec<_>>();
        let units = declarations
            .iter()
            .map(|source| poolster_core::source_layout::SourceUnit {
                bytes: source.len(),
                resource: None,
            })
            .collect::<Vec<_>>();
        let groups = poolster_core::source_layout::SourceLayout::Chunked {
            max_file_bytes: 128 * 1024,
            max_declarations: Some(RESOURCE_METHODS_PER_FILE),
        }
        .groups(&units, 2048 + resource.len() + client_type.len() * 4)?;
        for (chunk_index, indices) in groups.iter().enumerate() {
            let module = format!("chunk_{:04}", chunk_index + 1);
            let _ = writeln!(resource_index_source, "mod {module};");
            let _ = writeln!(resource_index_source, "pub use {module}::*;");
            let mut contents = format!(
                "{NOTICE}\n\nuse super::super::super::*;\n#[allow(unused_imports)]\nuse crate::models::*;\n\n"
            );
            if chunk_index == 0 {
                let _ = writeln!(
                    contents,
                    "impl Client {{\n    /// Returns the {resource} resource client.\n    pub fn {accessor}(&self) -> {client_type}<'_> {{\n        {client_type} {{ client: self }}\n    }}\n}}\n"
                );
                let _ = writeln!(
                    contents,
                    "/// Resource-first operations for {resource}.\npub struct {client_type}<'a> {{\n    pub(super) client: &'a Client,\n}}\n"
                );
            }
            let _ = writeln!(contents, "impl {client_type}<'_> {{");
            for index in indices {
                contents.push_str(&declarations[*index]);
            }
            contents.push_str("}\n");
            files.push(GeneratedFile::new(
                format!("src/client/resources/{resource_module}/{module}.rs"),
                contents,
            )?);
        }
        files.push(GeneratedFile::new(
            format!("src/client/resources/{resource_module}/mod.rs"),
            format!("{NOTICE}\n{resource_index_source}"),
        )?);
        let _ = writeln!(module_index, "mod {resource_module};");
        let _ = writeln!(module_index, "pub use {resource_module}::*;");
    }
    files.insert(
        0,
        GeneratedFile::new(
            "src/client/resources/mod.rs",
            format!("{NOTICE}\n{module_index}"),
        )?,
    );
    Ok(files)
}
