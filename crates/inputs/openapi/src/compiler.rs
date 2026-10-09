//! Invoke Poolster's existing compiler without owning platform installation.
use anyhow::{Context, Result, bail};
use std::{path::Path, process::Command};

/// Compile a raw OpenAPI document into the existing artifact format.
/// The caller supplies its installed/bundled compiler and output directory.
/// Native compiler diagnostics and local-reference handling remain unchanged.
pub fn compile(
    executable: &Path,
    source: &Path,
    output: &Path,
    source_url: Option<&str>,
) -> Result<()> {
    let mut command = Command::new(executable);
    command.arg("--out").arg(output);
    if let Some(url) = source_url {
        command.arg("--source-url").arg(url);
    }
    let status = command
        .arg(source)
        .status()
        .with_context(|| format!("cannot start OpenAPI compiler {}", executable.display()))?;
    if !status.success() {
        bail!("OpenAPI compiler failed ({status}); no SDK files were written");
    }
    Ok(())
}
