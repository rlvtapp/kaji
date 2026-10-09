use super::ident;
use anyhow::{Result, bail, ensure};
use poolster_core::native::{ModelField, ModelKind, ModelType};
use std::{collections::BTreeSet, fmt::Write};
#[derive(Default)]
pub(super) struct Models {
    pub source: String,
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
            writeln!(
                body,
                "[JsonPropertyName({})]",
                serde_json::to_string(&field.name)?
            )
            .unwrap();
            if field.optional {
                body.push_str("[JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingDefault)]\n");
            }
            let required = if field.optional { "" } else { "required " };
            writeln!(
                body,
                "public {required}{target} {property} {{ get; init; }}"
            )
            .unwrap();
        }
        body.push_str("}\n");
        self.source.push_str(&body);
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
