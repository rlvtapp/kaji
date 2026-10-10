use super::*;

pub(super) fn identifier(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && name
                .chars()
                .enumerate()
                .all(|(i, c)| c.is_ascii_alphabetic() || c == '_' || (i > 0 && c.is_ascii_digit())),
        "unsupported TypeScript event identifier {name}"
    );
    ensure!(
        ![
            "default",
            "function",
            "const",
            "let",
            "var",
            "class",
            "import",
            "export",
            "await",
            "yield",
            "return",
            "new",
            "delete",
            "in",
            "if",
            "else",
            "for",
            "while",
            "throw",
            "try",
            "catch",
            "switch",
            "case",
            "break",
            "continue",
            "this",
            "super",
            "null",
            "true",
            "false",
            "typeof",
            "void",
            "enum",
            "interface",
            "extends",
            "implements",
            "private",
            "protected",
            "public",
            "static"
        ]
        .contains(&name),
        "reserved TypeScript event identifier {name}"
    );
    Ok(())
}
