//! Shared TypeScript package state and immutable provider contracts.
use crate::TypeScript;
use anyhow::{Result, bail};
use kaji_core::{
    GeneratedFile,
    engine::{Contract, FinalizeContext},
};
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    /// Package-relative module without its `.ts` extension.
    pub module: PathBuf,
    pub name: String,
}
impl Symbol {
    /// Produces a relative import specifier from a package-relative TS file.
    pub fn import_from(&self, file: impl AsRef<Path>) -> Result<String> {
        let file = GeneratedFile::new(file, "")?.path;
        let source: Vec<_> = file
            .parent()
            .unwrap_or(Path::new(""))
            .components()
            .filter(|c| !matches!(c, Component::CurDir))
            .collect();
        let target: Vec<_> = self.module.components().collect();
        let shared = source
            .iter()
            .zip(&target)
            .take_while(|(a, b)| a == b)
            .count();
        let mut parts = vec!["..".to_owned(); source.len() - shared];
        parts.extend(
            target[shared..]
                .iter()
                .map(|p| p.as_os_str().to_string_lossy().into_owned()),
        );
        let result = parts.join("/");
        Ok(if result.starts_with("../") {
            result
        } else {
            format!("./{result}")
        })
    }
}

/// Schema symbols actually emitted by the types-only plugin. Operation/client
/// contracts will be added when the maintained SDK renderer is split.
pub struct TsTypes {
    pub schemas: BTreeMap<String, Symbol>,
}
impl Contract for TsTypes {
    const NAME: &'static str = "typescript.schema-types";
}

#[derive(Default)]
pub struct Workspace {
    symbols: BTreeMap<(PathBuf, String), String>,
    package_files: BTreeMap<PathBuf, String>,
    dependencies: BTreeMap<String, String>,
}
impl Workspace {
    pub fn declare(&mut self, module: impl AsRef<Path>, name: &str, owner: &str) -> Result<Symbol> {
        let module = GeneratedFile::new(module, "")?
            .path
            .components()
            .filter(|c| !matches!(c, Component::CurDir))
            .collect::<PathBuf>();
        if module.as_os_str().is_empty() {
            bail!("TypeScript module cannot be empty");
        }
        let key = (module.clone(), name.to_owned());
        if let Some(previous) = self.symbols.get(&key) {
            bail!(
                "symbol {name} in {} is declared by both {previous} and {owner}",
                module.display()
            );
        }
        self.symbols.insert(key, owner.into());
        Ok(Symbol {
            module,
            name: name.into(),
        })
    }

    /// Conservatively requires identical dependency declarations. Semver range
    /// intersection is deliberately not guessed during this migration.
    pub fn dependency(&mut self, name: impl Into<String>, range: impl Into<String>) -> Result<()> {
        let name = name.into();
        let range = range.into();
        if let Some(previous) = self.dependencies.get(&name) {
            if previous != &range {
                bail!("conflicting npm dependency {name}: {previous} versus {range}");
            }
        }
        self.dependencies.insert(name, range);
        Ok(())
    }

    pub(crate) fn package_file(&mut self, file: GeneratedFile) -> Result<()> {
        if self.package_files.contains_key(&file.path) {
            bail!(
                "multiple plugins own {}; put complete SDKs in separate packages",
                file.path.display()
            );
        }
        self.package_files.insert(file.path, file.contents);
        Ok(())
    }
}

pub(crate) fn finalize(cx: &mut FinalizeContext<'_, TypeScript>) -> Result<()> {
    let mut files = std::mem::take(&mut cx.workspace.package_files);
    if !cx.workspace.dependencies.is_empty() {
        let manifest = files.get_mut(Path::new("package.json")).ok_or_else(|| {
            anyhow::anyhow!("npm dependencies require a package manifest provider")
        })?;
        let mut value: serde_json::Value = serde_json::from_str(manifest)?;
        for (name, range) in &cx.workspace.dependencies {
            for field in ["dependencies", "devDependencies", "peerDependencies"] {
                if let Some(previous) = value[field][name].as_str() {
                    if previous != range {
                        bail!("conflicting npm dependency {name}: {previous} versus {range}");
                    }
                }
            }
            value["dependencies"][name] = serde_json::Value::String(range.clone());
        }
        *manifest = format!("{}\n", serde_json::to_string_pretty(&value)?);
    }
    for (path, contents) in files {
        cx.files.emit(GeneratedFile::new(path, contents)?)?;
    }
    Ok(())
}
