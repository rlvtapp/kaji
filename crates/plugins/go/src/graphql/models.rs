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
    pub fn named_type(&mut self, name: &str, ty: &ModelType, input: bool) -> Result<()> {
        if let ModelKind::Object(fields) = &ty.kind {
            self.object(name, fields, input)
        } else {
            let target = self.ty(name, ty, input)?;
            ensure!(
                self.names.insert(name.into()),
                "GraphQL type naming collision {name}"
            );
            writeln!(self.source, "type {name} = {target}").unwrap();
            Ok(())
        }
    }
    pub fn object(&mut self, name: &str, fields: &[ModelField], input: bool) -> Result<()> {
        ensure!(
            ![
                "Client",
                "GraphQLError",
                "GraphQLErrors",
                "GraphQLResponse",
                "HTTPError",
                "Optional"
            ]
            .contains(&name),
            "GraphQL type conflicts with runtime {name}"
        );
        ensure!(
            self.names.insert(name.into()),
            "GraphQL type naming collision {name}"
        );
        let mut body = format!("type {name} struct {{\n");
        let mut marshal =
            format!("func (v {name}) MarshalJSON() ([]byte,error) {{ fields:=map[string]any{{}}\n");
        let mut names = BTreeSet::new();
        for field in fields {
            let field_name = ident(&field.name);
            ensure!(
                names.insert(field_name.clone()),
                "GraphQL field naming collision {field_name}"
            );
            let mut ty = field.ty.clone();
            if field.optional {
                ty.nullable = false
            };
            let target = self.ty(&format!("{name}{field_name}"), &ty, input)?;
            let target = if field.optional {
                format!("Optional[{target}]")
            } else {
                target
            };
            let json = serde_json::to_string(&field.name)?;
            writeln!(body, "{field_name} {target} `json:{json}`").unwrap();
            if field.optional {
                writeln!(
                    marshal,
                    "if v.{field_name}.Set {{ fields[{json}]=v.{field_name}.Value }}"
                )
                .unwrap()
            } else {
                writeln!(marshal, "fields[{json}]=v.{field_name}").unwrap()
            }
        }
        body.push_str("}\n");
        self.source.push_str(&body);
        if input {
            marshal.push_str("return json.Marshal(fields)\n}\n");
            self.source.push_str(&marshal)
        }
        Ok(())
    }
    fn ty(&mut self, name: &str, ty: &ModelType, input: bool) -> Result<String> {
        let base = match &ty.kind {
            ModelKind::Scalar(s) => match s.as_str() {
                "String" | "ID" => "string".into(),
                "Int" => "int32".into(),
                "Float" => "float64".into(),
                "Boolean" => "bool".into(),
                _ => "json.RawMessage".into(),
            },
            ModelKind::Named(s) => ident(s),
            ModelKind::Enum(_) | ModelKind::Literal(_) => "string".into(),
            ModelKind::List(item) => format!("[]{}", self.ty(&format!("{name}Item"), item, input)?),
            ModelKind::Object(fields) => {
                self.object(name, fields, input)?;
                name.into()
            }
            ModelKind::Union(_) => {
                bail!("Go GraphQL abstract union selections are not supported yet")
            }
        };
        Ok(if ty.nullable {
            format!("*{base}")
        } else {
            base
        })
    }
}
