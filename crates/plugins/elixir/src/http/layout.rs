//! Layout planning and emission for Elixir HTTP.
use crate::*;

pub(crate) fn bound_elixir_facade(
    tree: &mut GeneratedTree,
    root: &str,
    entry: &str,
    folder: &str,
    facade: &str,
    api: &Api,
    delegates: &[(&Operation, String)],
) -> Result<()> {
    let declarations = delegates.iter().map(|(operation, target)| {
        let name = elixir_identifier(&operation.id);
        let mut value = format!("      def {name}(client, options \\\\ []), do: {target}.{name}(client, options)\n");
        if cursor_pagination(operation).is_some() || page_pagination::render(api, operation).ok().flatten().is_some() {
            let _ = writeln!(value, "      def {name}_pages(client, options \\\\ []), do: {target}.{name}_pages(client, options)");
        }
        value
    }).collect::<Vec<_>>();
    let sizes = declarations.iter().map(String::len).collect::<Vec<_>>();
    let groups = elixir_source_ranges(&sizes, 512 + facade.len(), 100);
    let mut entry_source = format!(
        "{NOTICE}defmodule {facade} do\n  @moduledoc \"Typed API operations for {}.\"\n",
        escape_elixir_string(&api.name)
    );
    for (index, range) in groups.into_iter().enumerate() {
        let macro_module = format!("{facade}.Delegates{index:04}");
        let source = format!(
            "{NOTICE}defmodule {macro_module} do\n  @moduledoc false\n  defmacro __using__(_options) do\n    quote do\n{}    end\n  end\nend\n",
            declarations[range].concat()
        );
        insert(
            tree,
            root,
            &format!("{folder}/delegates_{index:04}.ex"),
            source,
        )?;
        let _ = writeln!(entry_source, "  use {macro_module}");
    }
    entry_source.push_str("end\n");
    tree.replace(GeneratedFile::new(format!("{root}/{entry}"), entry_source)?)?;
    Ok(())
}

pub(crate) fn elixir_source_ranges(
    sizes: &[usize],
    overhead: usize,
    count: usize,
) -> Vec<std::ops::Range<usize>> {
    let units = sizes
        .iter()
        .map(|bytes| poolster_core::source_layout::SourceUnit {
            bytes: *bytes,
            resource: None,
        })
        .collect::<Vec<_>>();
    poolster_core::source_layout::SourceLayout::Chunked {
        max_file_bytes: 128 * 1024,
        max_declarations: Some(count),
    }
    .groups(&units, overhead)
    .expect("valid bounded source layout")
    .into_iter()
    .map(|group| group[0]..group[group.len() - 1] + 1)
    .collect()
}

pub(crate) fn elixir_operation_groups(module: &str, api: &Api) -> Vec<std::ops::Range<usize>> {
    let overhead = render_operation_chunk(module, api, &[], 0).len();
    let sizes = api
        .operations
        .iter()
        .map(|operation| {
            render_operation_chunk(module, api, std::slice::from_ref(operation), 0)
                .len()
                .saturating_sub(overhead)
        })
        .collect::<Vec<_>>();
    elixir_source_ranges(&sizes, overhead + 64, OPERATIONS_PER_FILE)
}

pub(crate) fn elixir_error_groups(module: &str, api: &Api) -> Vec<std::ops::Range<usize>> {
    let overhead = render_declared_errors(module, &[]).len();
    let sizes = api
        .operations
        .iter()
        .map(|operation| {
            render_declared_errors(module, std::slice::from_ref(operation))
                .len()
                .saturating_sub(overhead)
        })
        .collect::<Vec<_>>();
    elixir_source_ranges(&sizes, overhead + 64, ERROR_OPERATIONS_PER_FILE)
}

pub(crate) fn elixir_resource_groups(
    module: &str,
    api: &Api,
    resource: &str,
    operations: &[&Operation],
) -> Vec<std::ops::Range<usize>> {
    let overhead = render_resource_chunk(module, api, resource, &[], 0).len();
    let sizes = operations
        .iter()
        .map(|operation| {
            render_resource_chunk(module, api, resource, std::slice::from_ref(operation), 0)
                .len()
                .saturating_sub(overhead)
        })
        .collect::<Vec<_>>();
    elixir_source_ranges(&sizes, overhead + 64, RESOURCE_METHODS_PER_FILE)
}
