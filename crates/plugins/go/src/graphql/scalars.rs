use anyhow::{Result, ensure};
use std::collections::BTreeMap;
#[derive(Clone, Debug)]
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
pub(super) fn validate(mappings: &BTreeMap<String, GraphqlScalarMapping>) -> Result<()> {
    for (name, mapping) in mappings {
        ensure!(
            !matches!(name.as_str(), "String" | "ID" | "Int" | "Float" | "Boolean"),
            "built-in GraphQL scalar cannot be remapped"
        );
        for ty in [&mapping.input, &mapping.output] {
            ensure!(
                matches!(
                    ty.as_str(),
                    "string"
                        | "bool"
                        | "int"
                        | "int32"
                        | "int64"
                        | "uint64"
                        | "float32"
                        | "float64"
                        | "json.RawMessage"
                        | "time.Time"
                ),
                "unsupported self-contained Go GraphQL scalar mapping {ty:?}"
            );
        }
    }
    Ok(())
}
