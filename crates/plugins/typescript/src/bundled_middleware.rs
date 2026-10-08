use crate::Symbol;
use anyhow::{Result, bail};
use poolster_core::{GeneratedFile, GeneratedTree, customization::BundledMiddleware};
use std::path::{Component, Path};

pub(crate) fn bundle(tree: &mut GeneratedTree, middleware: &[BundledMiddleware]) -> Result<()> {
    const ANCHOR: &str = "const middleware = [...(config.middleware ?? [])]";
    let runtimes = tree
        .iter()
        .filter(|(path, contents)| {
            path.extension().is_some_and(|ext| ext == "ts") && contents.contains(ANCHOR)
        })
        .map(|(path, contents)| (path.to_owned(), contents.to_owned()))
        .collect::<Vec<_>>();
    if runtimes.is_empty() {
        bail!(
            "bundled TypeScript middleware requires a maintained Fetch or Axios transport provider"
        );
    }
    let mut imports = Vec::new();
    for (index, item) in middleware.iter().enumerate() {
        item.validate()?;
        if item.async_symbol.is_some() {
            bail!(
                "TypeScript middleware uses one async ClientMiddleware symbol; async_symbol is Python-only"
            );
        }
        let path = item
            .path
            .components()
            .filter(|c| !matches!(c, Component::CurDir))
            .collect::<std::path::PathBuf>();
        if path.extension().is_none_or(|ext| ext != "ts")
            || path.to_string_lossy().ends_with(".d.ts")
        {
            bail!("bundled TypeScript middleware must be a .ts implementation file");
        }
        if !path.components().all(|c| {
            c.as_os_str().to_str().is_some_and(|p| {
                p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
            })
        }) {
            bail!("bundled TypeScript middleware path must use portable module names");
        }
        if tree.iter().any(|(p, _)| {
            p.components()
                .filter(|c| !matches!(c, Component::CurDir))
                .collect::<std::path::PathBuf>()
                == path
        }) {
            bail!(
                "bundled middleware source collides with generated file {}",
                path.display()
            );
        }
        let alias = format!("poolsterBundledMiddleware{index}");
        imports.push((
            Symbol {
                module: path.with_extension(""),
                name: item.symbol.clone(),
            },
            alias,
        ));
        tree.insert(GeneratedFile::new(&path, &item.contents)?)?;
        tree.set_owner(&path, format!("bundled-middleware:{}", path.display()))?;
    }
    let defaults = imports
        .iter()
        .map(|(_, alias)| alias.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    for (path, runtime) in runtimes {
        if runtime.matches(ANCHOR).count() != 1 {
            bail!(
                "ambiguous TypeScript middleware registration anchor in {}",
                path.display()
            );
        }
        let mut prelude = String::new();
        for (symbol, alias) in &imports {
            prelude.push_str(&format!(
                "import {{ {} as {alias} }} from {}\n",
                symbol.name,
                serde_json::to_string(&symbol.import_from(&path)?)?
            ));
        }
        let contents = format!("{prelude}{}", runtime.replace(ANCHOR, &format!("const middleware: readonly ClientMiddleware[] = [{defaults}, ...(config.middleware ?? [])]")));
        tree.replace(GeneratedFile::new(path, contents)?)?;
    }
    if let Some(readme) = tree.get("README.md") {
        let contents = format!(
            "{readme}\n## Bundled middleware\n\nThis SDK includes author-supplied middleware enabled by default. No constructor registration is needed. Additional customer middleware can still be configured; bundled layers are outermost, in recipe order. Middleware source modules ship with the SDK.\n"
        );
        tree.replace(GeneratedFile::new(Path::new("README.md"), contents)?)?;
    }
    Ok(())
}
