use super::ident;
use anyhow::{Result, ensure};
use poolster_core::native::{ModelField, ModelKind, ModelType};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};
#[derive(Default)]
pub(super) struct Models {
    pub source: String,
    pub files: BTreeMap<String, String>,
    names: BTreeSet<String>,
}
impl Models {
    pub fn object(&mut self, name: &str, fields: &[ModelField]) -> Result<()> {
        self.object_with_base(name, fields, None)
    }
    fn object_with_base(
        &mut self,
        name: &str,
        fields: &[ModelField],
        parent: Option<&str>,
    ) -> Result<()> {
        ensure!(
            ![
                "GraphqlClient",
                "GraphqlOperations",
                "GraphqlError",
                "GraphqlResponse",
                "GraphqlException",
                "Optional",
                "OptionalConverter",
                "OptionalConverterFactory"
            ]
            .contains(&name),
            "GraphQL type conflicts with runtime {name}"
        );
        ensure!(
            self.names.insert(name.into()),
            "GraphQL type naming collision {name}"
        );
        let inheritance = parent.map(|p| format!(" : {p}")).unwrap_or_default();
        let mut body = format!("public sealed record {name}{inheritance}\n{{\n  ");
        let mut names = BTreeSet::new();
        let mut properties = Vec::new();
        for field in fields {
            let property = ident(&field.name);
            ensure!(
                property != name && names.insert(property.clone()),
                "GraphQL field naming collision {property}"
            );
            let target = self.ty(&format!("{name}{property}"), &field.ty)?;
            let target = if field.optional {
                format!("Optional<{target}>")
            } else {
                target
            };
            let mut property_source = String::new();
            writeln!(
                property_source,
                "[JsonPropertyName({})]",
                serde_json::to_string(&field.name)?
            )
            .unwrap();
            if field.optional {
                property_source
                    .push_str("[JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingDefault)]\n");
            }
            let required = if field.optional { "" } else { "required " };
            writeln!(
                property_source,
                "public {required}{target} {property} {{\n  get;\n  init;\n\n}}"
            )
            .unwrap();
            body.push_str(&property_source);
            properties.push(property_source);
        }
        body.push_str("}\n");
        self.source.push_str(&body);
        let header = format!("public sealed partial record {name}{inheritance}\n{{\n  ");
        let units = properties
            .iter()
            .map(|p| poolster_core::source_layout::SourceUnit {
                bytes: p.len(),
                resource: None,
            })
            .collect::<Vec<_>>();
        let mut groups = poolster_core::source_layout::SourceLayout::default()
            .groups(&units, header.len() + 1024)?;
        if groups.is_empty() {
            groups.push(vec![]);
        }
        for (part, group) in groups.iter().enumerate() {
            let source = header.clone()
                + &group
                    .iter()
                    .map(|i| properties[*i].as_str())
                    .collect::<String>()
                + "}\n";
            let filename = crate::bounded_filename(name, 0, "cs");
            self.files.insert(
                format!(
                    "Models/{}.part{part:03}.cs",
                    filename.trim_end_matches(".cs")
                ),
                source,
            );
        }
        Ok(())
    }
    fn union(&mut self, name: &str, alternatives: &[ModelType]) -> Result<()> {
        ensure!(
            self.names.insert(name.into()) && self.names.insert(format!("{name}Converter")),
            "GraphQL type naming collision {name}"
        );
        let variants = union_variants(alternatives)?;
        let key = &variants[0].0;
        let mut read = String::new();
        let mut write = String::new();
        for (_, label, fields) in &variants {
            let variant = format!("{name}{}", ident(label));
            self.object_with_base(&variant, fields, Some(name))?;
            writeln!(read,"case {}: return JsonSerializer.Deserialize<{variant}>(document.RootElement.GetRawText(),options) ?? throw new JsonException(\"Null GraphQL alternative\");",serde_json::to_string(label)?).unwrap();
            writeln!(write,"case {variant} variant when variant.{} == {}: JsonSerializer.Serialize(writer,variant,options); return;",ident(key),serde_json::to_string(label)?).unwrap();
        }
        let body = format!(
            "[JsonConverter(typeof({name}Converter))]\npublic abstract record {name};\n\npublic sealed class {name}Converter : JsonConverter<{name}> {{\n  public override {name} Read(ref Utf8JsonReader reader,Type type,JsonSerializerOptions options) {{\n    using var document=JsonDocument.ParseValue(ref reader);\n    if(document.RootElement.ValueKind != JsonValueKind.Object || !document.RootElement.TryGetProperty({key:?},out var tag) || tag.ValueKind != JsonValueKind.String) throw new JsonException(\"Missing or invalid GraphQL typename\");\n    switch(tag.GetString()) {{\n      {read}default: throw new JsonException(\"Unknown GraphQL typename\");\n      \n}}\n\n  }}\n  \npublic override void Write(Utf8JsonWriter writer,{name} value,JsonSerializerOptions options) {{\n    switch(value) {{\n      {write}default: throw new JsonException(\"Invalid GraphQL typename or alternative\");\n      \n}}\n\n  }}\n  \n}}\n\n"
        );
        self.source.push_str(&body);
        self.files.insert(
            format!("Models/{}", crate::bounded_filename(name, 0, "cs")),
            body,
        );
        Ok(())
    }
    pub fn ty(&mut self, name: &str, ty: &ModelType) -> Result<String> {
        let base = match &ty.kind {
            ModelKind::Scalar(s) => match s.as_str() {
                "String" | "ID" => "string".into(),
                "Int" => "int".into(),
                "Float" => "double".into(),
                "Boolean" => "bool".into(),
                _ => "JsonElement".into(),
            },
            ModelKind::Named(s) => ident(s),
            ModelKind::Enum(_) | ModelKind::Literal(_) => "string".into(),
            ModelKind::List(item) => format!("List<{}>", self.ty(&format!("{name}Item"), item)?),
            ModelKind::Object(fields) => {
                self.object(name, fields)?;
                name.into()
            }
            ModelKind::Union(alternatives) => {
                self.union(name, alternatives)?;
                name.into()
            }
        };
        Ok(if ty.nullable {
            format!("{base}?")
        } else {
            base
        })
    }
}

fn union_variants(alternatives: &[ModelType]) -> Result<Vec<(String, String, Vec<ModelField>)>> {
    ensure!(
        !alternatives.is_empty(),
        "GraphQL union has no alternatives"
    );
    let mut values = BTreeSet::new();
    let mut key = None;
    alternatives
        .iter()
        .map(|ty| {
            let ModelKind::Object(fields) = &ty.kind else {
                anyhow::bail!("GraphQL union alternatives must be selected objects")
            };
            let (field, value) = fields
                .iter()
                .find_map(|f| {
                    if let ModelKind::Literal(v) = &f.ty.kind {
                        (!f.optional && !f.ty.nullable).then_some((f, v))
                    } else {
                        None
                    }
                })
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "GraphQL abstract selections require a nonoptional __typename discriminator"
                    )
                })?;
            ensure!(
                key.as_ref().is_none_or(|k| k == &field.name),
                "GraphQL union discriminator aliases must agree"
            );
            key = Some(field.name.clone());
            ensure!(
                values.insert(value.clone()),
                "duplicate GraphQL typename {value}"
            );
            Ok((field.name.clone(), value.clone(), fields.clone()))
        })
        .collect()
}
