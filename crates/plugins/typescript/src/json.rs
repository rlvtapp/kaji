//! Schema-directed lossless JSON plans shared by generated transports.
use crate::{Int64Type, ModelOptions};
use poolster_core::{AdditionalProperties, Api, SchemaKind, SchemaValue};
use serde_json::{Value, json};

mod plans;
pub(crate) use plans::*;

pub(crate) const RUNTIME: &str = include_str!("json/runtime.ts.tmpl");

mod traversal;
pub(crate) use traversal::*;

#[cfg(test)]
mod tests;
