//! A stable inventory of generated artifacts.
//!
//! It is deliberately source-oriented.  A future formatter-aware writer can
//! extend this with on-disk hashes, like Kaji's output manifest, without
//! changing the portable manifest emitted by the generator itself.

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{GeneratedFile, GeneratedTree};

pub const MANIFEST_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub path: String,
    pub bytes: usize,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedManifest {
    pub version: u32,
    pub entries: Vec<ManifestEntry>,
}

impl GeneratedManifest {
    pub fn from_tree(tree: &GeneratedTree) -> Self {
        let entries = tree
            .iter()
            .map(|(path, contents)| ManifestEntry {
                path: path.to_string_lossy().replace('\\', "/"),
                bytes: contents.len(),
                sha256: sha256(contents),
            })
            .collect();
        Self {
            version: MANIFEST_VERSION,
            entries,
        }
    }

    pub fn to_pretty_json(&self) -> Result<String> {
        Ok(format!("{}\n", serde_json::to_string_pretty(self)?))
    }

    pub fn as_file(&self, path: impl AsRef<Path>) -> Result<GeneratedFile> {
        GeneratedFile::new(path, self.to_pretty_json()?)
    }
}

fn sha256(contents: &str) -> String {
    let digest = Sha256::digest(contents.as_bytes());
    format!("{digest:x}")
}
