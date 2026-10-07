use super::*;
use kaji_core::pagination::{PaginationKind, SelectorSegment, normalize_pagination};
pub(super) fn render(api: &Api, operation: &Operation) -> Result<Option<String>> {
    let extension = operation
        .annotations
        .get("x-kaji-pagination")
        .or_else(|| operation.annotations.get("x-speakeasy-pagination"));
    if extension
        .and_then(|value| value.get("type"))
        .and_then(serde_json::Value::as_str)
        != Some("page")
    {
        return Ok(None);
    }
    let Some(plan) = normalize_pagination(api, operation, None)? else {
        return Ok(None);
    };
    if plan.kind != PaginationKind::Page {
        return Ok(None);
    }
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
        .find(|input| input.role == "page")
        .unwrap();
    let key = elixir_identifier(&page.name);
    let initial = if page.required {
        format!("Keyword.get(options, :{key})")
    } else {
        format!("Keyword.get(options, :{key}) || 1")
    };
    let limit = plan
        .inputs
        .iter()
        .find(|input| input.role == "limit")
        .map(|input| format!("Keyword.get(options, :{})", elixir_identifier(&input.name)))
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
    let name = snake_case(&operation.id);
    Ok(Some(format!(
        r#"
  @doc "Lazily yields full declared page-number responses; errors terminate the stream."
  def {name}_pages(client, options \\ []) do
    Stream.resource(
      fn -> {{{initial}, {limit}, 0}} end,
      fn
        :halt -> {{:halt, :halt}}
        {{page, limit, count}} ->
          cond do
            not is_integer(page) or page < 0 -> {{[{{:error, :invalid_pagination_page}}], :halt}}
            limit != nil and (not is_integer(limit) or limit <= 0) -> {{[{{:error, :invalid_pagination_limit}}], :halt}}
            count >= 10000 -> {{[{{:error, :pagination_limit}}], :halt}}
            true ->
              case {name}(client, Keyword.put(options, :{key}, page)) do
                {{:ok, response}} ->
                  case Client.json_path(response, [{segments}]) do
                    items when is_list(items) ->
                      next = if items == [] or (limit != nil and length(items) < limit), do: :halt, else: {{page + 1, limit, count + 1}}
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn page_plan_emits_lazy_canonical_helpers_and_facades() {
        use kaji_core::{HttpMethod, OperationParameter, OperationResponse};
        let mut operation = Operation {
            id: "listPets".into(),
            method: HttpMethod::Get,
            path: "/pets".into(),
            ..Default::default()
        };
        operation.parameters = vec![OperationParameter {
            name: "page".into(),
            location: "query".into(),
            required: false,
            schema: Some(SchemaValue::new(SchemaKind::Integer)),
            description: None,
            annotations: Default::default(),
        }];
        operation.responses = vec![OperationResponse::json(
            "200",
            SchemaValue::new(SchemaKind::Array {
                items: Box::new(SchemaValue::new(SchemaKind::String)),
            }),
        )];
        operation.annotations.insert("x-kaji-pagination".into(), serde_json::json!({"type":"page","inputs":[{"name":"page","type":"page","in":"parameters"}],"outputs":{"results":"$"}}));
        let mut api = Api {
            operations: vec![operation],
            ..Default::default()
        };
        let source = render(&api, &api.operations[0]).unwrap().unwrap();
        assert!(source.contains("Stream.resource"));
        assert!(source.contains("Keyword.get(options, :page) || 1"));
        assert!(source.contains("count >= 10000"));
        assert!(source.contains("Client.json_path(response, [])"));
        assert!(render_api_facade("Pets", &api).contains("list_pets_pages"));
        assert!(
            render_resource_facade("Pets", &api, "Pets", &[&api.operations[0]])
                .contains("list_pets_pages")
        );
        api.operations[0].parameters[0].required = true;
        let source = render(&api, &api.operations[0]).unwrap().unwrap();
        assert!(!source.contains("|| 1"));
        api.operations[0].annotations.insert("x-kaji-pagination".into(), serde_json::json!({"type":"page","inputs":[{"name":"page","type":"page"}],"outputs":{"results":"$.missing"}}));
        assert!(render(&api, &api.operations[0]).is_err());
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;
    #[test]
    #[ignore = "requires Elixir toolchain"]
    fn generated_page_stream_executes_lazily() {
        use kaji_core::{HttpMethod, OperationParameter, OperationResponse};
        let mut operation = Operation {
            id: "listPets".into(),
            method: HttpMethod::Get,
            path: "/pets".into(),
            ..Default::default()
        };
        operation.parameters = vec![
            OperationParameter {
                name: "page".into(),
                location: "query".into(),
                required: false,
                schema: Some(SchemaValue::new(SchemaKind::Integer)),
                description: None,
                annotations: Default::default(),
            },
            OperationParameter {
                name: "limit".into(),
                location: "header".into(),
                required: false,
                schema: Some(SchemaValue::new(SchemaKind::Integer)),
                description: None,
                annotations: Default::default(),
            },
        ];
        operation.responses = vec![OperationResponse::json(
            "200",
            SchemaValue::new(SchemaKind::Array {
                items: Box::new(SchemaValue::new(SchemaKind::String)),
            }),
        )];
        operation.annotations.insert("x-kaji-pagination".into(), serde_json::json!({"type":"page","inputs":[{"name":"page","type":"page"},{"name":"limit","type":"limit"}],"outputs":{"results":"$"}}));
        let api = Api {
            operations: vec![operation],
            ..Default::default()
        };
        let helper = render(&api, &api.operations[0]).unwrap().unwrap();
        let script = format!(
            r#"
defmodule Client do
 def json_path(value, []), do: value
end
defmodule Probe do
 def list_pets(_client, options) do
  page = Keyword.fetch!(options, :page)
  Process.put(:pages, Process.get(:pages, []) ++ [page])
  if Keyword.get(options, :tenant) != "kept", do: raise("lost options")
  {{:ok, if(page == 0, do: ["a", "b"], else: ["c"])}}
 end
{helper}
end
stream = Probe.list_pets_pages(nil, page: 0, limit: 2, tenant: "kept")
if Process.get(:pages) != nil, do: raise("eager request")
if Enum.to_list(stream) != [{{:ok, ["a", "b"]}}, {{:ok, ["c"]}}], do: raise("pages")
if Process.get(:pages) != [0, 1], do: raise("controls")
Process.delete(:pages)
if Enum.to_list(Probe.list_pets_pages(nil, page: -1)) != [{{:error, :invalid_pagination_page}}], do: raise("negative page")
if Enum.to_list(Probe.list_pets_pages(nil, limit: 0)) != [{{:error, :invalid_pagination_limit}}], do: raise("invalid limit")
if Process.get(:pages) != nil, do: raise("invalid request executed")
"#
        );
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("probe.exs");
        std::fs::write(&path, script).unwrap();
        let output = std::process::Command::new("elixir")
            .arg(path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
