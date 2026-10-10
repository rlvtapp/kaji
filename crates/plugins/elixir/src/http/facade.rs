//! Facade planning and emission for Elixir HTTP.
use crate::*;

pub(crate) fn render_api_facade(module: &str, api: &Api) -> String {
    let mut output = format!(
        "{NOTICE}\ndefmodule {module}.API do\n  @moduledoc \"Typed API operations for {}.\"\n\n",
        escape_elixir_string(&api.name)
    );
    for (index, range) in elixir_operation_groups(module, api).into_iter().enumerate() {
        let operations = &api.operations[range];
        let _ = writeln!(output, "  alias {module}.API.Operations{index:04}");
        for operation in operations {
            let name = elixir_identifier(&operation.id);
            let _ = writeln!(
                output,
                "\n  def {name}(client, options \\\\ []), do: Operations{index:04}.{name}(client, options)"
            );
            if cursor_pagination(operation).is_some()
                || page_pagination::render(api, operation)
                    .ok()
                    .flatten()
                    .is_some()
            {
                let _ = writeln!(
                    output,
                    "  def {name}_pages(client, options \\\\ []), do: Operations{index:04}.{name}_pages(client, options)"
                );
            }
        }
    }
    output.push_str("end\n");
    output
}

pub(crate) fn render_operation_chunk(
    module: &str,
    api: &Api,
    operations: &[Operation],
    index: usize,
) -> String {
    let mut output = format!(
        "{NOTICE}\ndefmodule {module}.API.Operations{index:04} do\n  @moduledoc false\n\n  alias {module}.Client\n\n"
    );
    for operation in operations {
        output.push_str(&render_operation(module, api, operation));
    }
    output.push_str("end\n");
    output
}

pub(crate) fn render_resource_facade(
    module: &str,
    api: &Api,
    resource: &str,
    operations: &[&Operation],
) -> String {
    let mut output = format!(
        "{NOTICE}\ndefmodule {module}.Resources.{resource} do\n  @moduledoc \"Resource-namespaced operations for {resource}.\"\n\n"
    );
    for (index, range) in elixir_resource_groups(module, api, resource, operations)
        .into_iter()
        .enumerate()
    {
        let chunk = &operations[range];
        let _ = writeln!(
            output,
            "\n  alias {module}.Resources.{resource}.Chunk{index:04}"
        );
        for operation in chunk {
            let name = elixir_identifier(&operation.id);
            let _ = writeln!(
                output,
                "  def {name}(client, options \\\\ []), do: Chunk{index:04}.{name}(client, options)"
            );
            if cursor_pagination(operation).is_some()
                || page_pagination::render(api, operation)
                    .ok()
                    .flatten()
                    .is_some()
            {
                let _ = writeln!(
                    output,
                    "  def {name}_pages(client, options \\\\ []), do: Chunk{index:04}.{name}_pages(client, options)"
                );
            }
        }
    }
    output.push_str("end\n");
    output
}

pub(crate) fn render_resource_chunk(
    module: &str,
    api: &Api,
    resource: &str,
    operations: &[&Operation],
    index: usize,
) -> String {
    let mut output = format!(
        "{NOTICE}\ndefmodule {module}.Resources.{resource}.Chunk{index:04} do\n  @moduledoc false\n\n  alias {module}.{{API, Client}}\n"
    );
    for operation in operations {
        let name = elixir_identifier(&operation.id);
        let _ = writeln!(
            output,
            "\n  @spec {name}(Client.t(), keyword()) :: {{:ok, term()}} | {{:error, term()}}\n  def {name}(client, options \\\\ []), do: API.{name}(client, options)"
        );
        if cursor_pagination(operation).is_some()
            || page_pagination::render(api, operation)
                .ok()
                .flatten()
                .is_some()
        {
            let _ = writeln!(
                output,
                "\n  @spec {name}_pages(Client.t(), keyword()) :: Enumerable.t()\n  def {name}_pages(client, options \\\\ []), do: API.{name}_pages(client, options)"
            );
        }
    }
    output.push_str("end\n");
    output
}

pub(crate) fn operation_groups(api: &Api) -> Vec<String> {
    let mut groups = api
        .operations
        .iter()
        .map(operation_resource_name)
        .collect::<Vec<_>>();
    groups.sort();
    groups.dedup();
    groups
}
