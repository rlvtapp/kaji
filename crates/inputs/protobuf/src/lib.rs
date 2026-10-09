//! Protobuf compilation preserves descriptors, imports, and RPC streaming semantics.
use anyhow::{Context, Result};
use poolster_core::input::{InputOperation as OperationSummary, InputSummary as ContractSummary};
pub mod blocks;
pub mod contracts;

use contracts::{RpcContract, RpcFile, RpcMethod, RpcService, RpcStreaming};
use protox::prost_reflect::DescriptorPool;
use std::{collections::BTreeMap, path::Path};

#[derive(Debug, Clone)]
pub struct ProtobufDocument {
    pub descriptors: DescriptorPool,
    pub root_file: String,
    pub sources: BTreeMap<String, String>,
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
    let mut roots = includes
        .iter()
        .map(|root| {
            root.canonicalize()
                .with_context(|| format!("reading Protobuf import root {}", root.display()))
        })
        .collect::<Result<Vec<_>>>()?;
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
    let sources = compiler
        .files()
        .filter_map(|file| file.path().map(|path| (file.name(), path)))
        .map(|(name, path)| {
            Ok((
                name.to_owned(),
                std::fs::read_to_string(path)
                    .with_context(|| format!("retaining Protobuf source {}", path.display()))?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    Ok(ProtobufDocument {
        sources,
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
    /// Lower into Poolster-owned RPC metadata; descriptor bytes retain all wire details.
    pub fn rpc_contract(&self) -> RpcContract {
        let mut files: Vec<_> = self
            .descriptors
            .files()
            .map(|file| RpcFile {
                name: file.name().into(),
                package: file.package_name().into(),
                syntax: file
                    .file_descriptor_proto()
                    .syntax
                    .clone()
                    .unwrap_or_else(|| "proto2".into()),
                go_package: file
                    .file_descriptor_proto()
                    .options
                    .as_ref()
                    .and_then(|o| o.go_package.clone()),
                imports: file.dependencies().map(|f| f.name().to_owned()).collect(),
                source: self.sources.get(file.name()).cloned(),
            })
            .collect();
        files.sort_by(|a, b| a.name.cmp(&b.name));
        let mut services: Vec<_> = self
            .descriptors
            .services()
            .map(|service| RpcService {
                full_name: service.full_name().into(),
                file: service.parent_file().name().into(),
                methods: service
                    .methods()
                    .map(|method| RpcMethod {
                        name: method.name().into(),
                        full_name: method.full_name().into(),
                        input_type: method.input().full_name().into(),
                        output_type: method.output().full_name().into(),
                        streaming: match (
                            method.is_client_streaming(),
                            method.is_server_streaming(),
                        ) {
                            (false, false) => RpcStreaming::Unary,
                            (true, false) => RpcStreaming::Client,
                            (false, true) => RpcStreaming::Server,
                            (true, true) => RpcStreaming::Bidirectional,
                        },
                    })
                    .collect(),
            })
            .collect();
        services.sort_by(|a, b| a.full_name.cmp(&b.full_name));
        RpcContract {
            root_files: vec![self.root_file.clone()],
            descriptor_set: self.descriptors.encode_to_vec(),
            files,
            services,
        }
    }

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

        let rpc = document.rpc_contract();
        let source = path.canonicalize()?.to_string_lossy().replace('\\', "/");
        input.publish(blocks::method_blocks(&rpc, source.clone()))?;
        input.publish_with_reference(
            rpc.clone(),
            poolster_core::blocks::ContractReference::from_bytes(
                <poolster_core::native::rpc::RpcContract as poolster_core::engine::Contract>::NAME,
                source.clone(),
                &serde_json::to_vec(&rpc)?,
            ),
        )?;
        input.publish(document)?;
        Ok(input)
    }
    fn load_with_options(
        &self,
        path: &Path,
        options: &poolster_core::input::InputOptions,
    ) -> Result<poolster_core::input::InputContract> {
        anyhow::ensure!(
            options.operation_files.is_empty()
                && options.broker.is_none()
                && options.workflow_sources.is_empty(),
            "Protobuf input supports import_roots only"
        );
        let document = load_with_includes(path, &options.import_roots)?;
        let mut input = poolster_core::input::InputContract::new(document.summary());
        let rpc = document.rpc_contract();
        let source = path.canonicalize()?.to_string_lossy().replace('\\', "/");
        input.publish(blocks::method_blocks(&rpc, source.clone()))?;
        input.publish_with_reference(
            rpc.clone(),
            poolster_core::blocks::ContractReference::from_bytes(
                <poolster_core::native::rpc::RpcContract as poolster_core::engine::Contract>::NAME,
                source.clone(),
                &serde_json::to_vec(&rpc)?,
            ),
        )?;
        input.publish(document)?;
        Ok(input)
    }
}
