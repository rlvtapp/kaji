use super::*;
pub use poolster_core::native::capnproto::{CapabilityMethod, CapnpNodeBlock, CapnpNodeKind};
pub type NodeBlocks = poolster_core::blocks::Blocks<CapnpNodeBlock>;
use poolster_core::blocks::{Block, BlockId, BlockMetadata, BlockReference, Blocks, BuildingBlock};
impl CapnProtoDocument {
    pub fn node_blocks(&self, source: impl Into<String>) -> Result<Blocks<CapnpNodeBlock>> {
        let source = source.into();
        let message = capnp::serialize::read_message(
            &mut Cursor::new(&self.schema_request),
            capnp::message::ReaderOptions::new(),
        )?;
        let request = message.get_root::<code_generator_request::Reader<'_>>()?;
        let mut items = Vec::new();
        for schema in request.get_nodes()? {
            let mut methods = Vec::new();
            let kind = match schema.which()? {
                node::File(_) => CapnpNodeKind::File,
                node::Struct(_) => CapnpNodeKind::Struct,
                node::Enum(_) => CapnpNodeKind::Enum,
                node::Interface(interface) => {
                    for (ordinal, method) in interface.get_methods()?.iter().enumerate() {
                        methods.push(CapabilityMethod {
                            ordinal: ordinal.try_into()?,
                            name: method.get_name()?.to_str()?.into(),
                            parameters: method.get_param_struct_type(),
                            results: method.get_result_struct_type(),
                        });
                    }
                    CapnpNodeKind::Interface
                }
                node::Const(_) => CapnpNodeKind::Constant,
                node::Annotation(_) => CapnpNodeKind::Annotation,
            };
            let mut references = std::collections::BTreeSet::new();
            if schema.get_scope_id() != 0 {
                references.insert(schema.get_scope_id());
            }
            for method in &methods {
                references.insert(method.parameters);
                references.insert(method.results);
            }
            references.remove(&0);
            let tag = match kind {
                CapnpNodeKind::Interface => "rpc.capability",
                CapnpNodeKind::Struct | CapnpNodeKind::Enum => "model",
                _ => "capnp.metadata",
            };
            items.push(BuildingBlock {
                metadata: BlockMetadata {
                    parent: Some(poolster_core::blocks::ContractReference::from_bytes(
                        <Self as poolster_core::engine::Contract>::NAME,
                        source.clone(),
                        &self.schema_request,
                    )),
                    id: BlockId {
                        source: source.clone(),
                        local: format!("{:016x}", schema.get_id()),
                    },
                    capabilities: ["wire.capnp".into(), tag.into()].into(),
                    references: references
                        .into_iter()
                        .map(|id| BlockReference {
                            contract: CapnpNodeBlock::CONTRACT_NAME.into(),
                            id: BlockId {
                                source: source.clone(),
                                local: format!("{id:016x}"),
                            },
                        })
                        .collect(),
                    location: Some(schema.get_display_name()?.to_str()?.into()),
                },
                value: CapnpNodeBlock {
                    id: schema.get_id(),
                    name: schema.get_display_name()?.to_str()?.into(),
                    scope: schema.get_scope_id(),
                    kind,
                    methods,
                },
            });
        }
        let blocks = Blocks {
            parent: None,
            state: poolster_core::blocks::CollectionState::Complete,
            items,
        };
        blocks.validate()?;
        Ok(blocks)
    }
}
