//! Models emission for the elixir HTTP SDK.
use crate::*;

pub(crate) fn render_model(module: &str, schema: &Schema) -> String {
    let type_name = pascal_case(&schema.name);
    let model = format!("{module}.Models.{type_name}");
    match &schema.value.kind {
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            let open = !matches!(additional_properties, AdditionalProperties::Forbidden);
            let mut presence = "poolster_present_fields".to_owned();
            while fields
                .iter()
                .any(|field| elixir_field_identifier(fields, field) == presence)
            {
                presence.push('_');
            }
            let mut extras = "additional_properties".to_owned();
            while extras == presence
                || fields
                    .iter()
                    .any(|field| elixir_field_identifier(fields, field) == extras)
            {
                extras.push('_');
            }
            let mut output = format!(
                "{NOTICE}\ndefmodule {model} do\n  @moduledoc \"Generated model for {}.\"\n  alias {module}.JSON\n\n",
                escape_elixir_string(&schema.name)
            );
            if fields.iter().any(|field| field.required) {
                let required = fields
                    .iter()
                    .filter(|field| field.required)
                    .map(|field| format!(":{}", elixir_field_identifier(fields, field)))
                    .collect::<Vec<_>>();
                let _ = writeln!(output, "  @enforce_keys [{}]", required.join(", "));
            }
            let mut names = fields
                .iter()
                .map(|field| format!(":{}", elixir_field_identifier(fields, field)))
                .collect::<Vec<_>>();
            names.push(format!("{presence}: nil"));
            if open {
                names.push(format!("{extras}: %{{}}"));
            }
            // Keyword defaults must follow positional atom entries.
            let _ = writeln!(output, "  defstruct [{}]", names.join(", "));
            output.push_str("\n  @type t :: %__MODULE__{\n");
            for field in fields {
                let optional = !field.required || field.value.nullable;
                let _ = writeln!(
                    output,
                    "    {}: {}{},",
                    elixir_field_identifier(fields, field),
                    elixir_type(&field.value, module),
                    if optional { " | nil" } else { "" }
                );
            }
            let _ = writeln!(output, "    {presence}: [String.t()] | nil,");
            if open {
                let _ = writeln!(output, "    {extras}: map(),");
            }
            output.push_str("  }\n\n  @spec to_map(t()) :: map()\n  def to_map(model) do\n    [\n");
            for field in fields {
                let _ = writeln!(
                    output,
                    "      {{\"{}\", model.{}}},",
                    escape_elixir_string(&field.name),
                    elixir_field_identifier(fields, field)
                );
            }
            let known_keys = fields
                .iter()
                .map(|field| format!("\"{}\"", escape_elixir_string(&field.name)))
                .collect::<Vec<_>>()
                .join(", ");
            let required_keys = fields
                .iter()
                .filter(|field| field.required)
                .map(|field| format!("\"{}\"", escape_elixir_string(&field.name)))
                .collect::<Vec<_>>()
                .join(", ");
            output.push_str(&format!("    ]\n    |> Enum.reject(fn {{key, value}} -> is_nil(value) and key not in [{required_keys}] and key not in (model.{presence} || []) end)\n"));
            output.push_str("    |> Map.new(fn {key, value} -> {key, JSON.to_wire(value)} end)\n");
            if open {
                output.push_str(&format!(
                    "    |> then(fn known -> Map.merge(Map.drop(JSON.to_wire(model.{extras}), [{known_keys}]), known) end)\n"
                ));
            }
            let _ = writeln!(
                output,
                "  end\n\n  @doc \"Return a copy with explicit nulls present at the given wire keys.\"\n  @spec with_present_fields(t(), [String.t() | atom()]) :: t()\n  def with_present_fields(model, fields) when is_list(fields) do\n    keys = Enum.map(fields, &to_string/1)\n    if Enum.any?(keys, &(&1 not in [{known_keys}])), do: raise(ArgumentError, \"unknown model wire field\")\n    %{{model | {presence}: Enum.uniq((model.{presence} || []) ++ keys)}}\n  end\n\n  @spec from_map(map()) :: t()\n  def from_map(map) when is_map(map) do\n    %__MODULE__{{"
            );
            for field in fields {
                let access = format!("Map.get(map, \"{}\")", escape_elixir_string(&field.name));
                let _ = writeln!(
                    output,
                    "      {}: {},",
                    elixir_field_identifier(fields, field),
                    decode_value(&access, &field.value, module)
                );
            }
            let _ = writeln!(output, "      {presence}: Map.keys(map),");
            if open {
                let known = fields
                    .iter()
                    .map(|field| format!("\"{}\"", escape_elixir_string(&field.name)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let extra_value = match additional_properties {
                    AdditionalProperties::Schema { value } => format!(
                        "Map.new(Map.drop(map, [{known}]), fn {{key, item}} -> {{key, {}}} end)",
                        decode_value("item", value, module)
                    ),
                    _ => format!("Map.drop(map, [{known}])"),
                };
                let _ = writeln!(output, "      {extras}: {extra_value},");
            }
            output.push_str("    }\n  end\n  def from_map(value), do: value\nend\n");
            output
        }
        _ => format!(
            "{NOTICE}\ndefmodule {model} do\n  @moduledoc \"Generated type for {}.\"\n  @type t :: {}\n  def from_map(value) do\n    {}\n  end\nend\n",
            escape_elixir_string(&schema.name),
            elixir_type(&schema.value, module),
            decode_value("value", &schema.value, module)
        ),
    }
}
