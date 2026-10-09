use super::{ident, member};
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
        let mut body = format!("public final class {name}: Codable {{\n");
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
                writeln!(decode,"if !c.contains(.{property}) {{ self.{property} = .omitted }} else if try c.decodeNil(forKey:.{property}) {{ {decode_null} }} else {{ self.{property} = .value(try c.decode({target}.self,forKey:.{property})) }}").unwrap();
                writeln!(encode,"switch self.{property} {{ case .omitted: break; case .null: {encode_null}; case .value(let value): try c.encode(value,forKey:.{property}) }}").unwrap();
            } else {
                writeln!(
                    decode,
                    "self.{property} = try c.decode({target}.self,forKey:.{property})"
                )
                .unwrap();
                writeln!(encode, "try c.encode(self.{property},forKey:.{property})").unwrap();
            }
        }
        writeln!(body, "public init({}) {{\n{init}}}", parameters.join(", ")).unwrap();
        if fields.is_empty() {
            body.push_str("public init(from decoder: Decoder) throws {}\npublic func encode(to encoder: Encoder) throws { _ = encoder.container(keyedBy: EmptyCodingKeys.self) }\nprivate enum EmptyCodingKeys: String, CodingKey { case unused }\n");
        } else {
            writeln!(body,"enum CodingKeys: String, CodingKey {{\n{keys}}}\npublic init(from decoder: Decoder) throws {{ let c = try decoder.container(keyedBy:CodingKeys.self)\n{decode}}}\npublic func encode(to encoder: Encoder) throws {{ var c = encoder.container(keyedBy:CodingKeys.self)\n{encode}}}").unwrap();
        }
        body.push_str("}\n");
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
            ModelKind::Union(_) => {
                bail!("Swift GraphQL abstract union selections are not supported yet")
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
