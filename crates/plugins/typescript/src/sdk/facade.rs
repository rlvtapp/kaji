use super::*;

pub(crate) fn poolster_sdk_client(
    api: &Api,
    class_name: &str,
    group_by_tag: bool,
    style: SdkClientStyle,
    root: &str,
    runtime_dir: &str,
) -> Result<Vec<GeneratedFile>> {
    match style {
        SdkClientStyle::Flat => Ok(vec![GeneratedFile::new(
            format!("{root}/client.ts"),
            poolster_flat_sdk_client(api, class_name, group_by_tag, runtime_dir),
        )?]),
        SdkClientStyle::Namespaced => {
            poolster_namespaced_sdk_client(api, class_name, group_by_tag, root, runtime_dir)
        }
    }
}

pub(crate) fn poolster_flat_sdk_client(
    api: &Api,
    class_name: &str,
    group_by_tag: bool,
    runtime_dir: &str,
) -> String {
    let mut output = String::from(&format!(
        "import type {{ ClientConfig, ClientInstance }} from './{runtime_dir}/client'\nimport {{ createClient }} from './{runtime_dir}/client'\n"
    ));
    for operation in &api.operations {
        let function = lower_camel_identifier(&operation.id);
        let module = operation_file_identifier(&operation.id);
        let group = if group_by_tag {
            operation_tag_directory(operation)
        } else {
            String::new()
        };
        let path = if group.is_empty() {
            format!("./clients/{module}")
        } else {
            format!("./clients/{group}/{module}")
        };
        output.push_str(&format!("import {{ {function} }} from '{path}'\n"));
    }
    if has_pagination(api) {
        output.push_str(pagination_helpers());
    }
    output.push_str(&format!(
        "\nexport class {class_name} {{\n  /** Configured request transport for direct operations and generated framework hooks. */\n  readonly transport: ClientInstance\n"
    ));
    for operation in &api.operations {
        let function = lower_camel_identifier(&operation.id);
        output.push_str(&format!("  readonly {function}: typeof {function}\n"));
        if pagination(operation).is_some() {
            output.push_str(&format!(
                "  readonly {function}Pages: (options: Parameters<typeof {function}>[0]) => AsyncIterable<Awaited<ReturnType<typeof {function}>>>\n"
            ));
        }
    }
    output.push_str(
        "\n  constructor(config: ClientConfig = {}) {\n    this.transport = createClient(config)\n",
    );
    for operation in &api.operations {
        let function = lower_camel_identifier(&operation.id);
        output.push_str(&format!(
            "    this.{function} = ((options: Parameters<typeof {function}>[0]) => {function}({{ ...options, client: this.transport }})) as typeof {function}\n"
        ));
        if let Some(pagination) = pagination(operation) {
            output.push_str(&render_pagination_iterator(
                &function,
                &function,
                &pagination,
            ));
        }
    }
    output.push_str("  }\n}\n");
    output
}

pub(crate) fn poolster_namespaced_sdk_client(
    api: &Api,
    class_name: &str,
    group_by_tag: bool,
    root: &str,
    runtime_dir: &str,
) -> Result<Vec<GeneratedFile>> {
    let mut groups: BTreeMap<String, Vec<(&Operation, String)>> = BTreeMap::new();
    for operation in &api.operations {
        let namespace = sdk_namespace(operation);
        let method = sdk_method_name(operation, &namespace);
        groups
            .entry(namespace)
            .or_default()
            .push((operation, method));
    }

    let mut client_types = Vec::new();
    let mut files = Vec::new();
    for (namespace, operations) in &groups {
        let namespace_type = format!("{}Client", pascal_identifier(namespace));
        client_types.push((namespace.clone(), namespace_type.clone()));
        let resolved_methods = resolved_resource_methods(operations);
        let mut chunk_types = Vec::new();
        for (index, chunk) in resolved_methods.chunks(BARREL_EXPORTS_PER_FILE).enumerate() {
            let chunk_type = format!("{namespace_type}Operations{:04}", index + 1);
            let chunk_file = format!("operations_{:04}", index + 1);
            chunk_types.push((chunk_type.clone(), chunk_file.clone()));
            files.push(GeneratedFile::new(
                format!("{root}/resources/{namespace}/{chunk_file}.ts"),
                render_namespaced_resource_chunk(chunk, &chunk_type, group_by_tag, runtime_dir),
            )?);
        }
        let mut output = String::from(&format!(
            "import type {{ ClientInstance }} from '../{runtime_dir}/client'\n"
        ));
        for (chunk_type, chunk_file) in &chunk_types {
            output.push_str(&format!(
                "import {{ {chunk_type} }} from './{namespace}/{chunk_file}'\n"
            ));
        }
        output.push_str(&format!("\nexport interface {namespace_type} extends "));
        output.push_str(
            &chunk_types
                .iter()
                .map(|(chunk_type, _)| chunk_type.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );
        output.push_str(" {}\n\n");
        output.push_str(&format!("export class {namespace_type} {{\n"));
        output.push_str("  constructor(client: ClientInstance) {\n    Object.assign(this, ");
        output.push_str(
            &chunk_types
                .iter()
                .map(|(chunk_type, _)| format!("new {chunk_type}(client)"))
                .collect::<Vec<_>>()
                .join(", "),
        );
        output.push_str(")\n  }\n}\n");
        files.push(GeneratedFile::new(
            format!("{root}/resources/{namespace}.ts"),
            output,
        )?);
    }

    let mut output = String::from(&format!(
        "import type {{ ClientConfig, ClientInstance }} from './{runtime_dir}/client'\nimport {{ createClient }} from './{runtime_dir}/client'\n"
    ));
    for (namespace, namespace_type) in &client_types {
        output.push_str(&format!(
            "import {{ {namespace_type} }} from './resources/{namespace}'\n"
        ));
    }
    output.push_str(&format!(
        "\nexport class {class_name} {{\n  /** Configured request transport for direct operations and generated framework hooks. */\n  readonly transport: ClientInstance\n"
    ));
    for (namespace, namespace_type) in &client_types {
        output.push_str(&format!("  readonly {namespace}: {namespace_type}\n"));
    }
    output.push_str(
        "\n  constructor(config: ClientConfig = {}) {\n    this.transport = createClient(config)\n",
    );
    for (namespace, namespace_type) in &client_types {
        output.push_str(&format!(
            "    this.{namespace} = new {namespace_type}(this.transport)\n"
        ));
    }
    output.push_str("  }\n}\n");
    files.push(GeneratedFile::new(format!("{root}/client.ts"), output)?);
    Ok(files)
}

pub(crate) fn resolved_resource_methods<'a>(
    operations: &'a [(&'a Operation, String)],
) -> Vec<(&'a Operation, String)> {
    let mut names = std::collections::BTreeSet::new();
    operations
        .iter()
        .map(|(operation, proposed_name)| {
            let function = lower_camel_identifier(&operation.id);
            let name = if names.insert(proposed_name.clone()) {
                proposed_name.clone()
            } else {
                function
            };
            (*operation, name)
        })
        .collect()
}

pub(crate) fn render_namespaced_resource_chunk(
    operations: &[(&Operation, String)],
    class_name: &str,
    group_by_tag: bool,
    runtime_dir: &str,
) -> String {
    let mut output = String::from(&format!(
        "import type {{ ClientInstance }} from '../../{runtime_dir}/client'\n"
    ));
    for (operation, _) in operations {
        let function = lower_camel_identifier(&operation.id);
        let module = operation_file_identifier(&operation.id);
        let group = if group_by_tag {
            operation_tag_directory(operation)
        } else {
            String::new()
        };
        let path = if group.is_empty() {
            format!("../../clients/{module}")
        } else {
            format!("../../clients/{group}/{module}")
        };
        output.push_str(&format!("import {{ {function} }} from '{path}'\n"));
    }
    if operations
        .iter()
        .any(|(operation, _)| pagination(operation).is_some())
    {
        output.push_str(pagination_helpers());
    }
    output.push_str(&format!("\nexport class {class_name} {{\n"));
    for (operation, name) in operations {
        let function = lower_camel_identifier(&operation.id);
        output.push_str(&format!("  readonly {name}: typeof {function}\n"));
        if pagination(operation).is_some() {
            output.push_str(&format!(
                "  readonly {name}Pages: (options: Parameters<typeof {function}>[0]) => AsyncIterable<Awaited<ReturnType<typeof {function}>>>\n"
            ));
        }
    }
    output.push_str("\n  constructor(client: ClientInstance) {\n");
    for (operation, name) in operations {
        let function = lower_camel_identifier(&operation.id);
        output.push_str(&format!(
            "    this.{name} = ((options: Parameters<typeof {function}>[0]) => {function}({{ ...options, client }})) as typeof {function}\n"
        ));
        if let Some(pagination) = pagination(operation) {
            output.push_str(&render_pagination_iterator_with_client(
                name,
                &function,
                &pagination,
                "client",
            ));
        }
    }
    output.push_str("  }\n}\n");
    output
}

pub(crate) fn sdk_namespace(operation: &Operation) -> String {
    operation_tag_directory_if_present(operation).unwrap_or_else(|| {
        operation
            .path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .find(|segment| {
                !segment.starts_with('{')
                    && !segment.starts_with('v')
                    && *segment != "email"
                    && *segment != "api"
            })
            .map(lower_camel_identifier)
            .filter(|segment| !segment.is_empty())
            .unwrap_or_else(|| "api".into())
    })
}

pub(crate) fn sdk_method_name(operation: &Operation, namespace: &str) -> String {
    let function = lower_camel_identifier(&operation.id);
    let singular = namespace.strip_suffix('s').unwrap_or(namespace);
    let candidates = [pascal_identifier(namespace), pascal_identifier(singular)];
    for candidate in candidates {
        if let Some(method) = function.strip_suffix(&candidate) {
            if !method.is_empty() {
                return method.into();
            }
        }
    }
    let prefix = lower_camel_identifier(namespace);
    if let Some(method) = function.strip_prefix(&prefix) {
        if !method.is_empty() {
            return lower_camel_identifier(method);
        }
    }
    function
}
