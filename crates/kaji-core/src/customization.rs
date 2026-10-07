//! Explicit author-owned source overlays, applied after package finalization.
use crate::{GeneratedFile, GeneratedTree};
use anyhow::{Context, Result, bail};
use std::path::{Component, Path, PathBuf};

/// SDK-author middleware source shipped and registered by the language renderer.
/// The symbol's native ABI is language-specific; consumers need no registration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundledMiddleware {
    pub path: PathBuf,
    pub contents: String,
    pub symbol: String,
    pub async_symbol: Option<String>,
}
impl BundledMiddleware {
    pub fn validate(&self) -> Result<()> {
        normalized(&self.path)?;
        for symbol in std::iter::once(self.symbol.as_str()).chain(self.async_symbol.as_deref()) {
            let mut chars = symbol.chars();
            if !chars
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                bail!("bundled middleware symbol must be a simple ASCII identifier: {symbol:?}");
            }
        }
        Ok(())
    }
}

/// Code supplied by the SDK author. Contents are data; Kaji never executes them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodeCustomization {
    Add {
        path: PathBuf,
        contents: String,
    },
    Replace {
        path: PathBuf,
        contents: String,
    },
    Patch {
        path: PathBuf,
        find: String,
        replacement: String,
    },
}
impl CodeCustomization {
    pub fn path(&self) -> &Path {
        match self {
            Self::Add { path, .. } | Self::Replace { path, .. } | Self::Patch { path, .. } => path,
        }
    }
    pub fn prefixed(&self, prefix: &Path) -> Self {
        let path = prefix.join(self.path());
        match self {
            Self::Add { contents, .. } => Self::Add {
                path,
                contents: contents.clone(),
            },
            Self::Replace { contents, .. } => Self::Replace {
                path,
                contents: contents.clone(),
            },
            Self::Patch {
                find, replacement, ..
            } => Self::Patch {
                path,
                find: find.clone(),
                replacement: replacement.clone(),
            },
        }
    }
}
fn normalized(path: &Path) -> Result<PathBuf> {
    GeneratedFile::new(path, "")?;
    let path: PathBuf = path
        .components()
        .filter(|p| !matches!(p, Component::CurDir))
        .collect();
    if path.as_os_str().is_empty() {
        bail!("customization path must name a file");
    }
    if matches!(
        path.to_str(),
        Some(".kaji/ownership.json" | ".kaji/generation.lock.json" | ".kaji/package.json")
    ) || path.ends_with(".kaji/ownership.json")
        || path.ends_with(".kaji/generation.lock.json")
        || path.ends_with(".kaji/package.json")
    {
        bail!(
            "customization cannot replace Kaji bookkeeping: {}",
            path.display()
        );
    }
    Ok(path)
}
/// Apply the entire sequence atomically in memory. Normal ownership protection
/// still applies when writing: edits in output are never silently overwritten.
pub fn apply_code_customizations(
    tree: &mut GeneratedTree,
    changes: &[CodeCustomization],
) -> Result<()> {
    let mut staged = tree.clone();
    for change in changes {
        let path = normalized(change.path())?;
        let existing = staged
            .iter()
            .find(|(candidate, _)| {
                candidate
                    .components()
                    .filter(|p| !matches!(p, Component::CurDir))
                    .collect::<PathBuf>()
                    == path
            })
            .map(|(p, c)| (p.to_owned(), c.to_owned()));
        let target = existing
            .as_ref()
            .map(|(p, _)| p.clone())
            .unwrap_or(path.clone());
        if staged.preserves_existing(&target) {
            bail!(
                "customization cannot overwrite create-once user file {}",
                path.display()
            );
        }
        let contents = match change {
            CodeCustomization::Add { contents, .. } => {
                if existing.is_some() {
                    bail!(
                        "customization add collides with generated file {}",
                        path.display()
                    );
                }
                contents.clone()
            }
            CodeCustomization::Replace { contents, .. } => {
                existing.as_ref().with_context(|| {
                    format!("customization replace target missing: {}", path.display())
                })?;
                contents.clone()
            }
            CodeCustomization::Patch {
                find, replacement, ..
            } => {
                let (_, contents) = existing.as_ref().with_context(|| {
                    format!("customization patch target missing: {}", path.display())
                })?;
                if find.is_empty() || contents.matches(find).count() != 1 {
                    bail!(
                        "customization patch must match exactly once in {}",
                        path.display()
                    );
                }
                contents.replacen(find, replacement, 1)
            }
        };
        let file = GeneratedFile::new(&target, contents)?;
        if existing.is_some() {
            staged.replace(file)?;
        } else {
            staged.insert(file)?;
        }
        staged.set_owner(&target, format!("code-customization:{}", path.display()))?;
    }
    *tree = staged;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scoped_overrides_are_transactional_and_reject_stale_patches() {
        let mut tree = GeneratedTree::default();
        for p in ["ts/client.ts", "go/client.go"] {
            tree.insert(GeneratedFile::new(p, "original").unwrap())
                .unwrap();
        }
        apply_code_customizations(
            &mut tree,
            &[CodeCustomization::Patch {
                path: "ts/client.ts".into(),
                find: "original".into(),
                replacement: "custom".into(),
            }],
        )
        .unwrap();
        assert_eq!(tree.get("ts/client.ts"), Some("custom"));
        assert_eq!(tree.get("go/client.go"), Some("original"));
        let before = tree.clone();
        assert!(
            apply_code_customizations(
                &mut tree,
                &[
                    CodeCustomization::Add {
                        path: "helper.ts".into(),
                        contents: "helper".into()
                    },
                    CodeCustomization::Patch {
                        path: "ts/client.ts".into(),
                        find: "original".into(),
                        replacement: "bad".into()
                    },
                ]
            )
            .is_err()
        );
        assert_eq!(tree, before);
    }
    #[test]
    fn rejects_escape_collision_bookkeeping_and_user_owned_targets() {
        let mut tree = GeneratedTree::default();
        tree.insert_custom(GeneratedFile::new("custom.ts", "starter").unwrap())
            .unwrap();
        for path in [
            "../escape",
            ".",
            ".kaji/package.json",
            "ts/.kaji/ownership.json",
            "custom.ts",
        ] {
            assert!(
                apply_code_customizations(
                    &mut tree,
                    &[CodeCustomization::Replace {
                        path: path.into(),
                        contents: "bad".into()
                    }]
                )
                .is_err()
            );
        }
        tree.insert(GeneratedFile::new("./client.ts", "x x").unwrap())
            .unwrap();
        assert!(
            apply_code_customizations(
                &mut tree,
                &[CodeCustomization::Add {
                    path: "client.ts".into(),
                    contents: "bad".into()
                }]
            )
            .is_err()
        );
        assert!(
            apply_code_customizations(
                &mut tree,
                &[CodeCustomization::Patch {
                    path: "client.ts".into(),
                    find: "x".into(),
                    replacement: "y".into()
                }]
            )
            .is_err()
        );
    }
}
