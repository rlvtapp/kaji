use super::*;
use poolster_core::{Operation, OperationParameter, OperationResponse};
fn arguments(value: &str) -> Vec<OsString> {
    value.split_whitespace().map(Into::into).collect()
}

mod checking;
mod configuration;
mod generation;
mod parsing;
mod recipes;
mod schema;
