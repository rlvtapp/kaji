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
        source = format!(
            "import json\nVALUE=json.loads({})\n",
            serde_json::to_string(&json)?
        );
    } else {
        let mut expression = Vec::new();
        match value {
            serde_json::Value::Array(values) => {
                for (index, chunk) in values.chunks(20).enumerate() {
                    let child = format!("{name}_{index}");
                    emit(&serde_json::Value::Array(chunk.to_vec()), &child, files)?;
                    writeln!(source, "from .{child} import VALUE as _part{index}")?;
                    expression.push(format!("*_part{index}"));
                }
                writeln!(source, "VALUE=[{}]", expression.join(","))?;
            }
            serde_json::Value::Object(values) => {
                for (index, (key, value)) in values.iter().enumerate() {
                    let child = format!("{name}_{index}");
                    emit(value, &child, files)?;
                    writeln!(source, "from .{child} import VALUE as _part{index}")?;
                    expression.push(format!("{}:_part{index}", serde_json::to_string(key)?));
                }
                writeln!(source, "VALUE={{{}}}", expression.join(","))?;
            }
            _ => {
                source = format!(
                    "import json\nVALUE=json.loads({})\n",
                    serde_json::to_string(&json)?
                )
            }
        }
    }
    files.insert(format!("_codec_inputs/{name}.py"), source);
    Ok(())
}
