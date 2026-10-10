//! HTTP-specific Ruby lowering, package assembly and source emission.
use crate::*;

mod assembly;
pub(crate) use assembly::*;
mod models;
pub(crate) use models::*;
mod client;
pub(crate) use client::*;
mod operations;
pub(crate) use operations::*;
mod names;
pub(crate) use names::*;
