use super::{ident, member};
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
        ensure!(
            ![
                "GraphqlClient",
                "GraphqlJSON",
                "GraphqlField",
                "GraphqlResponse",
                "GraphqlError",
                "GraphqlFailure",
                "GraphqlHTTPError",
                "GraphqlRequest",
                "GraphqlEmptyVariables"
            ]
            .contains(&name),
            "GraphQL type conflicts with runtime {name}"
        );
        ensure!(
            self.names.insert(name.into()),
            "GraphQL type naming collision {name}"
        );
        let mut body = format!("public final class {name}: Codable {{\n  ");
        let mut keys = String::new();
        let mut parameters = Vec::new();
        let mut init = String::new();
        let mut decode = String::new();
        let mut encode = String::new();
        let mut names = BTreeSet::new();
        for field in fields {
            let property = member(&field.name);
            ensure!(
                names.insert(property.clone()),
                "GraphQL field naming collision {property}"
            );
            let mut ty = field.ty.clone();
            if field.optional {
                ty.nullable = false;
            }
            let target = self.ty(&format!("{name}{}", ident(&field.name)), &ty)?;
            let declaration = if field.optional {
                format!("GraphqlField<{target}>")
            } else {
                target.clone()
            };
            writeln!(body, "public let {property}: {declaration}").unwrap();
            parameters.push(format!(
                "{property}: {declaration}{}",
                if field.optional { " = .omitted" } else { "" }
            ));
            writeln!(init, "self.{property} = {property}").unwrap();
            writeln!(keys, "case {property} = {}", literal(&field.name)).unwrap();
            if field.optional {
                let decode_null = if field.ty.nullable {
                    format!("self.{property} = .null")
                } else {
                    format!(
                        "throw DecodingError.dataCorruptedError(forKey:.{property},in:c,debugDescription: \"Non-null GraphQL field cannot be null\")"
                    )
                };
                let encode_null = if field.ty.nullable {
                    format!("try c.encodeNil(forKey:.{property})")
                } else {
                    format!(
                        "throw EncodingError.invalidValue(self.{property},.init(codingPath:encoder.codingPath + [CodingKeys.{property}],debugDescription: \"Non-null GraphQL field cannot be null\"))"
                    )
                };
                writeln!(decode,"if !c.contains(.{property}) {{\n  self.{property} = .omitted\n}}else if try c.decodeNil(forKey:.{property}) {{\n  {decode_null}\n}}else {{\n  self.{property} = .value(try c.decode({target}.self,forKey:.{property}))\n}}").unwrap();
                writeln!(encode,"switch self.{property} {{\n  case .omitted: break;\n  case .null: {encode_null};\n  case .value(let value): try c.encode(value,forKey:.{property})\n}}").unwrap();
            } else {
                writeln!(
                    decode,
                    "self.{property} = try c.decode({target}.self,forKey:.{property})"
                )
                .unwrap();
                writeln!(encode, "try c.encode(self.{property},forKey:.{property})").unwrap();
            }
        }
        writeln!(
            body,
            "public init({}) {{\n  {init}\n}}",
            parameters.join(", ")
        )
        .unwrap();
        if fields.is_empty() {
            body.push_str("public init(from decoder: Decoder) throws {}\npublic func encode(to encoder: Encoder) throws { _ = encoder.container(keyedBy: EmptyCodingKeys.self) }\nprivate enum EmptyCodingKeys: String, CodingKey { case unused }\n");
        } else {
            writeln!(body,"enum CodingKeys: String, CodingKey {{\n  {keys}\n}}\npublic init(from decoder: Decoder) throws {{\n  let c = try decoder.container(keyedBy:CodingKeys.self)\n{decode}\n}}\npublic func encode(to encoder: Encoder) throws {{\n  var c = encoder.container(keyedBy:CodingKeys.self)\n{encode}\n}}").unwrap();
        }
        body.push_str("}\n");
        self.source.push_str(&body);
        self.files.insert(
            format!("Models/{}", super::filename(&format!("Model{name}"))),
            body,
        );
        Ok(())
    }
    fn union(&mut self, name: &str, alternatives: &[ModelType]) -> Result<()> {
        ensure!(
            self.names.insert(name.into()),
            "GraphQL type naming collision {name}"
        );
        let variants = union_variants(alternatives)?;
        let key = &variants[0].0;
        let mut body = format!("public enum {name}: Codable {{\n  ");
        let mut decode = String::new();
        let mut encode = String::new();
        for (_, label, fields) in &variants {
            let variant = format!("{name}{}", ident(label));
            let case = member(label);
            self.object(&variant, fields)?;
            writeln!(body, "case {case}({variant})").unwrap();
            writeln!(
                decode,
                "case {}: self = .{case}(try {variant}(from:decoder))",
                literal(label)
            )
            .unwrap();
            writeln!(encode,"case .{case}(let value): guard value.{} == {} else {{ throw EncodingError.invalidValue(value,.init(codingPath:encoder.codingPath,debugDescription: \"Invalid GraphQL typename\")) }}; try value.encode(to:encoder)",member(key),literal(label)).unwrap();
        }
        writeln!(body,"private enum CodingKeys: String, CodingKey {{\n  case typename = {}\n}}\n\npublic init(from decoder: Decoder) throws {{\n  let c = try decoder.container(keyedBy:CodingKeys.self);\n  let typename = try c.decode(String.self,forKey:.typename);\n  switch typename {{\n    {decode}default: throw DecodingError.dataCorruptedError(forKey:.typename,in:c,debugDescription: \"Unknown GraphQL typename\")\n}}\n\n}}\n\npublic func encode(to encoder: Encoder) throws {{\n  switch self {{\n    {encode\n  }}}\n}}\n\n}}",literal(key)).unwrap();
        self.source.push_str(&body);
        self.files.insert(
            format!("Models/{}", super::filename(&format!("Model{name}"))),
            body,
        );
        Ok(())
    }
    pub fn ty(&mut self, name: &str, ty: &ModelType) -> Result<String> {
        let base = match &ty.kind {
            ModelKind::Scalar(s) => match s.as_str() {
                "String" | "ID" => "String".into(),
                "Int" => "Int32".into(),
                "Float" => "Double".into(),
                "Boolean" => "Bool".into(),
                _ => "GraphqlJSON".into(),
            },
            ModelKind::Named(s) => ident(s),
            ModelKind::Enum(_) | ModelKind::Literal(_) => "String".into(),
            ModelKind::List(item) => format!("[{}]", self.ty(&format!("{name}Item"), item)?),
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
fn literal(s: &str) -> String {
    serde_json::to_string(s).unwrap()
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
