//! Owned Protobuf RPC metadata plus the official descriptor wire representation.
//!
//! Message presence, field numbers, oneofs, maps, defaults, custom options and
//! extensions remain in FileDescriptorSet bytes. They are not flattened into
//! HTTP schemas or interpreted as GraphQL nullable fields.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RpcStreaming {
    Unary,
    Client,
    Server,
    Bidirectional,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcMethod {
    pub name: String,
    pub full_name: String,
    pub input_type: String,
    pub output_type: String,
    pub streaming: RpcStreaming,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcService {
    pub full_name: String,
    pub file: String,
    pub methods: Vec<RpcMethod>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcFile {
    pub name: String,
    pub package: String,
    pub syntax: String,
    pub go_package: Option<String>,
    pub imports: Vec<String>,
    /// Imported built-in definitions may have no physical source file.
    pub source: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcContract {
    pub root_files: Vec<String>,
    /// Serialized google.protobuf.FileDescriptorSet, including all dependencies.
    pub descriptor_set: Vec<u8>,
    pub files: Vec<RpcFile>,
    pub services: Vec<RpcService>,
}
impl crate::engine::Contract for RpcContract {
    const NAME: &'static str = "poolster.protobuf-rpc.v1";
}

/// Optional service-method block. Message wire definitions remain in RpcContract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcMethodBlock {
    pub service: String,
    pub file: String,
    pub method: RpcMethod,
}
impl crate::blocks::Block for RpcMethodBlock {
    const CONTRACT_NAME: &'static str = "poolster.rpc-method-blocks.v1";
}
impl RpcContract {
    /// Explicit opt-in projection; opaque RPC contracts remain independently usable.
    /// `source` is the caller's stable document identity, not an output filename.
    /// Local IDs are native fully qualified method coordinates and remain stable
    /// when source files or generated packages move. Input/output references are
    /// fully qualified message coordinates in this contract's descriptor set;
    /// they do not imply that separate message blocks have been published.
    /// The projection is an independent snapshot. Editing blocks does not rewrite
    /// `descriptor_set`, and official code generators consume that authoritative
    /// descriptor set rather than mutations to these optional metadata blocks.
    pub fn method_blocks(
        &self,
        source: impl Into<String>,
    ) -> crate::blocks::Blocks<RpcMethodBlock> {
        use crate::blocks::{BlockId, BlockMetadata, BlockReference, Blocks, BuildingBlock};
        use crate::engine::Contract;
        let source = source.into();
        let mut items = Vec::new();
        for service in &self.services {
            for method in &service.methods {
                let streaming = match method.streaming {
                    RpcStreaming::Unary => "rpc.unary",
                    RpcStreaming::Client => "rpc.client-streaming",
                    RpcStreaming::Server => "rpc.server-streaming",
                    RpcStreaming::Bidirectional => "rpc.bidirectional-streaming",
                };
                let mut capabilities = std::collections::BTreeSet::from([
                    "rpc.method".into(),
                    "protobuf".into(),
                    streaming.into(),
                ]);
                if method.streaming != RpcStreaming::Unary {
                    capabilities.insert("rpc.streaming".into());
                }
                let reference = |name: &str| BlockReference {
                    contract: Self::NAME.into(),
                    id: BlockId {
                        source: source.clone(),
                        local: name.into(),
                    },
                };
                items.push(BuildingBlock {
                    metadata: BlockMetadata {
                        parent: Some(crate::blocks::ContractReference::from_bytes(
                            <Self as crate::engine::Contract>::NAME,
                            source.clone(),
                            &serde_json::to_vec(self).expect("owned contract is serializable"),
                        )),
                        id: BlockId {
                            source: source.clone(),
                            local: method.full_name.clone(),
                        },
                        capabilities,
                        references: vec![
                            reference(&method.input_type),
                            reference(&method.output_type),
                        ],
                        location: Some(method.full_name.clone()),
                    },
                    value: RpcMethodBlock {
                        service: service.full_name.clone(),
                        file: service.file.clone(),
                        method: method.clone(),
                    },
                });
            }
        }
        Blocks {
            parent: Some(crate::blocks::ContractReference::from_bytes(
                <Self as crate::engine::Contract>::NAME,
                source.clone(),
                &serde_json::to_vec(self).expect("owned contract is serializable"),
            )),
            state: crate::blocks::CollectionState::Complete,
            items,
        }
    }
}
