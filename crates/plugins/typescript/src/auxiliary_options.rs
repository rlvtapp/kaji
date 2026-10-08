//! Typed author controls for deterministic fixtures and executable smoke suites.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

fn default_depth() -> usize {
    8
}
fn default_attempts() -> usize {
    64
}
fn default_timeout() -> u64 {
    30_000
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FixtureOptions {
    pub seed: Option<u64>,
    pub max_depth: usize,
    pub max_attempts: usize,
    /// Explicit component fixtures for schemas that need domain-specific data.
    pub overrides: BTreeMap<String, Value>,
}
impl Default for FixtureOptions {
    fn default() -> Self {
        Self {
            seed: None,
            max_depth: default_depth(),
            max_attempts: default_attempts(),
            overrides: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CypressOptions {
    pub base_url: Option<String>,
    pub headers: BTreeMap<String, String>,
    /// Mutating operations require an explicit opt-in or per-operation enable.
    pub include_mutations: bool,
    pub timeout_ms: u64,
    pub operation_overrides: BTreeMap<String, CypressOperationOptions>,
}
impl Default for CypressOptions {
    fn default() -> Self {
        Self {
            base_url: None,
            headers: BTreeMap::new(),
            include_mutations: false,
            timeout_ms: default_timeout(),
            operation_overrides: BTreeMap::new(),
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CypressOperationOptions {
    pub enabled: Option<bool>,
    pub path: Option<String>,
    pub query: BTreeMap<String, Value>,
    pub headers: BTreeMap<String, String>,
    pub body: Option<Value>,
    pub expected_statuses: Vec<u16>,
}
