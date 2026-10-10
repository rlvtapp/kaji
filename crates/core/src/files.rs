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
    owners: BTreeMap<PathBuf, String>,
    retained_prefixes: BTreeSet<PathBuf>,
}

type OutputPlan = (
    OutputChanges,
    BTreeMap<PathBuf, String>,
    Ownership,
    Vec<PathBuf>,
);

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
        self.into_owned_files()
            .map(|(file, custom, _)| (file, custom))
    }

    pub fn into_owned_files(self) -> impl Iterator<Item = (GeneratedFile, bool, Option<String>)> {
        let Self {
            files,
            preserve_existing,
            mut owners,
            retained_prefixes: _,
        } = self;
        files.into_iter().map(move |(path, contents)| {
            let custom = preserve_existing.contains(&path);
            let owner = owners.remove(&path);
            (GeneratedFile { path, contents }, custom, owner)
        })
    }

    /// Assign a persistent package/plugin identity, never a process-local ID.
    pub fn set_owner(&mut self, path: impl AsRef<Path>, owner: impl Into<String>) -> Result<()> {
        if !self.files.contains_key(path.as_ref()) {
            bail!("cannot assign ownership to missing generated file");
        }
        let owner = owner.into();
        if owner.trim().is_empty() {
            bail!("generated owner cannot be empty");
        }
        self.owners.insert(path.as_ref().to_path_buf(), owner);
        Ok(())
    }

    pub fn append(&mut self, other: GeneratedTree) -> Result<()> {
        self.retained_prefixes
            .extend(other.retained_prefixes.iter().cloned());
        for (file, custom, owner) in other.into_owned_files() {
            let path = file.path.clone();
            if custom {
                self.insert_custom(file)?;
            } else {
                self.insert(file)?;
            }
            if let Some(owner) = owner {
                self.set_owner(path, owner)?;
            }
        }
        Ok(())
    }

    /// Retain prior ownership and bytes for a skipped package during mixed generation.
    /// Files under this prefix are neither written nor removed, including local edits.
    /// The prior hash is retained so skipping does not silently adopt those edits.
    pub fn preserve_owned_prefix(
        &mut self,
        root: impl AsRef<Path>,
        prefix: impl AsRef<Path>,
    ) -> Result<()> {
        let prefix = prefix.as_ref();
        GeneratedFile::new(prefix, "")?;
        let prefix: PathBuf = prefix
            .components()
            .filter(|part| !matches!(part, Component::CurDir))
            .collect();
        if prefix.as_os_str().is_empty() {
            bail!("retained package prefix cannot be empty or '.'");
        }
        safe_path(root.as_ref(), &prefix)?;
        if self.files.keys().any(|path| path.starts_with(&prefix)) {
            bail!(
                "retained package prefix overlaps generated files: {}",
                prefix.display()
            );
        }
        self.retained_prefixes.insert(prefix);
        Ok(())
    }

    /// Compare the complete generated output without modifying its destination.
    pub fn check(&self, root: impl AsRef<Path>) -> Result<OutputChanges> {
        Ok(self.plan(root.as_ref(), false)?.0)
    }

    /// Validate all changes before writing. Only previously owned, unchanged
    /// files may be removed; create-once files remain user-owned.
    pub fn write_to(&self, root: impl AsRef<Path>) -> Result<()> {
        let root = root.as_ref();
        let (_, output, manifest, removed) = self.plan(root, true)?;
        fs::create_dir_all(root)?;
        for (relative, contents) in output {
            let destination = root.join(relative);
            fs::create_dir_all(destination.parent().context("missing output parent")?)?;
            fs::write(&destination, contents)?;
        }
        for relative in removed {
            fs::remove_file(root.join(relative))?;
        }
        let destination = root.join(OWNERSHIP_PATH);
        fs::create_dir_all(destination.parent().unwrap())?;
        fs::write(destination, serde_json::to_string_pretty(&manifest)? + "\n")?;
        Ok(())
    }

    fn plan(&self, root: &Path, enforce_edits: bool) -> Result<OutputPlan> {
        safe_path(root, Path::new(OWNERSHIP_PATH))?;
        let manifest_path = root.join(OWNERSHIP_PATH);
        let previous: Ownership = if manifest_path.exists() {
            serde_json::from_str(&fs::read_to_string(&manifest_path)?)
                .context("invalid Poolster ownership manifest")?
        } else {
            Ownership::default()
        };
        if previous.version != 1 {
            bail!("unsupported Poolster ownership manifest version");
        }
        for path in previous.files.keys() {
            GeneratedFile::new(path, "")?;
            if path == Path::new(OWNERSHIP_PATH) {
                bail!("ownership manifest cannot own itself");
            }
            safe_path(root, path)?;
        }
        let mut changes = OutputChanges::default();
        let mut output = BTreeMap::new();
        let mut manifest = Ownership::default();
        for (path, owned) in &previous.files {
            if self
                .retained_prefixes
                .iter()
                .any(|prefix| path.starts_with(prefix))
            {
                manifest.files.insert(path.clone(), owned.clone());
            }
        }
        let mut removed = Vec::new();
        for (path, generated) in &self.files {
            if self
                .retained_prefixes
                .iter()
                .any(|prefix| path.starts_with(prefix))
            {
                bail!(
                    "retained package prefix overlaps generated file {}",
                    path.display()
                );
            }
            GeneratedFile::new(path, "")?;
            if path == Path::new(OWNERSHIP_PATH) {
                bail!("reserved Poolster ownership path");
            }
            safe_path(root, path)?;
            let existing = if root.join(path).exists() {
                Some(fs::read_to_string(root.join(path))?)
            } else {
                None
            };
            if self.preserve_existing.contains(path) && existing.is_some() {
                // Retain the original owner/hash without adopting author edits.
                if let Some(owned) = previous.files.get(path) {
                    manifest.files.insert(path.clone(), owned.clone());
                }
                continue;
            }
            let npm = path.file_name().is_some_and(|name| name == "package.json")
                && !path
                    .components()
                    .any(|part| part.as_os_str() == ".poolster");
            let contents = if npm {
                match &existing {
                    Some(existing) => merge_npm_manifest(existing, generated)?,
                    None => merge_npm_manifest("{}", generated)?,
                }
            } else {
                generated.clone()
            };
            if let Some(existing) = &existing {
                if !npm {
                    if let Some(owned) = previous.files.get(path) {
                        if enforce_edits
                            && existing != &contents
                            && digest(existing) != owned.sha256
                            && !equal_package_metadata(path, existing, &contents)
                        {
                            bail!(
                                "locally modified generated file {}; preserve or restore it before regeneration",
                                path.display()
                            );
                        }
                    } else if existing != generated
                        && path != Path::new(".poolster/generation.lock.json")
                    {
                        bail!("refusing to overwrite unowned file {}", path.display());
                    }
                }
                if existing != &contents {
                    changes.modified.push(path.clone());
                }
            } else {
                changes.added.push(path.clone());
            }
            manifest.files.insert(
                path.clone(),
                OwnedFile {
                    owner: self.owners.get(path).cloned().unwrap_or_else(|| {
                        format!(
                            "poolster:{}",
                            path.components()
                                .next()
                                .unwrap()
                                .as_os_str()
                                .to_string_lossy()
                        )
                    }),
                    sha256: digest(&contents),
                    create_once: self.preserve_existing.contains(path),
                },
            );
            output.insert(path.clone(), contents);
        }
        for (path, owned) in &previous.files {
            if self
                .retained_prefixes
                .iter()
                .any(|prefix| path.starts_with(prefix))
                || self.files.contains_key(path)
                || owned.create_once
                || (path.file_name().is_some_and(|name| name == "package.json")
                    && !path
                        .components()
                        .any(|part| part.as_os_str() == ".poolster"))
            {
                continue;
            }
            if root.join(path).exists() {
                let existing = fs::read_to_string(root.join(path))?;
                if enforce_edits && digest(&existing) != owned.sha256 {
                    bail!(
                        "refusing to remove locally modified generated file {}",
                        path.display()
                    );
                }
                changes.removed.push(path.clone());
                removed.push(path.clone());
            }
        }
        Ok((changes, output, manifest, removed))
    }
}

fn equal_package_metadata(path: &Path, existing: &str, generated: &str) -> bool {
    path.ends_with(".poolster/package.json")
        && matches!(
            (serde_json::from_str::<serde_json::Value>(existing), serde_json::from_str::<serde_json::Value>(generated)),
            (Ok(existing), Ok(generated)) if existing == generated
        )
}

pub const OWNERSHIP_PATH: &str = ".poolster/ownership.json";

#[derive(Clone, Debug, Default, serde::Serialize, PartialEq, Eq)]
pub struct OutputChanges {
    pub added: Vec<PathBuf>,
    pub modified: Vec<PathBuf>,
    pub removed: Vec<PathBuf>,
}
impl OutputChanges {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.modified.is_empty() && self.removed.is_empty()
    }
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Ownership {
    version: u8,
    files: BTreeMap<PathBuf, OwnedFile>,
}
impl Default for Ownership {
    fn default() -> Self {
        Self {
            version: 1,
            files: BTreeMap::new(),
        }
    }
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnedFile {
    owner: String,
    sha256: String,
    #[serde(default)]
    create_once: bool,
}
/// A deterministic, portable stem for generated source files. Public symbols remain unchanged.
/// Long or unsafe identities receive a digest suffix; ordinary identifiers stay readable.
pub fn source_file_stem(identity: &str) -> String {
    if identity.len() <= 96
        && !identity.is_empty()
        && identity
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-$".contains(&c))
    {
        return identity.to_owned();
    }
    let prefix: String = identity
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect();
    format!("{prefix}_{}", &digest(identity)[..16])
}

fn digest(value: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn safe_path(root: &Path, relative: &Path) -> Result<()> {
    // Reject links even when they point inside the root; never follow them while
    // checking, creating directories, reading, writing, or removing output.
    // macOS exposes temporary storage through /var and /tmp aliases. Permit
    // those system aliases, but reject user-controlled links above a missing root.
    let mut ancestor = PathBuf::new();
    for component in root.components() {
        ancestor.push(component.as_os_str());
        if fs::symlink_metadata(&ancestor).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            let system_alias = cfg!(target_os = "macos")
                && matches!(ancestor.to_str(), Some("/var" | "/tmp" | "/etc"))
                && fs::read_link(&ancestor).is_ok_and(|target| {
                    matches!(
                        target.to_str(),
                        Some(
                            "private/var"
                                | "private/tmp"
                                | "private/etc"
                                | "/private/var"
                                | "/private/tmp"
                                | "/private/etc"
                        )
                    )
                });
            if !system_alias {
                bail!(
                    "refusing symlink ancestor of output root {}",
                    ancestor.display()
                );
            }
        }
    }
    if fs::symlink_metadata(root).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        bail!("refusing symlink output root {}", root.display());
    }
    let mut current = root.to_path_buf();
    for part in relative.components() {
        current.push(part.as_os_str());
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            if metadata.file_type().is_symlink() {
                bail!(
                    "refusing generated output through symlink {}",
                    current.display()
                );
            }
        }
    }
    Ok(())
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
mod source_stem_tests;
#[cfg(test)]
mod tests;
