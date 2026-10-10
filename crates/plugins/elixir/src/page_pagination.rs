use super::*;
use poolster_core::pagination::{PaginationKind, SelectorSegment, normalize_pagination};
pub(super) fn render(api: &Api, operation: &Operation) -> Result<Option<String>> {
    let extension = poolster_core::poolster_extension(&operation.annotations, "pagination")
        .or_else(|| operation.annotations.get("x-speakeasy-pagination"));
    let kind = extension
        .and_then(|value| value.get("type"))
        .and_then(serde_json::Value::as_str);
    if !matches!(kind, Some("page" | "offsetLimit" | "url")) {
        return Ok(None);
    }
    let Some(plan) = normalize_pagination(api, operation, None)? else {
        return Ok(None);
    };
    if matches!(plan.kind, PaginationKind::OffsetLimit | PaginationKind::Url) {
        ensure_buffered_json(operation)?;
    }
    if plan.kind == PaginationKind::Url {
        return render_url(operation, &plan);
    }
    let offset = plan.kind == PaginationKind::OffsetLimit;
    if plan
        .inputs
        .iter()
        .any(|input| input.location == "requestBody")
    {
        bail!(
            "Elixir page pagination for {} requires parameter controls",
            operation.id
        )
    }
    let page = plan
        .inputs
        .iter()
        .find(|input| input.role == if offset { "offset" } else { "page" })
        .unwrap();
    let key = elixir_pagination_argument(operation, page);
    let initial = if page.required {
        format!("Keyword.get(options, :{key})")
    } else {
        format!(
            "Keyword.get(options, :{key}) || {}",
            if offset { 0 } else { 1 }
        )
    };
    let limit = plan
        .inputs
        .iter()
        .find(|input| input.role == "limit")
        .map(|input| {
            format!(
                "Keyword.get(options, :{})",
                elixir_pagination_argument(operation, input)
            )
        })
        .unwrap_or_else(|| "nil".into());
    let segments = plan
        .results
        .unwrap()
        .segments
        .iter()
        .map(|segment| match segment {
            SelectorSegment::Field(field) => {
                format!(
                    "{{:field, \"{}\"}}",
                    escape_elixir_string(field).replace("#{", "\\#{")
                )
            }
            SelectorSegment::Index(index) => format!("{{:index, {index}}}"),
        })
        .collect::<Vec<_>>()
        .join(", ");
    let name = elixir_identifier(&operation.id);
    let advance = if offset {
        "page + length(items)"
    } else {
        "page + 1"
    };
    let control = if offset { "offset" } else { "page" };
    Ok(Some(format!(
        r#"
  @doc "Lazily yields full declared {control} responses; errors terminate the stream."
  def {name}_pages(client, options \\ []) do
    Stream.resource(
      fn -> {{{initial}, {limit}, 0}} end,
      fn
        :halt -> {{:halt, :halt}}
        {{page, limit, count}} ->
          cond do
            not is_integer(page) or page < 0 -> {{[{{:error, :invalid_pagination_{control}}}], :halt}}
            limit != nil and (not is_integer(limit) or limit <= 0) -> {{[{{:error, :invalid_pagination_limit}}], :halt}}
            count >= 10000 -> {{[{{:error, :pagination_limit}}], :halt}}
            true ->
              case {name}(client, Keyword.put(options, :{key}, page)) do
                {{:ok, response}} ->
                  case Client.json_path(response, [{segments}]) do
                    items when is_list(items) ->
                      next = if items == [] or (limit != nil and length(items) < limit), do: :halt, else: {{{advance}, limit, count + 1}}
                      {{[{{:ok, response}}], next}}
                    _ -> {{[{{:error, :invalid_pagination_results}}], :halt}}
                  end
                {{:error, reason}} -> {{[{{:error, reason}}], :halt}}
              end
          end
      end,
      fn _ -> :ok end)
  end
"#
    )))
}

fn ensure_buffered_json(operation: &Operation) -> Result<()> {
    let response = operation
        .responses
        .iter()
        .find(|response| response.status.starts_with('2'))
        .ok_or_else(|| {
            anyhow::anyhow!("Elixir pagination requires a declared successful JSON response")
        })?;
    anyhow::ensure!(
        !response.media_types.is_empty()
            && response
                .media_types
                .iter()
                .all(|media| media.content_type == "application/json"
                    || media.content_type.ends_with("+json")),
        "Elixir offset/URL pagination requires buffered JSON responses"
    );
    Ok(())
}

fn render_url(
    operation: &Operation,
    plan: &poolster_core::pagination::PaginationPlan,
) -> Result<Option<String>> {
    let segments = plan
        .continuation
        .as_ref()
        .unwrap()
        .segments
        .iter()
        .map(|segment| match segment {
            SelectorSegment::Field(field) => format!(
                "{{:field, \"{}\"}}",
                escape_elixir_string(field).replace("#{", "\\#{")
            ),
            SelectorSegment::Index(index) => format!("{{:index, {index}}}"),
        })
        .collect::<Vec<_>>()
        .join(", ");
    let name = elixir_identifier(&operation.id);
    Ok(Some(format!(
        r#"
  @doc "Lazy absolute same-origin next-URL responses; relative URLs are rejected."
  def {name}_pages(client, options \\ []) do
    Stream.resource(fn -> {{nil, MapSet.new(), 0}} end,
      fn
        :halt -> {{:halt, :halt}}
        :invalid -> {{[{{:error, :invalid_pagination_url}}], :halt}}
        {{url, seen, count}} ->
          cond do
            count >= 10000 -> {{[{{:error, :pagination_limit}}], :halt}}
            url != nil and MapSet.member?(seen, url) -> {{[{{:error, :pagination_loop}}], :halt}}
            true ->
              request_options = if url == nil, do: Keyword.delete(options, :_poolster_pagination_url), else: Keyword.put(options, :_poolster_pagination_url, url)
              case {name}(client, request_options) do
                {{:ok, response}} ->
                  seen = if url == nil, do: seen, else: MapSet.put(seen, url)
                  next = case Client.json_path(response, [{segments}]) do
                    nil -> :halt
                    "" -> :halt
                    next when is_binary(next) -> {{next, seen, count + 1}}
                    _ -> :invalid
                  end
                  {{[{{:ok, response}}], next}}
                {{:error, reason}} -> {{[{{:error, reason}}], :halt}}
              end
          end
      end, fn _ -> :ok end)
  end
"#
    )))
}

#[cfg(test)]
#[path = "page_pagination/tests.rs"]
mod tests;
