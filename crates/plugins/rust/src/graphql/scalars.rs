//! Validated, self-contained serde wire types for custom GraphQL scalars.
use anyhow::{Result, ensure};
use poolster_core::native::{GraphqlOperations, ModelKind, ModelType};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Independent input/output JSON wire types, without runtime codecs or dependencies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphqlScalarMapping {
    pub input: String,
    pub output: String,
}
impl GraphqlScalarMapping {
    pub fn new(input: impl Into<String>, output: impl Into<String>) -> Self {
        Self {
            input: input.into(),
            output: output.into(),
        }
    }
}
pub(super) fn validate_mappings(
    mappings: &BTreeMap<String, GraphqlScalarMapping>,
    contract: &GraphqlOperations,
) -> Result<BTreeMap<String, GraphqlScalarMapping>> {
    fn collect<'a>(ty: &'a ModelType, used: &mut BTreeSet<&'a str>) {
        match &ty.kind {
            ModelKind::Scalar(name) => {
                used.insert(name);
            }
            ModelKind::Object(fields) => {
                for field in fields {
                    collect(&field.ty, used);
                }
            }
            ModelKind::List(item) => collect(item, used),
            ModelKind::Union(items) => {
                for item in items {
                    collect(item, used);
                }
            }
            _ => {}
        }
    }
    let mut used = BTreeSet::new();
    for fields in contract.input_objects.values() {
        for field in fields {
            collect(&field.ty, &mut used);
        }
    }
    for operation in &contract.operations {
        collect(&operation.result, &mut used);
        for field in &operation.variables {
            collect(&field.ty, &mut used);
        }
    }
    let mut normalized = BTreeMap::new();
    for (name, mapping) in mappings {
        ensure!(
            !name.is_empty()
                && name
                    .chars()
                    .enumerate()
                    .all(|(i, c)| c.is_ascii_alphabetic()
                        || c == '_'
                        || (i > 0 && c.is_ascii_digit())),
            "invalid GraphQL scalar mapping name {name:?}"
        );
        ensure!(
            !matches!(name.as_str(), "String" | "ID" | "Int" | "Float" | "Boolean"),
            "built-in GraphQL scalar {name} cannot be remapped"
        );
        ensure!(
            used.contains(name.as_str()),
            "GraphQL scalar mapping {name:?} is not used by the selected operations; check its spelling or remove the unused mapping"
        );
        let input = wire_type(&mapping.input)
            .map_err(|error| anyhow::anyhow!("GraphQL scalar {name} input mapping: {error}"))?;
        let output = wire_type(&mapping.output)
            .map_err(|error| anyhow::anyhow!("GraphQL scalar {name} output mapping: {error}"))?;
        normalized.insert(name.clone(), GraphqlScalarMapping::new(input, output));
    }
    Ok(normalized)
}
fn wire_type(source: &str) -> Result<String> {
    ensure!(
        !source.contains(['\n', '\r', '\0']),
        "expected a supported self-contained Rust wire type"
    );
    struct Parser<'a> {
        source: &'a str,
        at: usize,
    }
    impl Parser<'_> {
        fn whitespace(&mut self) {
            while self
                .source
                .as_bytes()
                .get(self.at)
                .is_some_and(u8::is_ascii_whitespace)
            {
                self.at += 1;
            }
        }
        fn take(&mut self, token: u8) -> bool {
            self.whitespace();
            if self.source.as_bytes().get(self.at) == Some(&token) {
                self.at += 1;
                true
            } else {
                false
            }
        }
        fn ty(&mut self, depth: usize) -> Result<String> {
            ensure!(depth < 32, "Rust scalar mapping nesting exceeds 32 levels");
            self.whitespace();
            let start = self.at;
            while self
                .source
                .as_bytes()
                .get(self.at)
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b':')
            {
                self.at += 1;
            }
            let name = &self.source[start..self.at];
            let bare = match name {
                "String" | "std::string::String" => Some("std::string::String"),
                "serde_json::Value" => Some("serde_json::Value"),
                "bool" | "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32"
                | "u64" | "u128" | "usize" | "f32" | "f64" => Some(name),
                _ => None,
            };
            if let Some(bare) = bare {
                return Ok(bare.into());
            }
            let container = match name {
                "Vec" | "std::vec::Vec" => "std::vec::Vec",
                "Option" | "std::option::Option" => "std::option::Option",
                "BTreeMap" | "std::collections::BTreeMap" => "std::collections::BTreeMap",
                _ => anyhow::bail!(
                    "unsupported Rust wire type {name:?}; use String, primitive numbers/bool, serde_json::Value, Vec<T>, Option<T> or BTreeMap<String, T>"
                ),
            };
            ensure!(self.take(b'<'), "expected generic arguments for {name}");
            let first = self.ty(depth + 1)?;
            let content = if container == "std::collections::BTreeMap" {
                ensure!(
                    first == "std::string::String",
                    "JSON object mapping keys must be String"
                );
                ensure!(self.take(b','), "BTreeMap requires String and a value type");
                format!("{first}, {}", self.ty(depth + 1)?)
            } else {
                first
            };
            ensure!(
                self.take(b'>'),
                "unbalanced or unsupported Rust generic arguments"
            );
            Ok(format!("{container}<{content}>"))
        }
    }
    let mut parser = Parser { source, at: 0 };
    let result = parser.ty(0)?;
    parser.whitespace();
    ensure!(
        parser.at == source.len(),
        "unexpected trailing Rust type tokens"
    );
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validated_types_are_self_contained_and_nested() {
        assert_eq!(
            wire_type(" Vec < BTreeMap<String, Option<i64>> > ").unwrap(),
            "std::vec::Vec<std::collections::BTreeMap<std::string::String, std::option::Option<i64>>>"
        );
        for invalid in [
            "",
            "DateTime",
            "chrono::DateTime<Utc>",
            "String; fn injected() {}",
            "Vec<String",
            "BTreeMap<i32, String>",
            "Option<String, bool>",
            "&str",
            "String\n",
        ] {
            assert!(wire_type(invalid).is_err(), "accepted {invalid:?}");
        }
    }
}
