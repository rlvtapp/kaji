//! Cursor emission for the elixir HTTP SDK.
use crate::*;

pub(crate) fn cursor_pagination(operation: &Operation) -> Option<CursorPagination> {
    let extension = poolster_core::poolster_extension(&operation.annotations, "pagination")
        .or_else(|| operation.annotations.get("x-speakeasy-pagination"))?
        .as_object()?;
    if extension.get("type").and_then(Value::as_str) != Some("cursor") {
        return None;
    }
    let input = extension
        .get("inputs")
        .and_then(Value::as_array)?
        .iter()
        .find(|input| input.get("type").and_then(Value::as_str) == Some("cursor"))?
        .as_object()?;
    if !matches!(
        input.get("in").and_then(Value::as_str),
        None | Some("parameters")
    ) {
        return None;
    }
    let parameter_name = input.get("name")?.as_str()?;
    let parameter = operation.parameters.iter().find(|parameter| {
        parameter.name == parameter_name
            && matches!(parameter.location.as_str(), "query" | "header" | "path")
            && parameter
                .schema
                .as_ref()
                .is_some_and(|schema| matches!(schema.kind, SchemaKind::String))
            && (parameter.location != "path" || parameter.required)
    })?;
    Some(CursorPagination {
        parameter_name: elixir_parameter_identifier(operation, parameter),
        next_cursor_path: extension
            .get("outputs")?
            .get("nextCursor")?
            .as_str()?
            .to_owned(),
    })
}

pub(crate) fn render_cursor_paginator(
    operation: &Operation,
    pagination: &CursorPagination,
) -> String {
    let name = elixir_identifier(&operation.id);
    format!(
        "  @doc \"Lazily yields declared cursor pages.\"\n  @spec {name}_pages(Client.t(), keyword()) :: Enumerable.t()\n  def {name}_pages(client, options \\\\ []) when is_list(options) do\n    Stream.resource(\n      fn -> {{:next, options}} end,\n      fn\n        :halt -> {{:halt, :halt}}\n        {{:next, current}} ->\n          case {name}(client, current) do\n            {{:ok, response}} ->\n              case Client.json_path(response, {:?}) do\n                cursor when is_binary(cursor) and cursor != \"\" -> {{[{{:ok, response}}], {{:next, Keyword.put(current, :{}, cursor)}}}}\n                _ -> {{[{{:ok, response}}], :halt}}\n              end\n            {{:error, reason}} -> {{[{{:error, reason}}], :halt}}\n          end\n      end,\n      fn _ -> :ok end\n    )\n  end\n\n",
        pagination.next_cursor_path, pagination.parameter_name,
    )
}

// Facade emission for the elixir HTTP SDK.

/// Elixir keeps pagination explicit: a generated pager exists only for a
/// declared string cursor that maps to an existing operation option. The
/// ordinary operation owns query/header/path serialization on every page.
#[derive(Clone, Debug)]
pub(crate) struct CursorPagination {
    pub(crate) parameter_name: String,
    pub(crate) next_cursor_path: String,
}
