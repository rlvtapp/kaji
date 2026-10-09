//! Protobuf-owned public contract surface, independent of parser library types.
//!
//! The definitions are shared with consumers through Poolster core; reexports
//! preserve their existing typed graph identities.
pub use poolster_core::native::rpc::{RpcContract, RpcFile, RpcMethod, RpcService, RpcStreaming};
