use super::*;

pub(super) fn identifier(value: &str) -> Result<()> {
    const RESERVED: &[&str] = &[
        "default",
        "class",
        "function",
        "var",
        "let",
        "const",
        "import",
        "export",
        "return",
        "new",
        "delete",
        "in",
        "instanceof",
        "typeof",
        "void",
        "await",
        "yield",
        "enum",
        "extends",
        "implements",
        "interface",
        "package",
        "private",
        "protected",
        "public",
        "static",
        "null",
        "true",
        "false",
        "this",
        "super",
        "switch",
        "case",
        "break",
        "continue",
        "throw",
        "try",
        "catch",
        "finally",
        "while",
        "do",
        "for",
        "if",
        "else",
        "with",
        "debugger",
        "eval",
        "arguments",
        "any",
        "string",
        "number",
        "boolean",
        "unknown",
        "never",
        "object",
        "symbol",
        "bigint",
        "undefined",
    ];
    if value.is_empty()
        || !value
            .chars()
            .enumerate()
            .all(|(i, c)| c.is_ascii_alphabetic() || c == '_' || (i > 0 && c.is_ascii_digit()))
        || RESERVED.contains(&value)
    {
        bail!("GraphQL name {value:?} cannot be emitted as a TypeScript identifier");
    }
    Ok(())
}
pub(super) fn validate_type(ty: &ModelType, contract: &GraphqlOperations) -> Result<()> {
    match &ty.kind {
        ModelKind::Named(name) => {
            identifier(name)?;
            if !contract.input_objects.contains_key(name) {
                bail!("GraphQL contract references missing input type {name}");
            }
        }
        ModelKind::Object(fields) => {
            let mut names = std::collections::BTreeSet::new();
            for field in fields {
                if !names.insert(&field.name) {
                    bail!("GraphQL contract contains duplicate field {}", field.name);
                }
                validate_type(&field.ty, contract)?;
            }
        }
        ModelKind::List(item) => validate_type(item, contract)?,
        ModelKind::Union(items) => {
            for item in items {
                validate_type(item, contract)?;
            }
        }
        ModelKind::Enum(items) if items.is_empty() => bail!("GraphQL contract contains empty enum"),
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn substituted_models_reject_unsafe_names_and_missing_refs() {
        let contract = GraphqlOperations {
            schema_source: String::new(),
            operation_source: String::new(),
            operations: vec![],
            input_objects: Default::default(),
        };
        for name in ["Missing", "Injected; export const bad = 1"] {
            let ty = ModelType {
                nullable: false,
                kind: ModelKind::Named(name.into()),
            };
            assert!(validate_type(&ty, &contract).is_err());
        }
        assert!(
            validate_type(
                &ModelType {
                    nullable: false,
                    kind: ModelKind::Enum(vec![])
                },
                &contract
            )
            .is_err()
        );
        assert_eq!(
            render_type(
                &ModelType {
                    nullable: false,
                    kind: ModelKind::Union(vec![])
                },
                &BTreeMap::new(),
                false
            ),
            "never"
        );
    }
}
