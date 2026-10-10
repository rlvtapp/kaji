//! Bound descriptor fragments independently from generated model source.
use super::*;
pub(super) fn emit(
    value: &serde_json::Value,
    name: &str,
    files: &mut BTreeMap<String, String>,
) -> Result<()> {
    let json = serde_json::to_string(value)?;
    let mut source = String::new();
    if json.len() < 7000 {
        source = literal(value)?;
    } else {
        let mut expression = Vec::new();
        match value {
            serde_json::Value::Array(values) => {
                for (index, chunk) in values.chunks(20).enumerate() {
                    let child = format!("{name}_{index}");
                    emit(&serde_json::Value::Array(chunk.to_vec()), &child, files)?;
                    writeln!(source, "from .{child} import VALUE as _part{index}")?;
                    expression.push(format!("    *_part{index},"));
                }
                writeln!(source, "\nVALUE = [\n{}\n]", expression.join("\n"))?;
            }
            serde_json::Value::Object(values) => {
                for (index, (key, value)) in values.iter().enumerate() {
                    let child = format!("{name}_{index}");
                    emit(value, &child, files)?;
                    writeln!(source, "from .{child} import VALUE as _part{index}")?;
                    expression.push(format!(
                        "    {}: _part{index},",
                        serde_json::to_string(key)?
                    ));
                }
                writeln!(source, "\nVALUE = {{\n{}\n}}", expression.join("\n"))?;
            }
            _ => source = literal(value)?,
        }
    }
    files.insert(format!("_codec_inputs/{name}.py"), source);
    Ok(())
}

// Adjacent quoted lines preserve JSON exactly without an opaque 7,000-column literal.
fn literal(value: &serde_json::Value) -> Result<String> {
    let mut source = "import json\n\nVALUE = json.loads(\n".to_owned();
    for line in serde_json::to_string_pretty(value)?.lines() {
        writeln!(
            source,
            "    {}",
            serde_json::to_string(&format!("{line}\n"))?
        )?;
    }
    source.push_str(")\n");
    Ok(source)
}
