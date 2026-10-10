use super::*;

pub(crate) fn render_sdk(
    api: &Api,
    output_dir: &str,
    package_name: Option<&str>,
    client_style: SdkClientStyle,
) -> Result<GeneratedTree> {
    let prepared = symbols::prepare(api);
    let api = prepared.as_ref();
    for operation in &api.operations {
        poolster_core::openapi32::request_content(operation)?;
        poolster_core::openapi32::response_content(operation)?;
        for parameter in &operation.parameters {
            poolster_core::openapi32::parameter_content(parameter)?;
        }
    }
    let root = output_dir.trim_matches('/');
    if root.is_empty() {
        bail!("Ruby SDK output directory cannot be empty");
    }
    let gem = package_name
        .filter(|name| !name.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{}-sdk", kebab_case(&api.name)));
    let module = pascal_case(&gem);
    let file_module = ruby_file_name(&gem);
    let mut tree = GeneratedTree::default();
    let diagnostics=api.operations.iter().filter_map(|operation|pagination::plan(api,operation).err().map(|error|serde_json::json!({"operation":operation.id,"status":"unsupported","reason":error.to_string()}))).collect::<Vec<_>>();
    if api.operations.iter().any(|operation| {
        operation.annotations.contains_key("x-poolster-pagination")
            || operation.annotations.contains_key("x-speakeasy-pagination")
    }) {
        insert(
            &mut tree,
            root,
            ".poolster/pagination-diagnostics.json",
            serde_json::to_string_pretty(&serde_json::json!({"diagnostics":diagnostics}))?,
        )?;
    }

    insert(
        &mut tree,
        root,
        &format!("{gem}.gemspec"),
        gemspec(api, &gem),
    )?;
    insert(
        &mut tree,
        root,
        "Gemfile",
        "source \"https://rubygems.org\"\n\ngemspec\n".into(),
    )?;
    insert(
        &mut tree,
        root,
        "README.md",
        readme(api, &gem, &module, client_style),
    )?;
    insert(
        &mut tree,
        root,
        "STYLE_GUIDE.md",
        style_guide(api, &module, client_style),
    )?;
    insert(
        &mut tree,
        root,
        &format!("lib/{file_module}.rb"),
        format!(
            "{NOTICE}require_relative \"{file_module}/models\"\nrequire_relative \"{file_module}/client\"\n\nmodule {module}\nend\n"
        ),
    )?;
    let model_contents =
        if api.schemas.len() <= 100 && render_models(api, &module).len() <= 128 * 1024 {
            render_models(api, &module)
        } else {
            let mut empty = api.clone();
            empty.schemas.clear();
            let mut loader = render_models(&empty, &module);
            let models = api
                .schemas
                .iter()
                .map(|schema| render_model(api, schema))
                .collect::<Vec<_>>();
            for (index, chunk) in bounded_source_chunks(&models).iter().enumerate() {
                let contents = format!(
                    "{NOTICE}module {module}\n  module Models\n{}  end\nend\n",
                    chunk.concat()
                );
                insert(
                    &mut tree,
                    root,
                    &format!("lib/{file_module}/models/chunk_{index:04}.rb"),
                    contents,
                )?;
                let _ = writeln!(loader, "require_relative \"models/chunk_{index:04}\"");
            }
            loader
        };
    insert(
        &mut tree,
        root,
        &format!("lib/{file_module}/models.rb"),
        model_contents,
    )?;
    let client_contents = if api.operations.len() <= 100
        && render_client(api, &module, client_style).len() <= 128 * 1024
    {
        render_client(api, &module, client_style)
    } else {
        let mut loader = render_client_impl(api, &module, client_style, false);
        let operations = api
            .operations
            .iter()
            .map(|operation| {
                let mut content = render_operation(api, operation);
                if let Some(page) = pagination::render(api, operation) {
                    content.push_str(&page);
                }
                content
            })
            .collect::<Vec<_>>();
        for (index, chunk) in bounded_source_chunks(&operations).iter().enumerate() {
            let contents = format!(
                "{NOTICE}module {module}\n  class Client\n{}  end\nend\n",
                chunk.concat()
            );
            insert(
                &mut tree,
                root,
                &format!("lib/{file_module}/operations/chunk_{index:04}.rb"),
                contents,
            )?;
            let _ = writeln!(loader, "require_relative \"operations/chunk_{index:04}\"");
        }
        if client_style == SdkClientStyle::Namespaced {
            let factories = resource_operations(api).keys().map(|resource| {
                let name = ruby_resource_attribute(api, resource);
                let class = pascal_case(resource);
                format!("  ResourceFactories[:{name}] = ->(client) {{ {class}Resource.new(client) }}\n  Client.class_eval {{ attr_reader :{name} }}\n")
            }).collect::<Vec<_>>();
            for (index, chunk) in bounded_source_chunks(&factories).iter().enumerate() {
                let contents = format!("{NOTICE}module {module}\n{}end\n", chunk.concat());
                insert(
                    &mut tree,
                    root,
                    &format!("lib/{file_module}/resources/factories_{index:04}.rb"),
                    contents,
                )?;
                let _ = writeln!(
                    loader,
                    "require_relative \"resources/factories_{index:04}\""
                );
            }

            for (resource, operations) in resource_operations(api) {
                for (index, chunk) in operations.chunks(100).enumerate() {
                    let name = ruby_file_name(&resource);
                    let contents = format!(
                        "{NOTICE}module {module}\n{}end\n",
                        render_resource(api, &resource, chunk)
                    );
                    insert(
                        &mut tree,
                        root,
                        &format!("lib/{file_module}/resources/{name}_{index:04}.rb"),
                        contents,
                    )?;
                    let _ = writeln!(loader, "require_relative \"resources/{name}_{index:04}\"");
                }
            }
        }
        loader
    };
    insert(
        &mut tree,
        root,
        &format!("lib/{file_module}/client.rb"),
        client_contents,
    )?;
    for (path, contents) in response_validation::render_partitioned(api, &module) {
        insert(
            &mut tree,
            root,
            &format!("lib/{file_module}/{path}"),
            contents,
        )?;
    }
    Ok(tree)
}

/// Bound complete declarations; one indivisible large declaration is reported by output metrics.
pub(crate) fn bounded_source_chunks(items: &[String]) -> Vec<&[String]> {
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut bytes = 0;
    for (index, item) in items.iter().enumerate() {
        if index > start && (index - start >= 100 || bytes + item.len() > 128 * 1024 - 512) {
            chunks.push(&items[start..index]);
            start = index;
            bytes = 0;
        }
        bytes += item.len();
    }
    if start < items.len() {
        chunks.push(&items[start..]);
    }
    chunks
}

pub(crate) fn insert(
    tree: &mut GeneratedTree,
    root: &str,
    path: &str,
    contents: String,
) -> Result<()> {
    tree.insert(GeneratedFile::new(format!("{root}/{path}"), contents)?)
}

pub(crate) fn gemspec(api: &Api, gem: &str) -> String {
    format!(
        "Gem::Specification.new do |spec|\n  spec.name = {}\n  spec.version = {}\n  spec.summary = {}\n  spec.required_ruby_version = \">= 3.1\"\n  spec.files = Dir[\"lib/**/*.rb\"]\n  spec.require_paths = [\"lib\"]\nend\n",
        ruby_string(gem),
        ruby_string(&ruby_version(&api.version)),
        ruby_string(&format!("Generated Ruby SDK for {}", api.name))
    )
}

pub(crate) fn readme(api: &Api, gem: &str, module: &str, style: SdkClientStyle) -> String {
    let usage = match style {
        SdkClientStyle::Flat => "client.get_contact(contact_id: \"contact_123\")",
        SdkClientStyle::Namespaced => "client.contacts.get(contact_id: \"contact_123\")",
    };
    format!(
        "# {} Ruby SDK\n\nGenerated by Poolster for Ruby 3.1+. It only uses the Ruby standard library.\n\n```sh\ngem install {gem}\n```\n\n```ruby\nrequire {}\n\nclient = {module}::Client.new(base_url: \"https://api.example.com\", api_key: ENV.fetch(\"API_KEY\", nil))\n{usage}\n```\n\nSee [STYLE_GUIDE.md](STYLE_GUIDE.md) for the selected client surface.\n",
        api.name,
        ruby_string(&ruby_file_name(gem)),
    ) + &include_str!("../../templates/middleware_readme.md").replace("__MODULE__", module)
        + "\n## Pagination\n\nDeclared page-number operations expose `<operation>_pages(...)` as a lazy Enumerator, including resource facades. Each value is a complete response page. Optional page controls start at 1; explicit 0 is preserved. Empty results or a page shorter than a positive limit terminate iteration. Required JSON body controls are copied without mutating the caller. Iteration is bounded to 10,000 pages. Cursor, offset/limit and URL declarations are unsupported and appear in `.poolster/pagination-diagnostics.json`; ordinary operation calls remain available.\n"
}

pub(crate) fn style_guide(api: &Api, module: &str, style: SdkClientStyle) -> String {
    let surface = match style {
        SdkClientStyle::Flat => "Operations are direct methods such as `client.get_contact(...)`.",
        SdkClientStyle::Namespaced => {
            "Operations remain direct methods and are also grouped by resource, such as `client.contacts.get(...)`."
        }
    };
    format!(
        "# {} Ruby SDK style guide\n\nNamespace: `{module}`.\n\n{surface}\n",
        api.name
    )
}
