use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Result, bail};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedFile {
    pub path: PathBuf,
    pub contents: String,
}

impl GeneratedFile {
    pub fn new(path: impl AsRef<Path>, contents: impl Into<String>) -> Result<Self> {
        let path = path.as_ref();
        if path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            bail!("generated paths must be relative and cannot escape their output directory");
        }
        if path.as_os_str().is_empty() {
            bail!("generated file paths cannot be empty");
        }
        Ok(Self {
            path: path.to_path_buf(),
            contents: contents.into(),
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GeneratedTree {
    files: BTreeMap<PathBuf, String>,
    preserve_existing: BTreeSet<PathBuf>,
}

impl GeneratedTree {
    pub fn insert(&mut self, file: GeneratedFile) -> Result<()> {
        if self.files.contains_key(&file.path) {
            bail!("multiple generators emitted {}", file.path.display());
        }
        self.files.insert(file.path, file.contents);
        Ok(())
    }

    /// Adds a starter file that is generated only on first materialization.
    ///
    /// This is the escape hatch for package-owned customization modules: the
    /// generator supplies a valid initial file, then later generation passes
    /// keep a developer's edits intact instead of overwriting them.
    pub fn insert_custom(&mut self, file: GeneratedFile) -> Result<()> {
        if self.files.contains_key(&file.path) {
            bail!("multiple generators emitted {}", file.path.display());
        }
        self.preserve_existing.insert(file.path.clone());
        self.files.insert(file.path, file.contents);
        Ok(())
    }

    pub fn get(&self, path: impl AsRef<Path>) -> Option<&str> {
        self.files.get(path.as_ref()).map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Path, &str)> {
        self.files
            .iter()
            .map(|(path, contents)| (path.as_path(), contents.as_str()))
    }

    /// Merges a separately generated package tree, retaining the same
    /// collision protections as individual file insertion. SDK profile
    /// orchestrators use this to compose language-specific generators into a
    /// single validated release tree.
    pub fn append(&mut self, other: GeneratedTree) -> Result<()> {
        let GeneratedTree {
            files,
            preserve_existing,
        } = other;
        for (path, contents) in files {
            let file = GeneratedFile::new(&path, contents)?;
            if preserve_existing.contains(&path) {
                self.insert_custom(file)?;
            } else {
                self.insert(file)?;
            }
        }
        Ok(())
    }

    /// Materializes this validated tree beneath `root` without permitting
    /// symlink traversal outside the requested output directory.
    pub fn write_to(&self, root: impl AsRef<Path>) -> Result<()> {
        fs::create_dir_all(root.as_ref())?;
        let root = fs::canonicalize(root.as_ref())?;
        for (relative, contents) in &self.files {
            let destination = root.join(relative);
            let parent = destination
                .parent()
                .ok_or_else(|| anyhow::anyhow!("generated path has no parent"))?;
            fs::create_dir_all(parent)?;
            let canonical_parent = fs::canonicalize(parent)?;
            if !canonical_parent.starts_with(&root) {
                bail!("generated path escapes its output directory");
            }
            if fs::symlink_metadata(&destination)
                .map(|metadata| metadata.file_type().is_symlink())
                .unwrap_or(false)
            {
                bail!("refusing to overwrite generated output through a symlink");
            }
            if self.preserve_existing.contains(relative) && destination.exists() {
                continue;
            }
            fs::write(destination, contents)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{GeneratedFile, GeneratedTree};

    #[test]
    fn append_merges_isolated_trees_and_rejects_collisions() {
        let mut left = GeneratedTree::default();
        left.insert(GeneratedFile::new("rust/lib.rs", "left").unwrap())
            .unwrap();
        let mut right = GeneratedTree::default();
        right
            .insert(GeneratedFile::new("python/__init__.py", "right").unwrap())
            .unwrap();
        left.append(right).unwrap();
        assert_eq!(left.get("python/__init__.py"), Some("right"));

        let mut collision = GeneratedTree::default();
        collision
            .insert(GeneratedFile::new("rust/lib.rs", "other").unwrap())
            .unwrap();
        assert!(left.append(collision).is_err());
    }

    #[test]
    fn custom_files_are_created_once_and_preserved_on_regeneration() {
        let output = tempfile::tempdir().unwrap();
        let path = "typescript/custom/index.ts";
        let mut first = GeneratedTree::default();
        first
            .insert_custom(GeneratedFile::new(path, "export const first = true\n").unwrap())
            .unwrap();
        first.write_to(output.path()).unwrap();
        fs::write(output.path().join(path), "export const userOwned = true\n").unwrap();

        let mut regenerated = GeneratedTree::default();
        regenerated
            .insert_custom(GeneratedFile::new(path, "export const replacement = true\n").unwrap())
            .unwrap();
        regenerated.write_to(output.path()).unwrap();

        assert_eq!(
            fs::read_to_string(output.path().join(path)).unwrap(),
            "export const userOwned = true\n"
        );
    }
}
