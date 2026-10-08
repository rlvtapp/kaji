//! Protobuf compilation preserves descriptors, imports, and RPC streaming semantics.
use anyhow::{Context, Result};
use poolster_core::input::{InputOperation as OperationSummary, InputSummary as ContractSummary};
use protox::prost_reflect::DescriptorPool;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ProtobufDocument {
    pub descriptors: DescriptorPool,
    pub root_file: String,
}

pub fn load(path: &Path) -> Result<ProtobufDocument> {
    load_with_includes(path, &[])
}

/// Compile with explicit import roots, searched before the root's directory.
pub fn load_with_includes(
    path: &Path,
    includes: &[std::path::PathBuf],
) -> Result<ProtobufDocument> {
    let path = path
        .canonicalize()
        .with_context(|| format!("reading Protobuf input {}", path.display()))?;
    let parent = path
        .parent()
        .context("Protobuf input needs a parent directory")?;
    let mut roots = includes.to_vec();
    roots.push(parent.to_path_buf());
    let mut compiler = protox::Compiler::new(&roots)?;
    compiler.open_file(&path).map_err(|error| {
        if error.to_string().contains("found 'edition'") {
            anyhow::anyhow!(error).context("Protobuf editions are not supported by the current protox parser; use proto2/proto3 inputs")
        } else { anyhow::anyhow!(error) }
    }).with_context(|| {
        format!(
            "compiling Protobuf input {} (imports resolve relative to its directory)",
            path.display()
        )
    })?;
    Ok(ProtobufDocument {
        descriptors: compiler.descriptor_pool(),
        root_file: compiler
            .files()
            .find(|file| file.path() == Some(path.as_path()))
            .context("compiled root metadata missing")?
            .name()
            .to_owned(),
    })
}

impl ProtobufDocument {
    pub fn summary(&self) -> ContractSummary {
        let root = self
            .descriptors
            .get_file_by_name(&self.root_file)
            .expect("compiled root descriptor exists");
        let mut types: Vec<_> = self
            .descriptors
            .all_messages()
            .filter(|m| !m.is_map_entry())
            .map(|m| m.full_name().to_owned())
            .chain(
                self.descriptors
                    .all_enums()
                    .map(|e| e.full_name().to_owned()),
            )
            .chain(
                self.descriptors
                    .services()
                    .map(|s| s.full_name().to_owned()),
            )
            .collect();
        types.sort();
        let mut operations: Vec<_> = self
            .descriptors
            .services()
            .flat_map(|service| {
                service
                    .methods()
                    .map(|method| {
                        let kind =
                            match (method.is_client_streaming(), method.is_server_streaming()) {
                                (false, false) => "unary",
                                (true, false) => "client_streaming",
                                (false, true) => "server_streaming",
                                (true, true) => "bidirectional_streaming",
                            };
                        OperationSummary {
                            name: method.full_name().to_owned(),
                            kind: kind.into(),
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        operations.sort_by(|a, b| a.name.cmp(&b.name));
        ContractSummary {
            format: "protobuf".into(),
            title: if root.package_name().is_empty() {
                self.root_file.clone()
            } else {
                root.package_name().into()
            },
            version: None,
            types,
            operations,
        }
    }
}

impl poolster_core::engine::Contract for ProtobufDocument {
    const NAME: &'static str = "poolster.protobuf";
}

/// Native protobuf input provider.
pub struct ProtobufInput;
impl poolster_core::input::InputPlugin for ProtobufInput {
    fn id(&self) -> &str {
        "protobuf.protox"
    }
    fn format(&self) -> &str {
        "protobuf"
    }
    fn load(&self, path: &std::path::Path) -> anyhow::Result<poolster_core::input::InputContract> {
        let document = load(path)?;
        let mut input = poolster_core::input::InputContract::new(document.summary());

        input.publish(document)?;
        Ok(input)
    }
}
