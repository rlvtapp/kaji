//! Assembly emission for the elixir HTTP SDK.
use crate::*;

/// Generates a self-contained Elixir Mix SDK under `output_dir`.
///
/// The generated SDK owns a named Finch process, configurable via
/// `Client.new/1`, and has no generator-runtime dependency. `package_name`
/// controls the Mix application name when provided; it is otherwise derived
/// from the source API title.
#[cfg(test)]
pub(crate) fn render_test_sdk(
    api: &Api,
    output_dir: &str,
    package_name: Option<&str>,
) -> Result<GeneratedTree> {
    render_sdk(api, output_dir, package_name, SdkClientStyle::Flat)
}

/// Generates an Elixir SDK with a direct API module or a resource namespace
/// facade.
///
/// [`SdkClientStyle::Flat`] exports the direct `<Sdk>.API.operation/2`
/// output. [`SdkClientStyle::Namespaced`] adds modules such as
/// `<Sdk>.Resources.Contacts.operation/2`, delegating to those typed direct
/// functions while sharing the same normal `Client` value.
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
        bail!("Elixir SDK output directory cannot be empty");
    }

    let package = package_name
        .filter(|name| !name.trim().is_empty())
        .map(package_slug)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("{}-sdk", package_slug(&api.name)));
    let app = elixir_identifier(&package);
    let module = pascal_case(&package);
    for operation in &api.operations {
        page_pagination::render(api, operation)?;
    }
    let mut tree = GeneratedTree::default();
    insert(
        &mut tree,
        root,
        &format!("lib/{app}/multipart_body.ex"),
        include_str!("../../templates/multipart.ex.tmpl").replace("__POOLSTER_MODULE__", &module),
    )?;

    insert(
        &mut tree,
        root,
        "mix.exs",
        render_mix_exs(api, &app, &module),
    )?;
    insert(
        &mut tree,
        root,
        &format!("lib/{app}.ex"),
        render_root_module(&module, client_style),
    )?;
    insert(
        &mut tree,
        root,
        &format!("lib/{app}/application.ex"),
        render_application(&module),
    )?;
    insert(
        &mut tree,
        root,
        &format!("lib/{app}/api_error.ex"),
        render_api_error(api, &module),
    )?;
    insert(
        &mut tree,
        root,
        &format!("lib/{app}/json.ex"),
        render_json(&module),
    )?;
    insert(
        &mut tree,
        root,
        &format!("lib/{app}/client.ex"),
        render_client(&module),
    )?;
    let model_files = unique_file_stems(api.schemas.iter().map(|schema| schema.name.as_str()));
    for (schema, file_name) in api.schemas.iter().zip(model_files) {
        insert(
            &mut tree,
            root,
            &format!("lib/{app}/models/{file_name}.ex"),
            render_model(&module, schema),
        )?;
    }
    insert(
        &mut tree,
        root,
        &format!("lib/{app}/api.ex"),
        render_api_facade(&module, api),
    )?;
    for (index, range) in elixir_operation_groups(&module, api)
        .into_iter()
        .enumerate()
    {
        let operations = &api.operations[range];
        insert(
            &mut tree,
            root,
            &format!("lib/{app}/api/operations_{:04}.ex", index + 1),
            render_operation_chunk(&module, api, operations, index),
        )?;
    }
    let api_facade_path = format!("{root}/lib/{app}/api.ex");
    if tree
        .get(&api_facade_path)
        .is_some_and(|value| value.len() > 128 * 1024)
    {
        let module_ref = module.as_str();
        let delegates = elixir_operation_groups(&module, api)
            .into_iter()
            .enumerate()
            .flat_map(|(index, range)| {
                api.operations[range].iter().map(move |operation| {
                    (operation, format!("{module_ref}.API.Operations{index:04}"))
                })
            })
            .collect::<Vec<_>>();
        bound_elixir_facade(
            &mut tree,
            root,
            &format!("lib/{app}/api.ex"),
            &format!("lib/{app}/api"),
            &format!("{module}.API"),
            api,
            &delegates,
        )?;
    }
    for (index, range) in elixir_error_groups(&module, api).into_iter().enumerate() {
        let operations = &api.operations[range];
        insert(
            &mut tree,
            root,
            &format!("lib/{app}/errors/chunk_{:04}.ex", index + 1),
            render_declared_errors(&module, operations),
        )?;
    }
    if client_style == SdkClientStyle::Namespaced {
        for resource in operation_groups(api) {
            let file_name = bounded_file_stem(&resource);
            let operations = api
                .operations
                .iter()
                .filter(|operation| operation_resource_name(operation) == resource)
                .collect::<Vec<_>>();
            insert(
                &mut tree,
                root,
                &format!("lib/{app}/resources/{file_name}.ex"),
                render_resource_facade(&module, api, &resource, &operations),
            )?;
            for (index, range) in elixir_resource_groups(&module, api, &resource, &operations)
                .into_iter()
                .enumerate()
            {
                let chunk = &operations[range];
                insert(
                    &mut tree,
                    root,
                    &format!("lib/{app}/resources/{file_name}/chunk_{:04}.ex", index + 1),
                    render_resource_chunk(&module, api, &resource, chunk, index),
                )?;
            }
            let facade_path = format!("{root}/lib/{app}/resources/{file_name}.ex");
            if tree
                .get(&facade_path)
                .is_some_and(|value| value.len() > 128 * 1024)
            {
                let module_ref = module.as_str();
                let resource_ref = resource.as_str();
                let delegates = elixir_resource_groups(&module, api, &resource, &operations)
                    .into_iter()
                    .enumerate()
                    .flat_map(|(index, range)| {
                        operations[range].iter().map(move |operation| {
                            (
                                *operation,
                                format!("{module_ref}.Resources.{resource_ref}.Chunk{index:04}"),
                            )
                        })
                    })
                    .collect::<Vec<_>>();
                bound_elixir_facade(
                    &mut tree,
                    root,
                    &format!("lib/{app}/resources/{file_name}.ex"),
                    &format!("lib/{app}/resources/{file_name}"),
                    &format!("{module}.Resources.{resource}"),
                    api,
                    &delegates,
                )?;
            }
        }
    }
    insert(
        &mut tree,
        root,
        "README.md",
        render_readme(api, &package, &module, client_style)
            + if api.operations.iter().any(|operation| {
                page_pagination::render(api, operation)
                    .ok()
                    .flatten()
                    .is_some()
            }) {
                "\n## Page-number pagination\n\nDeclared page-number operations expose `<operation>_pages(client, options)`, a lazy stream of `{:ok, full_response}` pages. Use `Enum.take/2` or other stream consumers; each demand calls the original operation with retained options and an updated page. Optional page defaults to 1; explicit 0 is preserved; required page must be supplied. Empty and short pages are yielded and terminate iteration (short pages require a positive declared limit). Invalid controls, malformed results, operation errors, or the 10,000-page guard yield one `{:error, reason}` and halt. Page controls in request bodies are rejected during generation.\n"
            } else {
                ""
            },
    )?;
    insert(
        &mut tree,
        root,
        "STYLE_GUIDE.md",
        render_style_guide(api, &module, client_style),
    )?;
    Ok(tree)
}

pub(crate) fn insert(
    tree: &mut GeneratedTree,
    root: &str,
    path: &str,
    contents: String,
) -> Result<()> {
    tree.insert(GeneratedFile::new(format!("{root}/{path}"), contents)?)
}

pub(crate) fn render_mix_exs(api: &Api, app: &str, module: &str) -> String {
    format!(
        "defmodule {module}.MixProject do\n  use Mix.Project\n\n  def project do\n    [\n      app: :{app},\n      version: \"{}\",\n      elixir: \"~> 1.15\",\n      start_permanent: Mix.env() == :prod,\n      deps: deps(),\n      description: \"Generated Elixir SDK for {}\"\n    ]\n  end\n\n  def application do\n    [extra_applications: [:logger, :inets, :crypto], mod: {{{module}.Application, []}}]\n  end\n\n  defp deps do\n    [\n      {{:finch, \"~> 0.18\"}},\n      {{:jason, \"~> 1.4\"}}\n    ]\n  end\nend\n",
        package_version(&api.version),
        escape_elixir_string(&api.name),
    )
}

pub(crate) fn render_root_module(module: &str, client_style: SdkClientStyle) -> String {
    let resources = if client_style == SdkClientStyle::Namespaced {
        "\n  Resource modules are also available below `Resources`, for example `Resources.Contacts.create_contact(client, body: contact)`."
    } else {
        ""
    };
    format!(
        "{NOTICE}\ndefmodule {module} do\n  @moduledoc \"Generated Poolster SDK entry point.{resources}\"\n\n  alias {module}.Client\n\n  @spec client(keyword()) :: {{:ok, Client.t()}} | {{:error, term()}}\n  def client(options \\\\ []), do: Client.new(options)\nend\n"
    )
}

pub(crate) fn render_application(module: &str) -> String {
    format!(
        "{NOTICE}\ndefmodule {module}.Application do\n  @moduledoc false\n  use Application\n\n  @impl true\n  def start(_type, _args) do\n    Finch.start_link(name: {module}.Finch)\n  end\nend\n"
    )
}
