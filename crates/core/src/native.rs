//! Poolster-owned interchange contracts. Parser implementation types stay in inputs.
pub mod capnproto;
pub mod events;
mod graphql_blocks;
pub mod rpc;
pub mod workflows;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
/// Nullability belongs to each wrapper; presence belongs to its containing field.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelType {
    pub nullable: bool,
    pub kind: ModelKind,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ModelKind {
    Scalar(String),
    Enum(Vec<String>),
    Named(String),
    List(Box<ModelType>),
    Object(Vec<ModelField>),
    Union(Vec<ModelType>),
    Literal(String),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelField {
    pub name: String,
    pub ty: ModelType,
    pub optional: bool,
    pub default_value: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphqlOperationKind {
    Query,
    Mutation,
    Subscription,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphqlOperation {
    pub name: String,
    pub kind: GraphqlOperationKind,
    pub document: String,
    pub variables: Vec<ModelField>,
    pub result: ModelType,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphqlOperations {
    pub schema_source: String,
    pub operation_source: String,
    pub operations: Vec<GraphqlOperation>,
    pub input_objects: BTreeMap<String, Vec<ModelField>>,
}
impl crate::engine::Contract for GraphqlOperations {
    const NAME: &'static str = "poolster.graphql-operations.v1";
}
