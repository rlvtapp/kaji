use anyhow::{Result, ensure};
use poolster_core::{GeneratedFile, GeneratedTree};
use std::fmt::Write;
pub(super) fn modules(source: &str) -> Result<Vec<(String, String)>> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut name = None;
    for line in source.lines() {
        if let Some(header) = line.strip_prefix("defmodule ") {
            if let Some(old) = name.take() {
                result.push((old, std::mem::take(&mut current)));
            }
            let module = header
                .strip_suffix(" do")
                .ok_or_else(|| anyhow::anyhow!("Invalid generated Elixir module"))?;
            name = Some(module.into());
        }
        writeln!(current, "{line}")?;
    }
    if let Some(name) = name {
        result.push((name, current));
    }
    ensure!(!result.is_empty(), "No generated Elixir modules");
    Ok(result)
}
pub(super) fn facade(
    tree: &mut GeneratedTree,
    app: &str,
    module: &str,
    declarations: &[String],
    path: &str,
) -> Result<()> {
    let mut source = format!("defmodule {module} do\n");
    for (index, chunk) in declarations.chunks(25).enumerate() {
        let digest = module.bytes().fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
        });
        let name = format!(
            "{}.Internal.Exports{digest:016x}Part{index:03}",
            module.split('.').next().unwrap()
        );
        writeln!(source, "  use {name}")?;
        let code = format!(
            "defmodule {name} do\n defmacro __using__(_options) do\n  quote do\n{}\n  end\n end\nend\n",
            chunk.join("\n")
        );
        tree.insert(GeneratedFile::new(
            format!("lib/{app}/internal/{}_exports{index:03}.ex", stem(module)),
            code,
        )?)?;
    }
    source.push_str("end\n");
    tree.insert(GeneratedFile::new(path, source)?)?;
    Ok(())
}

/// Portable file components; module/function identities stay unchanged.
pub(super) fn stem(name: &str) -> String {
    let normalized = crate::elixir_identifier(name);
    if normalized.len() <= 180 {
        return normalized;
    }
    let hash = name.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    });
    format!("{}_{hash:016x}", &normalized[..160])
}
