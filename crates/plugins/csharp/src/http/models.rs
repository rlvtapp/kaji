//! Models emission for the csharp HTTP SDK.
use crate::*;

#[cfg(test)]
pub(crate) fn render_model(schema: &Schema, namespace: &str) -> String {
    render_model_with_policy(schema, namespace, false)
}

pub(crate) fn render_model_with_policy(
    schema: &Schema,
    namespace: &str,
    open_enums: bool,
) -> String {
    let mut output = format!(
        "{NOTICE}\nusing System.Text.Json;\nusing System.Text.Json.Serialization;\n\nnamespace {namespace};\n"
    );
    output.push('\n');
    render_schema_with_policy(&mut output, schema, open_enums);
    output
}

pub(crate) fn render_object_property(field: &poolster_core::Field, property: &str) -> String {
    let field_type = csharp_type(&field.value, !field.required);
    let required = if field.required && is_reference_type(&field.value) {
        "required "
    } else {
        ""
    };
    let preserve_null = if field.required
        && (field.value.nullable || field.value.optional || field.value.nullish)
    {
        "    [JsonIgnore(Condition = JsonIgnoreCondition.Never)]\n"
    } else {
        ""
    };
    format!(
        "    [JsonPropertyName({:?})]\n{preserve_null}    public {required}{field_type} {property} {{\n  get;\n  init;\n\n}}\n\n",
        field.name
    )
}

pub(crate) fn render_model_parts(
    schema: &Schema,
    namespace: &str,
    open_enums: bool,
    index: usize,
) -> Result<Vec<(String, String)>> {
    let whole = render_model_with_policy(schema, namespace, open_enums);
    let filename = bounded_filename(&schema.name, index, "cs");
    let SchemaKind::Object {
        fields,
        additional_properties,
    } = &schema.value.kind
    else {
        return Ok(vec![(filename, whole)]);
    };
    if whole.len() <= 128 * 1024 {
        return Ok(vec![(filename, whole)]);
    }
    let name = pascal_case(&schema.name);
    let names = native_names::field_names(fields, pascal_case, &[&name, "EqualityContract"]);
    let properties = fields
        .iter()
        .map(|field| render_object_property(field, &names[&field.name]))
        .collect::<Vec<_>>();
    let header = format!(
        "{NOTICE}\nusing System.Text.Json;\n\nusing System.Text.Json.Serialization;\n\nnamespace {namespace};\n\npublic sealed partial record {name}\n{{\n  "
    );
    let mut extra = String::new();
    if !matches!(additional_properties, AdditionalProperties::Forbidden) {
        let mut property = "AdditionalProperties".to_owned();
        while fields.iter().any(|field| names[&field.name] == property) {
            property.push('_');
        }
        extra = format!(
            "    [JsonExtensionData]\n    public Dictionary<string, JsonElement>? {property} {{\n  get;\n  init;\n\n}}\n\n"
        );
    }
    let units = properties
        .iter()
        .zip(fields)
        .map(|(source, field)| poolster_core::source_layout::SourceUnit {
            bytes: source.len() + if field.required { 0 } else { 128 },
            resource: None,
        })
        .collect::<Vec<_>>();
    let groups = poolster_core::source_layout::SourceLayout::default()
        .groups(&units, header.len() + extra.len() + 2)?;
    Ok(groups
        .iter()
        .enumerate()
        .map(|(part, indices)| {
            let mut source = header.clone();
            for index in indices {
                source.push_str(&properties[*index]);
            }
            if part == 0 {
                source.push_str(&extra);
            }
            source.push_str("}\n");
            let path = if part == 0 {
                filename.clone()
            } else {
                format!("{}.part{part:03}.cs", filename.trim_end_matches(".cs"))
            };
            (path, source)
        })
        .collect())
}

pub(crate) fn render_schema_with_policy(output: &mut String, schema: &Schema, open_enums: bool) {
    let name = pascal_case(&schema.name);
    match &schema.value.kind {
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            let names =
                native_names::field_names(fields, pascal_case, &[&name, "EqualityContract"]);
            let _ = writeln!(output, "public sealed record {name}");
            output.push_str("{\n");
            for field in fields {
                output.push_str(&render_object_property(field, &names[&field.name]));
            }
            if !matches!(additional_properties, AdditionalProperties::Forbidden) {
                let mut property = "AdditionalProperties".to_owned();
                while fields.iter().any(|field| names[&field.name] == property) {
                    property.push('_');
                }
                let _ = writeln!(
                    output,
                    "    [JsonExtensionData]\n    public Dictionary<string, JsonElement>? {property} {{\n  get;\n  init;\n\n}}"
                );
            }
            output.push_str("}\n");
        }
        SchemaKind::String if open_enums && !schema.value.enum_values.is_empty() => {
            let _ = writeln!(
                output,
                "[JsonConverter(typeof({name}JsonConverter))]\npublic sealed record {name}(string Value)\n{{\n  "
            );
            let mut members = std::collections::BTreeSet::from([
                name.clone(),
                "Value".into(),
                "Equals".into(),
                "GetHashCode".into(),
                "ToString".into(),
                "EqualityContract".into(),
            ]);
            for (index, value) in schema.value.enum_values.iter().enumerate() {
                let Some(value) = value.as_str() else {
                    continue;
                };
                let mut member = enum_member_name(value, index);
                while !members.insert(member.clone()) {
                    member.push('_');
                }
                let _ = writeln!(
                    output,
                    "    public static {name} {member} {{\n  get;\n\n}}\n= new({value:?});\n"
                );
            }
            let _ = writeln!(
                output,
                "\n}}\n\npublic sealed class {name}JsonConverter : JsonConverter<{name}>\n{{\npublic override {name} Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)\n        => reader.TokenType == JsonTokenType.String ? new(reader.GetString()!) : throw new JsonException(\"Expected string enum value\");\n\n    public override void Write(Utf8JsonWriter writer, {name} value, JsonSerializerOptions options)\n        => writer.WriteStringValue(value.Value);\n\n}}"
            );
        }
        SchemaKind::String if !schema.value.enum_values.is_empty() => {
            output.push_str("[JsonConverter(typeof(JsonStringEnumConverter))]\n");
            let _ = writeln!(output, "public enum {name}");
            output.push_str("{\n");
            let mut members = std::collections::BTreeSet::from([name.clone()]);
            for (index, value) in schema.value.enum_values.iter().enumerate() {
                let value = value.as_str().unwrap_or_default();
                let mut member = enum_member_name(value, index);
                while !members.insert(member.clone()) {
                    member.push('_');
                }
                let _ = writeln!(output, "    {member},");
            }
            output.push_str("}\n");
        }
        _ => {
            // C# has no public type aliases. A value record retains the named
            // schema in the generated API instead of silently erasing it.
            let value_type = csharp_type(&schema.value, false);
            let _ = writeln!(
                output,
                "[JsonConverter(typeof({name}JsonConverter))]\npublic sealed record {name}([property: JsonPropertyName(\"value\")] {value_type} Value);\n\n\npublic sealed class {name}JsonConverter : JsonConverter<{name}>\n{{\n  public override bool HandleNull => true;\n  \n    public override {name} Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)\n        => new(JsonSerializer.Deserialize<{value_type}>(ref reader, options)!);\n  \n    public override void Write(Utf8JsonWriter writer, {name} value, JsonSerializerOptions options)\n    {{\n    if (value is null) {{\n      writer.WriteNullValue();\n      return;\n\n    }}\n    \n        JsonSerializer.Serialize(writer, value.Value, options);\n    \n\n  }}\n  \n}}"
            );
        }
    }
}
