//! Explicit application-type mappings; runtime callbacks provide optional conversion.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// TypeScript application types for a custom GraphQL scalar's independent directions.
/// Mappings do not install conversion callbacks. Supply runtime scalar codecs when
/// application types differ from wire JSON; otherwise use wire-compatible mappings.
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
    pub(super) fn validate(&self, name: &str) -> Result<()> {
        ensure!(
            name.chars()
                .enumerate()
                .all(|(i, c)| c.is_ascii_alphabetic() || c == '_' || (i > 0 && c.is_ascii_digit()))
                && !name.is_empty(),
            "invalid GraphQL scalar mapping name {name:?}"
        );
        ensure!(
            !matches!(name, "Int" | "Float" | "String" | "ID" | "Boolean"),
            "built-in GraphQL scalar {name} cannot be remapped"
        );
        validate_expression(&self.input, name, "input")?;
        validate_expression(&self.output, name, "output")
    }
}
pub(crate) fn validate_mappings(
    mappings: &std::collections::BTreeMap<String, GraphqlScalarMapping>,
    contract: &poolster_core::native::GraphqlOperations,
) -> Result<()> {
    use poolster_core::native::{ModelKind, ModelType};
    fn collect<'a>(ty: &'a ModelType, used: &mut std::collections::BTreeSet<&'a str>) {
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
    let mut used = std::collections::BTreeSet::new();
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
    for (name, mapping) in mappings {
        mapping.validate(name)?;
        ensure!(
            used.contains(name.as_str()),
            "GraphQL scalar mapping {name:?} is not used by the selected operations; check its spelling or remove the unused mapping"
        );
    }
    Ok(())
}

// Check expression boundaries without pretending to implement the TypeScript
// grammar. The generated package compiler remains the syntax/type authority.
fn validate_expression(value: &str, name: &str, direction: &str) -> Result<()> {
    let error = || {
        format!(
            "GraphQL scalar {name} {direction} mapping must be a nonempty, balanced single-line TypeScript type expression"
        )
    };
    ensure!(
        !value.trim().is_empty() && !value.contains(['\n', '\r', '\0']),
        "{}",
        error()
    );
    let mut brackets = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    let mut previous = None;
    for c in value.chars() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
        } else {
            ensure!(
                !(previous == Some('/') && matches!(c, '/' | '*')),
                "{}",
                error()
            );
            match c {
                '\'' | '"' | '`' => quote = Some(c),
                '(' | '[' | '{' => brackets.push(c),
                ')' | ']' | '}' => {
                    let expected = match c {
                        ')' => '(',
                        ']' => '[',
                        _ => '{',
                    };
                    ensure!(brackets.pop() == Some(expected), "{}", error());
                }
                ';' => ensure!(brackets.contains(&'{'), "{}", error()),
                _ => {}
            }
        }
        previous = Some(c);
    }
    ensure!(quote.is_none() && brackets.is_empty(), "{}", error());
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mappings_validate_boundary_and_keep_directions_explicit() {
        for value in [
            "",
            "string; export const injected = true",
            "string) ; console.log(1)",
            "string\nnumber",
            "string /* tail",
        ] {
            assert!(
                GraphqlScalarMapping::new(value, "string")
                    .validate("DateTime")
                    .is_err()
            );
        }
        for value in [
            "string",
            "string & { readonly __brand: 'DateTime'; }",
            "{ [key: string]: unknown }",
            "import('./scalar-types.js').DateValue",
            "Array<string | number>",
        ] {
            GraphqlScalarMapping::new(value, value)
                .validate("DateTime")
                .unwrap();
        }
        assert!(
            GraphqlScalarMapping::new("string", "string")
                .validate("Int")
                .is_err()
        );
        assert!(serde_json::from_str::<GraphqlScalarMapping>(r#"{"input":"string"}"#).is_err());
        assert!(
            serde_json::from_str::<GraphqlScalarMapping>(
                r#"{"input":"string","output":"string","codec":"date"}"#
            )
            .is_err()
        );
    }
}
