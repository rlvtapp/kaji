use anyhow::{Result, ensure};
/// Split generated top-level model declarations, respecting braces in Java strings.
pub(super) fn declarations(source: &str) -> Result<Vec<(String, String)>> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut depth = 0;
    let mut quoted = false;
    let mut escaped = false;
    let mut opened = false;
    for (i, c) in source.char_indices() {
        if quoted {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                quoted = false;
            }
            continue;
        }
        if c == '"' {
            quoted = true;
        } else if c == '{' {
            depth += 1;
            opened = true;
        } else if c == '}' {
            ensure!(depth > 0, "Invalid generated Java declaration");
            depth -= 1;
            if depth == 0 && opened {
                let text = source[start..=i].trim().to_owned();
                let tokens = text.split_whitespace().collect::<Vec<_>>();
                let token = if tokens.get(1) == Some(&"record") {
                    tokens.get(2)
                } else {
                    tokens.get(3)
                }
                .ok_or_else(|| anyhow::anyhow!("Invalid generated Java model"))?;
                let name = token.split('(').next().unwrap().to_owned();
                result.push((name, text));
                start = i + 1;
                opened = false;
            }
        }
    }
    ensure!(depth == 0 && !quoted, "Unclosed generated Java declaration");
    Ok(result)
}
