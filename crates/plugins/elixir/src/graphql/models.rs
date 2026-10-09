use anyhow::{Result, bail, ensure};
use poolster_core::native::{ModelField, ModelKind, ModelType};
use std::{collections::BTreeSet, fmt::Write};
pub(super) fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .enumerate()
            .all(|(i, c)| c == '_' || c.is_ascii_lowercase() || (i > 0 && c.is_ascii_digit()))
        && ![
            "def",
            "defmodule",
            "do",
            "end",
            "fn",
            "when",
            "and",
            "or",
            "not",
            "in",
            "true",
            "false",
            "nil",
            "else",
            "rescue",
            "catch",
            "after",
            "receive",
            "case",
            "cond",
            "try",
            "with",
            "for",
            "__struct__",
        ]
        .contains(&s)
}
pub(super) struct Models {
    pub source: String,
    module: String,
    names: BTreeSet<String>,
}
impl Models {
    pub fn new(module: &str) -> Self {
        Self {
            source: String::new(),
            module: module.into(),
            names: BTreeSet::new(),
        }
    }
    fn reserve(&mut self, name: &str) -> Result<String> {
        let name = crate::pascal_case(name);
        ensure!(
            !name.is_empty() && self.names.insert(name.clone()),
            "Elixir GraphQL model collision: {name}"
        );
        Ok(format!("{}.Models.{name}", self.module))
    }
    pub fn input(&mut self, name: &str, fields: &[ModelField]) -> Result<String> {
        self.record(name, fields, true)
    }
    fn record(&mut self, name: &str, fields: &[ModelField], input: bool) -> Result<String> {
        let full = self.reserve(name)?;
        let runtime = format!("{}.Runtime", self.module);
        let mut defaults = Vec::new();
        let mut types = Vec::new();
        let mut keys = Vec::new();
        let mut decode = Vec::new();
        let mut encode = String::new();
        let mut seen = BTreeSet::new();
        for field in fields {
            let id = crate::elixir_identifier(&field.name);
            ensure!(
                identifier(&id) && seen.insert(id.clone()),
                "Invalid/colliding Elixir GraphQL field {}",
                field.name
            );
            let nested = format!("{name}{}", crate::pascal_case(&field.name));
            let ty = if input {
                self.input_ty(&field.ty, &nested)?
            } else {
                self.ty(&field.ty, &nested)?
            };
            defaults.push(format!("{id}: :poolster_absent"));
            types.push(format!(
                "{id}: {}",
                if field.optional {
                    format!("{ty} | :poolster_absent")
                } else {
                    ty
                }
            ));
            if !field.optional {
                keys.push(format!(":{id}"));
            }
            let wire = super::elixir_string(&field.name);
            if input {
                let value = if field.ty.nullable {
                    format!("{runtime}.input(value.{id})")
                } else {
                    format!("{runtime}.required(value.{id})")
                };
                writeln!(
                    encode,
                    "    result=if value.{id}==:poolster_absent, do: result, else: Map.put(result,{wire},{value})"
                )?;
                if !field.optional {
                    writeln!(
                        encode,
                        "    if value.{id}==:poolster_absent, do: raise(ArgumentError,\"Missing required GraphQL variable\")"
                    )?;
                }
            } else {
                let expr = self.decode(
                    &field.ty,
                    &nested,
                    &format!("{runtime}.field(value,{wire})"),
                    0,
                )?;
                decode.push(format!(
                    "{id}: {}",
                    if field.optional {
                        format!("if(Map.has_key?(value,{wire}),do: {expr},else: :poolster_absent)")
                    } else {
                        expr
                    }
                ));
            }
        }
        writeln!(
            self.source,
            "defmodule {full} do\n @enforce_keys [{}]\n defstruct [{}]\n @type t :: %__MODULE__{{{}}}",
            keys.join(","),
            defaults.join(","),
            types.join(",")
        )?;
        if input {
            writeln!(
                self.source,
                " def to_wire(%__MODULE__{{}}={}) do\n   result=%{{}}\n{encode}\n   result\n end",
                if fields.is_empty() { "_value" } else { "value" }
            )?;
        } else {
            writeln!(
                self.source,
                " def from_wire(value) when is_map(value),do: %__MODULE__{{{}}}\n def from_wire(_value),do: raise(ArgumentError,\"Expected GraphQL object\")",
                decode.join(",")
            )?;
        }
        self.source.push_str("end\n");
        Ok(full)
    }
    fn input_ty(&mut self, t: &ModelType, name: &str) -> Result<String> {
        let base = match &t.kind {
            ModelKind::Named(name) => {
                format!("{}.Models.{}.t()", self.module, crate::pascal_case(name))
            }
            ModelKind::Object(fields) => format!("{}.t()", self.input(name, fields)?),
            ModelKind::List(inner) => format!("[{}]", self.input_ty(inner, name)?),
            ModelKind::Union(_) => bail!("GraphQL input union unsupported"),
            _ => self.scalar(t)?,
        };
        Ok(if t.nullable {
            format!("{base} | nil")
        } else {
            base
        })
    }
    fn scalar(&self, t: &ModelType) -> Result<String> {
        Ok(match &t.kind {
            ModelKind::Scalar(name) => match name.as_str() {
                "ID" | "String" => "String.t()",
                "Int" => "integer()",
                "Float" => "number()",
                "Boolean" => "boolean()",
                _ => "term()",
            }
            .into(),
            ModelKind::Literal(_) | ModelKind::Enum(_) => "String.t()".into(),
            _ => bail!("Unresolved GraphQL selection type"),
        })
    }
    pub fn ty(&mut self, t: &ModelType, name: &str) -> Result<String> {
        let base = match &t.kind {
            ModelKind::Object(fields) => format!("{}.t()", self.record(name, fields, false)?),
            ModelKind::Union(variants) => {
                let full = self.reserve(name)?;
                let mut types = Vec::new();
                let mut cases = String::new();
                for (i, v) in variants.iter().enumerate() {
                    let fields = match &v.kind {
                        ModelKind::Object(f) => f,
                        _ => bail!("Abstract GraphQL selections require object variants"),
                    };
                    let tag = fields
                        .iter()
                        .find_map(|f| {
                            if f.name == "__typename" {
                                if let ModelKind::Literal(v) = &f.ty.kind {
                                    Some(v)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "Abstract GraphQL selections require selected __typename"
                            )
                        })?;
                    let variant = format!("{name}Variant{i}");
                    let ty = self.ty(v, &variant)?;
                    types.push(ty);
                    writeln!(
                        cases,
                        "     {} -> {}.Models.{}.from_wire(value)",
                        super::elixir_string(tag),
                        self.module,
                        crate::pascal_case(&variant)
                    )?;
                }
                writeln!(
                    self.source,
                    "defmodule {full} do\n @type t :: {}\n def from_wire(value) do\n  case {}.Runtime.field(value,\"__typename\") do\n{cases}\n _ -> raise(ArgumentError,\"Unknown GraphQL typename\")\n end\n end\nend",
                    types.join(" | "),
                    self.module
                )?;
                format!("{full}.t()")
            }
            ModelKind::List(inner) => format!("[{}]", self.ty(inner, name)?),
            ModelKind::Named(_) => bail!("Unresolved named GraphQL result"),
            _ => self.scalar(t)?,
        };
        Ok(if t.nullable {
            format!("{base} | nil")
        } else {
            base
        })
    }
    fn decode(&self, t: &ModelType, name: &str, value: &str, depth: usize) -> Result<String> {
        let rt = format!("{}.Runtime", self.module);
        let base = match &t.kind {
            ModelKind::Object(_) | ModelKind::Union(_) => format!(
                "{}.Models.{}.from_wire({value})",
                self.module,
                crate::pascal_case(name)
            ),
            ModelKind::List(inner) => format!(
                "{rt}.list({value},fn item{depth} -> {} end)",
                self.decode(inner, name, &format!("item{depth}"), depth + 1)?
            ),
            ModelKind::Scalar(name) => format!(
                "{rt}.scalar({value},:{})",
                match name.as_str() {
                    "String" | "ID" => "string",
                    "Int" => "int",
                    "Float" => "float",
                    "Boolean" => "bool",
                    _ => "custom",
                }
            ),
            ModelKind::Enum(values) => format!(
                "{rt}.enum({value},[{}])",
                values
                    .iter()
                    .map(|v| super::elixir_string(v))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            ModelKind::Literal(v) => format!("{rt}.enum({value},[{}])", super::elixir_string(v)),
            _ => bail!("Unresolved GraphQL result"),
        };
        Ok(if t.nullable {
            format!(
                "{rt}.nullable({value},fn item -> {} end)",
                base.replace(value, "item")
            )
        } else {
            base
        })
    }
}
