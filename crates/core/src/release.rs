//! Optional package metadata for build and release orchestration.
//!
//! Commands are argument vectors, never interpolated shell programs. The plugin
//! works with community languages and does not teach core about registries.

use std::{collections::BTreeMap, marker::PhantomData};

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    GeneratedFile,
    engine::{Contract, Enforce, Language, Meta, Plugin, PluginContext, Provision},
};

pub const PACKAGE_METADATA_PATH: &str = ".poolster/package.json";
pub const PACKAGE_METADATA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageCommand {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
}

impl PackageCommand {
    pub fn new(
        program: impl Into<String>,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.program.is_empty() && !self.program.starts_with('-'),
            "package command needs an executable name"
        );
        ensure!(
            !self.program.contains(['\0', '\n', '\r']),
            "package executable contains a control character"
        );
        ensure!(
            self.args.iter().all(|arg| !arg.contains('\0')),
            "package argument contains a NUL byte"
        );
        Ok(())
    }
}

/// A registry integration declares its commands and release-please strategy.
/// Third-party plugins may name any registry; core has no registry enum.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackagePublisher {
    pub registry: String,
    pub release_type: String,
    #[serde(default)]
    pub commands: Vec<PackageCommand>,
    #[serde(default)]
    pub extra_files: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageMetadata {
    #[serde(default = "metadata_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub build: Vec<PackageCommand>,
    #[serde(default)]
    pub test: Vec<PackageCommand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<PackagePublisher>,
    /// Capability -> verification level, e.g. `serialization: wire-tested`.
    #[serde(default)]
    pub capabilities: BTreeMap<String, String>,
}

fn metadata_version() -> u32 {
    PACKAGE_METADATA_VERSION
}

impl PackageMetadata {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            schema_version: PACKAGE_METADATA_VERSION,
            language: String::new(),
            name: name.into(),
            version: String::new(),
            build: vec![],
            test: vec![],
            publisher: None,
            capabilities: BTreeMap::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == PACKAGE_METADATA_VERSION,
            "unsupported package metadata version {}",
            self.schema_version
        );
        for (label, value) in [
            ("name", &self.name),
            ("language", &self.language),
            ("version", &self.version),
        ] {
            ensure!(
                !value.trim().is_empty() && !value.contains(['\0', '\n', '\r']),
                "invalid package {label}"
            );
        }
        for command in self.build.iter().chain(&self.test).chain(
            self.publisher
                .iter()
                .flat_map(|publisher| &publisher.commands),
        ) {
            command.validate()?;
        }
        if let Some(publisher) = &self.publisher {
            ensure!(
                !publisher.registry.trim().is_empty() && !publisher.release_type.trim().is_empty(),
                "publisher requires registry and release_type"
            );
            ensure!(
                !publisher.commands.is_empty()
                    || matches!(
                        publisher.registry.as_str(),
                        "npm" | "pypi" | "crates.io" | "go"
                    ),
                "custom publisher requires at least one command"
            );
            for path in &publisher.extra_files {
                GeneratedFile::new(path, "")?;
            }
        }
        Ok(())
    }

    pub fn to_json(&self) -> Result<String> {
        self.validate()?;
        Ok(format!("{}\n", serde_json::to_string_pretty(self)?))
    }
}

impl Contract for PackageMetadata {
    const NAME: &'static str = "poolster.package-metadata";
}

pub struct Metadata<L: Language> {
    meta: Meta,
    value: PackageMetadata,
    language: PhantomData<L>,
}

/// Attach metadata explicitly to any language package. API/package versions
/// default from the normalized input after package settings have been applied.
pub fn metadata<L: Language>(value: PackageMetadata) -> Metadata<L> {
    Metadata {
        meta: Meta::new(),
        value,
        language: PhantomData,
    }
}

impl<L: Language> Plugin<L> for Metadata<L> {
    fn supports_native_input(&self) -> bool {
        true
    }
    fn kind(&self) -> &'static str {
        "package-metadata"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn enforce(&self) -> Enforce {
        Enforce::Post
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<PackageMetadata>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, L>) -> Result<()> {
        let mut value = self.value.clone();
        if value.language.is_empty() {
            value.language = L::NAME.into();
        }
        if value.version.is_empty() {
            value.version = cx
                .common
                .package_version
                .clone()
                .unwrap_or_else(|| cx.api.version.clone());
        }
        cx.files
            .emit(GeneratedFile::new(PACKAGE_METADATA_PATH, value.to_json()?)?)?;
        cx.publish(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Api,
        engine::{Package, Packages},
    };

    struct Community;
    impl Language for Community {
        const NAME: &'static str = "community";
        type Settings = ();
        type Workspace = ();
    }

    #[test]
    fn native_metadata_uses_explicit_package_version_without_http_input() {
        let tree = Packages::new()
            .package(
                Package::<Community>::new("native")
                    .common(crate::engine::Common::default().package_version("1.2.3"))
                    .with(metadata::<Community>(PackageMetadata::new("native-sdk"))),
            )
            .generate_native()
            .unwrap();
        let value: PackageMetadata =
            serde_json::from_str(tree.get("native/.poolster/package.json").unwrap()).unwrap();
        assert_eq!(value.language, "community");
        assert_eq!(value.version, "1.2.3");
    }

    #[test]
    fn works_for_a_community_language_with_its_own_registry() {
        let mut value = PackageMetadata::new("community-sdk");
        value.build.push(PackageCommand::new(
            "custom-compiler",
            ["compile", "$(literal)"],
        ));
        value.publisher = Some(PackagePublisher {
            registry: "community-registry".into(),
            release_type: "simple".into(),
            commands: vec![PackageCommand::new("community-publish", ["--release"])],
            extra_files: vec!["version.txt".into()],
        });
        let tree = Packages::new()
            .package(Package::<Community>::new("sdk").with(metadata(value)))
            .generate(
                &Api {
                    version: "1.2.3".into(),
                    ..Default::default()
                },
                None,
            )
            .unwrap();
        let value: PackageMetadata =
            serde_json::from_str(tree.get("sdk/.poolster/package.json").unwrap()).unwrap();
        assert_eq!(value.language, "community");
        assert_eq!(value.version, "1.2.3");
        assert_eq!(value.build[0].args[1], "$(literal)");
        assert_eq!(value.publisher.unwrap().registry, "community-registry");
    }

    #[test]
    fn rejects_invalid_versions_commands_and_escaping_release_files() {
        let mut value = PackageMetadata::new("sdk");
        value.language = "custom".into();
        value.version = "1.0.0".into();
        value.schema_version = 2;
        assert!(value.validate().is_err());
        value.schema_version = 1;
        value
            .build
            .push(PackageCommand::new("", Vec::<String>::new()));
        assert!(value.validate().is_err());
        value.build.clear();
        value.publisher = Some(PackagePublisher {
            registry: "custom".into(),
            release_type: "simple".into(),
            commands: vec![PackageCommand::new("publish", Vec::<String>::new())],
            extra_files: vec!["../secret".into()],
        });
        assert!(value.validate().is_err());
    }
}
