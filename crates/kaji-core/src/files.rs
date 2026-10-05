use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

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

    /// Replaces the contents of a file already owned by this generated tree.
    ///
    /// Composition layers can use this for a deliberate finalization pass,
    /// such as extending a generated package manifest. It never creates a new
    /// file, so accidental ownership conflicts remain visible through `insert`.
    pub fn replace(&mut self, file: GeneratedFile) -> Result<()> {
        if !self.files.contains_key(&file.path) {
            bail!(
                "cannot replace missing generated file {}",
                file.path.display()
            );
        }
        self.files.insert(file.path, file.contents);
        Ok(())
    }

    pub fn get(&self, path: impl AsRef<Path>) -> Option<&str> {
        self.files.get(path.as_ref()).map(String::as_str)
    }

    pub fn preserves_existing(&self, path: impl AsRef<Path>) -> bool {
        self.preserve_existing.contains(path.as_ref())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Path, &str)> {
        self.files
            .iter()
            .map(|(path, contents)| (path.as_path(), contents.as_str()))
    }

    /// Moves file contents and create-once metadata without copying the entire
    /// generated SDK between composition layers.
    pub fn into_files(self) -> impl Iterator<Item = (GeneratedFile, bool)> {
        let Self {
            files,
            preserve_existing,
        } = self;
        files.into_iter().map(move |(path, contents)| {
            let custom = preserve_existing.contains(&path);
            (GeneratedFile { path, contents }, custom)
        })
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

        // Validate each containing directory once before materializing files.
        // Large OpenAPI documents commonly produce tens of thousands of files;
        // canonicalizing the same directory for every file made that safe path
        // check dominate generation time. `GeneratedFile` has already rejected
        // absolute and parent-directory paths, and checking every unique parent
        // still prevents a generated path from traversing a pre-existing link.
        let parents = self
            .files
            .keys()
            .map(|relative| relative.parent().map(Path::to_path_buf).unwrap_or_default())
            .collect::<BTreeSet<_>>();
        for relative_parent in parents {
            let parent = root.join(relative_parent);
            fs::create_dir_all(&parent)?;
            let canonical_parent = fs::canonicalize(&parent)?;
            if !canonical_parent.starts_with(&root) {
                bail!("generated path escapes its output directory");
            }
        }
        // Resolve every existing npm manifest before writing any generated file.
        // Invalid user JSON must fail generation rather than be overwritten.
        let mut manifests = BTreeMap::new();
        for (relative, contents) in &self.files {
            if relative
                .file_name()
                .is_some_and(|name| name == "package.json")
                && !self.preserve_existing.contains(relative)
            {
                let destination = root.join(relative);
                if fs::symlink_metadata(&destination)
                    .map(|metadata| metadata.file_type().is_symlink())
                    .unwrap_or(false)
                {
                    bail!("refusing to read generated output through a symlink");
                }
                if destination.exists() {
                    let existing = fs::read_to_string(&destination)?;
                    let merged = merge_npm_manifest(&existing, contents)
                        .with_context(|| format!("cannot merge {}", destination.display()))?;
                    manifests.insert(relative.clone(), merged);
                }
            }
        }
        for (relative, contents) in &self.files {
            let contents = manifests.get(relative).unwrap_or(contents);
            let destination = root.join(relative);
            if fs::symlink_metadata(&destination)
                .map(|metadata| metadata.file_type().is_symlink())
                .unwrap_or(false)
            {
                bail!("refusing to overwrite generated output through a symlink");
            }
            if self.preserve_existing.contains(relative) && destination.exists() {
                continue;
            }
            fs::write(&destination, contents).with_context(|| {
                format!("cannot write generated file {}", destination.display())
            })?;
        }
        Ok(())
    }
}

/// User-owned values win, while missing generated requirements are appended.
/// Package identity/version and generated export destinations remain authoritative.
fn merge_npm_manifest(existing: &str, generated: &str) -> Result<String> {
    use serde_json::Value;
    let mut existing: Value = serde_json::from_str(existing)?;
    let mut generated: Value = serde_json::from_str(generated)?;
    let user = existing
        .as_object_mut()
        .context("existing package.json must be an object")?;
    let requirements = generated
        .as_object_mut()
        .context("generated package.json must be an object")?;
    let sections = [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ];
    for section in sections {
        if let Some(value) = user.get(section) {
            let entries = value
                .as_object()
                .with_context(|| format!("{section} must be an object"))?;
            if entries.values().any(|value| !value.is_string()) {
                bail!("{section} dependency versions must be strings");
            }
        }
    }
    // Respect the user's chosen range and dependency category, including optional peers.
    for section in sections {
        if let Some(entries) = requirements.get_mut(section).and_then(Value::as_object_mut) {
            entries.retain(|name, _| {
                !sections.iter().any(|section| {
                    user.get(*section)
                        .and_then(Value::as_object)
                        .is_some_and(|entries| entries.contains_key(name))
                })
            });
        }
    }
    fn append(user: &mut Value, required: Value) {
        match (user, required) {
            (Value::Object(user), Value::Object(required)) => {
                for (key, value) in required {
                    if let Some(existing) = user.get_mut(&key) {
                        append(existing, value);
                    } else {
                        user.insert(key, value);
                    }
                }
            }
            (Value::Array(user), Value::Array(required)) => {
                for value in required {
                    if !user.contains(&value) {
                        user.push(value);
                    }
                }
            }
            _ => {}
        }
    }
    for field in ["name", "version"] {
        if let Some(value) = requirements.remove(field) {
            user.insert(field.into(), value);
        }
    }
    if let Some(exports) = requirements.remove("exports") {
        if let Some(exports) = exports.as_object() {
            let target = user
                .entry("exports")
                .or_insert_with(|| serde_json::json!({}));
            let target = target
                .as_object_mut()
                .context("existing exports must be an object to add generated exports")?;
            for (key, value) in exports {
                target.insert(key.clone(), value.clone());
            }
        } else {
            user.insert("exports".into(), exports);
        }
    }
    append(&mut existing, generated);
    Ok(format!("{}\n", serde_json::to_string_pretty(&existing)?))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{GeneratedFile, GeneratedTree};

    #[test]
    fn npm_manifest_preserves_custom_settings_and_dependency_categories() {
        let existing = r#"{
            "name":"old", "version":"0.0.1", "private":true,
            "scripts":{"build":"custom-build", "test":"test-command"},
            "devDependencies":{"typescript":"5.9.3"},
            "peerDependencies":{"@tanstack/react-query":"^5.100.0"},
            "peerDependenciesMeta":{"@tanstack/react-query":{"optional":true}},
            "exports":{"./custom":"./custom.js", ".":"./old.js"},
            "files":["custom.js"], "repository":{"url":"custom"}
        }"#;
        let generated = r#"{
            "name":"@relevate/sdk", "version":"0.2.0", "type":"module",
            "scripts":{"build":"tsc", "generate":"kaji generate"},
            "devDependencies":{"typescript":"^7.0.0"},
            "dependencies":{"@tanstack/react-query":"^5.0.0", "zod":"^4.0.0"},
            "exports":{".":{"import":"./dist/index.js"}}, "files":["dist"]
        }"#;
        let merged = super::merge_npm_manifest(existing, generated).unwrap();
        let value: serde_json::Value = serde_json::from_str(&merged).unwrap();
        assert_eq!(value["name"], "@relevate/sdk");
        assert_eq!(value["version"], "0.2.0");
        assert_eq!(value["scripts"]["build"], "custom-build");
        assert_eq!(value["scripts"]["generate"], "kaji generate");
        assert_eq!(value["devDependencies"]["typescript"], "5.9.3");
        assert!(value["dependencies"].get("@tanstack/react-query").is_none());
        assert_eq!(
            value["peerDependencies"]["@tanstack/react-query"],
            "^5.100.0"
        );
        assert_eq!(value["dependencies"]["zod"], "^4.0.0");
        assert_eq!(value["exports"]["./custom"], "./custom.js");
        assert_eq!(value["exports"]["."]["import"], "./dist/index.js");
        assert_eq!(value["files"], serde_json::json!(["custom.js", "dist"]));
        assert_eq!(value["repository"]["url"], "custom");
        assert_eq!(
            super::merge_npm_manifest(&merged, generated).unwrap(),
            merged
        );
    }

    #[test]
    fn npm_manifest_is_merged_when_materialized() {
        let output = tempfile::tempdir().unwrap();
        fs::create_dir(output.path().join("ts")).unwrap();
        fs::write(
            output.path().join("ts/package.json"),
            r#"{"scripts":{"test":"test"}}"#,
        )
        .unwrap();
        let mut tree = GeneratedTree::default();
        tree.insert(
            GeneratedFile::new(
                "ts/package.json",
                r#"{"name":"@scope/sdk","scripts":{"build":"tsc"}}"#,
            )
            .unwrap(),
        )
        .unwrap();
        tree.write_to(output.path()).unwrap();
        let value: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(output.path().join("ts/package.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(value["scripts"]["test"], "test");
        assert_eq!(value["scripts"]["build"], "tsc");
    }

    #[test]
    fn invalid_npm_manifest_fails_before_writing_generated_files() {
        for invalid in [
            "invalid JSON",
            "[]",
            r#"{"dependencies":[]}"#,
            r#"{"dependencies":{"react":42}}"#,
        ] {
            let output = tempfile::tempdir().unwrap();
            fs::write(output.path().join("package.json"), invalid).unwrap();
            let mut tree = GeneratedTree::default();
            tree.insert(GeneratedFile::new("a.ts", "new code").unwrap())
                .unwrap();
            tree.insert(GeneratedFile::new("package.json", "{}").unwrap())
                .unwrap();
            assert!(tree.write_to(output.path()).is_err());
            assert!(!output.path().join("a.ts").exists());
            assert_eq!(
                fs::read_to_string(output.path().join("package.json")).unwrap(),
                invalid
            );
        }
    }

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

    #[test]
    fn replace_updates_an_owned_file_without_creating_new_ownership() {
        let mut tree = GeneratedTree::default();
        tree.insert(GeneratedFile::new("typescript/package.json", "{}\n").unwrap())
            .unwrap();
        tree.replace(
            GeneratedFile::new("typescript/package.json", "{\"name\":\"sdk\"}\n").unwrap(),
        )
        .unwrap();
        assert_eq!(
            tree.get("typescript/package.json"),
            Some("{\"name\":\"sdk\"}\n")
        );
        assert!(
            tree.replace(GeneratedFile::new("typescript/missing.json", "{}\n").unwrap())
                .is_err()
        );
    }
}
