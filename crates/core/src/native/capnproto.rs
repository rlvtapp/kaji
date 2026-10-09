//! Owned Cap'n Proto descriptor metadata; wire layout stays in native request bytes.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapnpNodeKind {
    File,
    Struct,
    Enum,
    Interface,
    Constant,
    Annotation,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityMethod {
    pub ordinal: u16,
    pub name: String,
    pub parameters: u64,
    pub results: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapnpNodeBlock {
    pub id: u64,
    pub name: String,
    pub scope: u64,
    pub kind: CapnpNodeKind,
    pub methods: Vec<CapabilityMethod>,
}
impl crate::blocks::Block for CapnpNodeBlock {
    const CONTRACT_NAME: &'static str = "poolster.capnp-node-blocks.v1";
}
