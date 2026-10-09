//! Optional typed projections of Protobuf RPC services.
//!
//! Method blocks retain service/file context. Message wire semantics remain in
//! the authoritative descriptor bytes in [`crate::contracts::RpcContract`].
pub use poolster_core::blocks::{
    Block, BlockId, BlockMetadata, BlockReference, Blocks, BuildingBlock,
};
pub use poolster_core::native::rpc::RpcMethodBlock;

/// The method-block contract published directly by [`crate::ProtobufInput`].
pub type RpcMethodBlocks = Blocks<RpcMethodBlock>;

/// Project RPC metadata using an application-owned stable source identity.
/// Editing the returned snapshot does not rewrite the RPC descriptor set.
pub fn method_blocks(
    contract: &crate::contracts::RpcContract,
    source: impl Into<String>,
) -> RpcMethodBlocks {
    contract.method_blocks(source)
}
