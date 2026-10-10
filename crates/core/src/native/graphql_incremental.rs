//! Incremental selections are a distinct execution capability, never plain HTTP results.
use super::GraphqlOperations;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphqlIncrementalDialect {
    /// Path-based multipart dialect negotiated using deferSpec=20220824.
    DeferSpec20220824,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphqlIncrementalKind {
    Defer,
    Stream,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphqlIncrementalCondition {
    Always,
    Variable(String),
    Never,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphqlIncrementalSelection {
    pub kind: GraphqlIncrementalKind,
    /// Response aliases, with `*` for each list item coordinate.
    pub path: Vec<String>,
    pub label: Option<String>,
    pub condition: GraphqlIncrementalCondition,
    /// GraphQL syntax, preserving literal or variable initialCount.
    pub initial_count: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphqlIncrementalOperations {
    pub dialect: GraphqlIncrementalDialect,
    /// Final selection shape; intermediate snapshots are recursively partial.
    pub definition: GraphqlOperations,
    pub selections: BTreeMap<String, Vec<GraphqlIncrementalSelection>>,
}
impl crate::engine::Contract for GraphqlIncrementalOperations {
    const NAME: &'static str = "poolster.graphql-incremental-operations.v1";
}
