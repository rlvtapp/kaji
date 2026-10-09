use super::ident;
use anyhow::{Result, bail, ensure};
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
        let mut body = format!("public sealed record {name}\n{{\n");
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
                "public {required}{target} {property} {{ get; init; }}"
            )
            .unwrap();
            body.push_str(&property_source);
            properties.push(property_source);
        }
        body.push_str("}\n");
        self.source.push_str(&body);
        let header = format!("public sealed partial record {name}\n{{\n");
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
            ModelKind::Union(_) => {
                bail!("C# GraphQL abstract union selections are not supported yet")
            }
        };
        Ok(if ty.nullable {
            format!("{base}?")
        } else {
            base
        })
    }
}
