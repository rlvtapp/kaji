//! Runtime emission for the elixir HTTP SDK.
use crate::*;

pub(crate) fn render_json(module: &str) -> String {
    format!(
        "{NOTICE}\ndefmodule {module}.JSON do\n  @moduledoc false\n\n  @spec to_wire(term()) :: term()\n  def to_wire(value) when is_list(value), do: Enum.map(value, &to_wire/1)\n  def to_wire(value) when is_map(value) do\n    if is_struct(value) and function_exported?(value.__struct__, :to_map, 1) do\n      value.__struct__.to_map(value)\n    else\n      Map.new(value, fn {{key, item}} -> {{key, to_wire(item)}} end)\n    end\n  end\n  def to_wire(value), do: value\n\n  @spec decode(binary()) :: {{:ok, term()}} | {{:error, term()}}\n  def decode(\"\"), do: {{:ok, nil}}\n  def decode(body), do: Jason.decode(body)\nend\n"
    )
}

pub(crate) fn render_client(module: &str) -> String {
    format!(
        "{NOTICE}\n{}",
        include_str!("../../templates/http/client.ex.tmpl")
    )
    .replace("__POOLSTER_MODULE__", module)
}
