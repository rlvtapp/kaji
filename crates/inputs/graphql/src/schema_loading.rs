//! Local schema composition and introspection conversion, without parser types in output APIs.
mod introspection;
use anyhow::{Context, Result, bail, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

pub struct Loaded {
    pub source: String,
    pub documents: BTreeMap<String, String>,
}
pub fn load(paths: &[PathBuf], roots: &[PathBuf]) -> Result<Loaded> {
    let mut seen = BTreeSet::new();
    let mut stack = BTreeSet::new();
    let mut documents = BTreeMap::new();
    let mut source = String::new();
    for path in paths {
        source.push_str(&visit(path, roots, &mut seen, &mut stack, &mut documents)?);
    }
    Ok(Loaded { source, documents })
}
fn visit(
    path: &Path,
    roots: &[PathBuf],
    seen: &mut BTreeSet<PathBuf>,
    stack: &mut BTreeSet<PathBuf>,
    documents: &mut BTreeMap<String, String>,
) -> Result<String> {
    let path = path
        .canonicalize()
        .with_context(|| format!("cannot resolve GraphQL source {}", path.display()))?;
    ensure!(
        !stack.contains(&path),
        "cyclic GraphQL import at {}",
        path.display()
    );
    if !seen.insert(path.clone()) {
        return Ok(String::new());
    }
    stack.insert(path.clone());
    let source = std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read GraphQL source {}", path.display()))?;
    documents.insert(path.to_string_lossy().replace('\\', "/"), source.clone());
    if source.trim_start().starts_with('{') {
        let out = introspection::to_sdl(
            &serde_json::from_str(&source).context("invalid introspection JSON")?,
        )?;
        stack.remove(&path);
        return Ok(out);
    }
    let mut out = String::new();
    for line in source.lines() {
        let line_trim = line.trim();
        if let Some(import) = line_trim
            .strip_prefix("#import ")
            .or_else(|| line_trim.strip_prefix("# import "))
        {
            let import = import
                .trim()
                .strip_prefix("* from ")
                .unwrap_or(import.trim());
            ensure!(
                import.len() >= 2,
                "invalid GraphQL import in {}",
                path.display()
            );
            let quote = import.as_bytes()[0];
            ensure!(
                (quote == b'\"' || quote == b'\'') && import.as_bytes().last() == Some(&quote),
                "GraphQL imports require a quoted full-file path; selective imports are unsupported: {line_trim}"
            );
            let relative = &import[1..import.len() - 1];
            ensure!(
                !relative.is_empty() && !relative.contains(['\"', '\'']),
                "invalid GraphQL import path"
            );
            let candidates = std::iter::once(path.parent().unwrap().join(relative))
                .chain(roots.iter().map(|root| root.join(relative)));
            let target = candidates.filter(|p| p.is_file()).collect::<Vec<_>>();
            let target = target.first().ok_or_else(|| {
                anyhow::anyhow!(
                    "GraphQL import {relative:?} not found from {}",
                    path.display()
                )
            })?;
            out.push_str(&visit(target, roots, seen, stack, documents)?);
        } else if line_trim.starts_with("#import") || line_trim.starts_with("# import") {
            bail!("malformed GraphQL import: {line_trim}");
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    stack.remove(&path);
    Ok(out)
}
