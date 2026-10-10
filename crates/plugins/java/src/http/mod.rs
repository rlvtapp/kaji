//! HTTP-specific backend implementation; public factories remain at crate root.
mod assembly;
pub(crate) use assembly::*;
mod facade;
pub(crate) use facade::*;
