//! Typed transport metadata for OpenAPI content, sequential data and multipart.
use crate::{Operation, OperationParameter, SchemaValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ContentDefinition {
    #[serde(default)]
    pub content_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_definition: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_schema_definition: Option<Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub encoding: BTreeMap<String, Encoding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prefix_encoding: Vec<Encoding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_encoding: Option<Encoding>,
}
impl ContentDefinition {
    pub fn schema(&self) -> Option<SchemaValue> {
        self.schema_definition
            .as_ref()
            .map(crate::adapter::openapi_sidecar::convert_value)
    }
    pub fn item_schema(&self) -> Option<SchemaValue> {
        self.item_schema_definition
            .as_ref()
            .map(crate::adapter::openapi_sidecar::convert_value)
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Encoding {
    #[serde(
        default,
        rename = "contentType",
        skip_serializing_if = "Option::is_none"
    )]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, PartHeader>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explode: Option<bool>,
    #[serde(default, rename = "allowReserved")]
    pub allow_reserved: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub encoding: BTreeMap<String, Encoding>,
    #[serde(
        default,
        rename = "prefixEncoding",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub prefix_encoding: Vec<Encoding>,
    #[serde(
        default,
        rename = "itemEncoding",
        skip_serializing_if = "Option::is_none"
    )]
    pub item_encoding: Option<Box<Encoding>>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PartHeader {
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub style: Option<String>,
    #[serde(default)]
    pub explode: Option<bool>,
    #[serde(default, rename = "allowReserved")]
    pub allow_reserved: bool,
    #[serde(default)]
    pub schema_definition: Option<Value>,
    #[serde(default)]
    pub example_json: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseContentDefinition {
    pub status: String,
    #[serde(flatten)]
    pub content: ContentDefinition,
}

fn read<T: for<'de> Deserialize<'de>>(
    annotations: &BTreeMap<String, Value>,
    key: &str,
) -> serde_json::Result<Vec<T>> {
    annotations
        .get(key)
        .map(|value| serde_json::from_value(value.clone()))
        .unwrap_or_else(|| Ok(Vec::new()))
}
pub fn request_content(operation: &Operation) -> serde_json::Result<Vec<ContentDefinition>> {
    read(&operation.annotations, "kaji.request_content")
}
pub fn response_content(
    operation: &Operation,
) -> serde_json::Result<Vec<ResponseContentDefinition>> {
    read(&operation.annotations, "kaji.response_content")
}
pub fn parameter_content(
    parameter: &OperationParameter,
) -> serde_json::Result<Vec<ContentDefinition>> {
    read(&parameter.annotations, "kaji.parameter_content")
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ApiMetadata {
    #[serde(default)]
    #[serde(rename = "self")]
    pub self_uri: Option<String>,
    #[serde(default)]
    pub tags: Vec<TagMetadata>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TagMetadata {
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
}
