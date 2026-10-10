//! HTTP-specific Python source and package assembly.
use crate::*;

mod assembly;
pub(crate) use assembly::*;
mod asynchronous;
pub(crate) use asynchronous::*;
