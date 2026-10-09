use anyhow::{Result, bail, ensure};
use poolster_core::native::{ModelField, ModelKind, ModelType};
use std::{collections::BTreeSet, fmt::Write};
#[derive(Default)]
pub(super) struct Models {
    pub source: String,
    names: BTreeSet<String>,
}
pub(super) fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s != "_"
        && s.chars()
            .enumerate()
            .all(|(i, c)| c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
        && ![
            "class",
            "interface",
            "record",
            "public",
            "private",
            "static",
            "void",
            "int",
            "float",
            "double",
            "boolean",
            "null",
            "true",
            "false",
            "new",
            "return",
            "switch",
            "case",
            "default",
            "package",
            "import",
            "var",
            "yield",
            "sealed",
            "permits",
            "extends",
            "implements",
            "final",
            "this",
            "super",
            "throw",
            "throws",
            "try",
            "catch",
            "finally",
            "for",
            "while",
            "do",
            "if",
            "else",
            "enum",
            "assert",
            "break",
            "continue",
            "instanceof",
            "long",
            "short",
            "byte",
            "char",
            "const",
            "goto",
            "abstract",
            "native",
            "synchronized",
            "transient",
            "volatile",
            "protected",
            "strictfp",
        ]
        .contains(&s)
}
impl Models {
    pub(super) fn reserve(&mut self, name: &str) -> Result<()> {
        ensure!(
            identifier(name)
                && ![
                    "Client",
                    "Field",
                    "Envelope",
                    "GraphqlError",
                    "Transport",
                    "HttpTransport",
                    "Input",
                    "Decoder",
                    "GraphqlException",
                    "String",
                    "Integer",
                    "Double",
                    "Boolean",
                    "JsonNode",
                    "ObjectNode",
                    "List",
                    "Map"
                ]
                .contains(&name),
            "Invalid/reserved GraphQL Java type {name}"
        );
        ensure!(
            name.len() <= 200,
            "GraphQL Java type {name} exceeds the portable filename limit (200 bytes)"
        );
        ensure!(
            self.names.insert(name.to_ascii_lowercase()),
            "GraphQL Java model collision on a case-insensitive filesystem: {name}"
        );
        Ok(())
    }
    pub fn input(&mut self, name: &str, fields: &[ModelField]) -> Result<()> {
        self.reserve(name)?;
        ensure!(
            fields.len() <= 250,
            "Java GraphQL record {name} exceeds the JVM constructor parameter limit"
        );
        let mut args = Vec::new();
        let mut body = String::new();
        let mut seen = BTreeSet::new();
        for f in fields {
            ensure!(
                identifier(&f.name)
                    && ![
                        "toJson",
                        "toString",
                        "hashCode",
                        "getClass",
                        "wait",
                        "notify",
                        "notifyAll"
                    ]
                    .contains(&f.name.as_str())
                    && seen.insert(f.name.clone()),
                "Invalid Java input field {}",
                f.name
            );
            let ty = self.input_ty(&f.ty, &format!("{name}{}", crate::type_name(&f.name)))?;
            args.push(format!(
                "{} {}",
                if f.optional {
                    format!("Field<{ty}>")
                } else {
                    ty
                },
                f.name
            ));
            let key = serde_json::to_string(&f.name)?;
            if f.optional {
                writeln!(
                    body,
                    "if ({}==null) throw new IllegalArgumentException(\"Use Field.absent() for omitted variables\"); if ({}.present()) {{ {} node.set({key},encode({}.value())); }}",
                    f.name,
                    f.name,
                    if !f.ty.nullable {
                        format!(
                            "if ({}.value()==null) throw new IllegalArgumentException(\"Non-null GraphQL input\");",
                            f.name
                        )
                    } else {
                        String::new()
                    },
                    f.name
                )?;
            } else {
                if !f.ty.nullable {
                    writeln!(
                        body,
                        "if ({}==null) throw new IllegalArgumentException(\"Non-null GraphQL input\");",
                        f.name
                    )?;
                }
                writeln!(body, "node.set({key},encode({}));", f.name)?;
            }
        }
        writeln!(
            self.source,
            "public record {name}({}) implements Input {{ public ObjectNode toJson() {{ObjectNode node=JSON.createObjectNode();{body}return node;}} }}",
            args.join(", ")
        )?;
        Ok(())
    }
    fn input_ty(&mut self, t: &ModelType, name: &str) -> Result<String> {
        match &t.kind {
            ModelKind::Named(n) => {
                ensure!(identifier(n), "Invalid GraphQL named type");
                Ok(n.clone())
            }
            ModelKind::Object(f) => {
                self.input(name, f)?;
                Ok(name.into())
            }
            ModelKind::List(inner) => {
                Ok(format!("java.util.List<{}>", self.input_ty(inner, name)?))
            }
            ModelKind::Union(_) => bail!("GraphQL input union unsupported"),
            _ => self.scalar(t),
        }
    }
    fn scalar(&self, t: &ModelType) -> Result<String> {
        Ok(match &t.kind {
            ModelKind::Scalar(s) => match s.as_str() {
                "String" | "ID" => "String",
                "Int" => "Integer",
                "Float" => "Double",
                "Boolean" => "Boolean",
                _ => "JsonNode",
            }
            .into(),
            ModelKind::Enum(_) | ModelKind::Literal(_) => "String".into(),
            _ => bail!("Unresolved GraphQL model"),
        })
    }
    pub fn ty(&mut self, t: &ModelType, name: &str) -> Result<String> {
        match &t.kind {
            ModelKind::Object(fields) => {
                self.reserve(name)?;
                ensure!(
                    fields.len() <= 250,
                    "Java GraphQL record {name} exceeds the JVM constructor parameter limit"
                );
                let mut args = Vec::new();
                let mut decode = Vec::new();
                let mut seen = BTreeSet::new();
                for f in fields {
                    ensure!(
                        identifier(&f.name)
                            && ![
                                "fromJson",
                                "toString",
                                "hashCode",
                                "getClass",
                                "wait",
                                "notify",
                                "notifyAll"
                            ]
                            .contains(&f.name.as_str())
                            && seen.insert(f.name.clone()),
                        "Invalid Java selected field {}",
                        f.name
                    );
                    let nested = format!("{name}{}", crate::type_name(&f.name));
                    let ty = self.ty(&f.ty, &nested)?;
                    let key = serde_json::to_string(&f.name)?;
                    args.push(format!(
                        "{} {}",
                        if f.optional {
                            format!("Field<{ty}>")
                        } else {
                            ty
                        },
                        f.name
                    ));
                    let expr = self.decode(&f.ty, &nested, &format!("field(node,{key})"))?;
                    decode.push(if f.optional {
                        format!("node.has({key})?Field.of({expr}):Field.absent()")
                    } else {
                        expr
                    });
                }
                writeln!(
                    self.source,
                    "public record {name}({}) {{ public static {name} fromJson(JsonNode node) {{if(!node.isObject())throw new IllegalArgumentException(\"Expected GraphQL object\");return new {name}({});}} }}",
                    args.join(", "),
                    decode.join(", ")
                )?;
                Ok(name.into())
            }
            ModelKind::Union(variants) => {
                self.reserve(name)?;
                let mut dispatch = String::new();
                let mut names = Vec::new();
                for (i, v) in variants.iter().enumerate() {
                    let variant = format!("{name}Variant{i}");
                    let fields = match &v.kind {
                        ModelKind::Object(f) => f,
                        _ => bail!("Java abstract selections require object variants"),
                    };
                    let tag = fields
                        .iter()
                        .find_map(|f| {
                            if f.name == "__typename" {
                                if let ModelKind::Literal(s) = &f.ty.kind {
                                    Some(s)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                        .ok_or_else(|| {
                            anyhow::anyhow!("Java abstract selections require selected __typename")
                        })?;
                    let n = self.ty(v, &variant)?;
                    let search = format!(") {{ public static {n} fromJson");
                    self.source = self.source.replace(
                        &search,
                        &format!(") implements {name} {{ public static {n} fromJson"),
                    );
                    writeln!(
                        dispatch,
                        "case {}: return {n}.fromJson(node);",
                        serde_json::to_string(tag)?
                    )?;
                    names.push(n);
                }
                writeln!(
                    self.source,
                    "public sealed interface {name} permits {} {{ static {name} fromJson(JsonNode node) {{ switch(text(field(node,\"__typename\"))) {{{dispatch}default: throw new IllegalArgumentException(\"Unknown GraphQL typename\");}} }} }}",
                    names.join(", ")
                )?;
                Ok(name.into())
            }
            ModelKind::List(inner) => Ok(format!("java.util.List<{}>", self.ty(inner, name)?)),
            ModelKind::Named(_) => bail!("Unresolved named GraphQL result"),
            _ => self.scalar(t),
        }
    }
    fn decode(&self, t: &ModelType, name: &str, node: &str) -> Result<String> {
        self.decode_at(t, name, node, 0)
    }
    fn decode_at(&self, t: &ModelType, name: &str, node: &str, depth: usize) -> Result<String> {
        let base = match &t.kind {
            ModelKind::Object(_) | ModelKind::Union(_) => format!("{name}.fromJson({node})"),
            ModelKind::List(inner) => {
                format!(
                    "list({node},item{depth} -> {})",
                    self.decode_at(inner, name, &format!("item{depth}"), depth + 1)?
                )
            }
            ModelKind::Scalar(s) => match s.as_str() {
                "ID" | "String" => format!("text({node})"),
                "Int" => format!("integer({node})"),
                "Float" => format!("number({node})"),
                "Boolean" => format!("bool({node})"),
                _ => format!("scalarNode({node})"),
            },
            ModelKind::Enum(values) => format!(
                "enumValue({node},java.util.List.of({}))",
                values
                    .iter()
                    .map(|v| serde_json::to_string(v).unwrap())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            ModelKind::Literal(v) => format!("literal({node},{})", serde_json::to_string(v)?),
            _ => bail!("Unresolved result"),
        };
        Ok(if t.nullable {
            format!("{node}.isNull()?null:{base}")
        } else {
            base
        })
    }
}
