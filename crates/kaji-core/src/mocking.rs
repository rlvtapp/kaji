//! Typed contracts for OpenAPI-derived mock scenarios.
//!
//! Kaji keeps mock behaviour on an operation's `x-kaji-mock` extension. The
//! extension is intentionally small and transport-neutral, so a generated
//! MSW handler, a standalone HTTP fixture, and a hosted mock environment can
//! all make the same request/response decision.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Api, Operation};

/// One named mock scenario declared on an API operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MockScenario {
    /// Stable operation identifier. This is copied from [`Operation::id`] so
    /// a mock backend does not need to carry the source AST beside fixtures.
    pub operation_id: String,
    /// A human-selectable scenario name, unique within its operation.
    pub name: String,
    #[serde(default)]
    pub when: MockRequestMatch,
    pub response: MockResponse,
}

/// Exact request predicates for a mock scenario.
///
/// Header, query, and path predicates are strings because HTTP transports are
/// string based. `body` deliberately remains JSON to allow a scenario to
/// match an entire structured request body without inventing another query
/// language.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MockRequestMatch {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub query: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub path: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
}

/// The response emitted when a [`MockRequestMatch`] succeeds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MockResponse {
    pub status: u16,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
    /// Optional deterministic delay before sending the response, in
    /// milliseconds. A hard upper bound keeps a typo from stalling CI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_ms: Option<u64>,
}

const EXTENSION: &str = "x-kaji-mock";
const MAX_DELAY_MS: u64 = 600_000;

/// Extracts every valid `x-kaji-mock` scenario from an API.
///
/// Operations without the extension simply produce no scenarios. An invalid
/// extension fails generation with an operation-qualified error; silently
/// ignoring a typo would make a test environment unexpectedly exercise the
/// default response instead of its intended failure path.
pub fn extract_mock_scenarios(api: &Api) -> Result<Vec<MockScenario>> {
    api.operations
        .iter()
        .map(extract_operation_mock_scenarios)
        .collect::<Result<Vec<_>>>()
        .map(|sets| sets.into_iter().flatten().collect())
}

/// Extracts and validates mock scenarios for one operation.
pub fn extract_operation_mock_scenarios(operation: &Operation) -> Result<Vec<MockScenario>> {
    let Some(value) = operation.annotations.get(EXTENSION) else {
        return Ok(Vec::new());
    };
    let object = value.as_object().with_context(|| {
        format!(
            "{EXTENSION} on operation {:?} must be an object",
            operation.id
        )
    })?;
    let scenarios = object
        .get("scenarios")
        .with_context(|| {
            format!(
                "{EXTENSION} on operation {:?} requires scenarios",
                operation.id
            )
        })?
        .as_array()
        .with_context(|| {
            format!(
                "{EXTENSION}.scenarios on operation {:?} must be an array",
                operation.id
            )
        })?;

    let mut names = BTreeSet::new();
    scenarios
        .iter()
        .enumerate()
        .map(|(index, scenario)| {
            parse_scenario(operation, scenario, index).and_then(|scenario| {
                if !names.insert(scenario.name.clone()) {
                    bail!(
                        "{EXTENSION}.scenarios on operation {:?} contains duplicate name {:?}",
                        operation.id,
                        scenario.name
                    );
                }
                Ok(scenario)
            })
        })
        .collect()
}

fn parse_scenario(operation: &Operation, value: &Value, index: usize) -> Result<MockScenario> {
    let context = format!(
        "{EXTENSION}.scenarios[{index}] on operation {:?}",
        operation.id
    );
    let object = value
        .as_object()
        .with_context(|| format!("{context} must be an object"))?;
    reject_unknown(object, &["name", "when", "response"], &context)?;
    let name = required_string(object, "name", &context)?;
    if name.trim().is_empty() {
        bail!("{context}.name must not be empty");
    }
    let when = object
        .get("when")
        .map(|value| parse_match(value, &format!("{context}.when")))
        .transpose()?
        .unwrap_or_default();
    let response = object
        .get("response")
        .with_context(|| format!("{context} requires response"))
        .and_then(|value| parse_response(value, &format!("{context}.response")))?;
    Ok(MockScenario {
        operation_id: operation.id.clone(),
        name: name.to_owned(),
        when,
        response,
    })
}

fn parse_match(value: &Value, context: &str) -> Result<MockRequestMatch> {
    let object = value
        .as_object()
        .with_context(|| format!("{context} must be an object"))?;
    reject_unknown(object, &["headers", "query", "path", "body"], context)?;
    Ok(MockRequestMatch {
        headers: optional_string_map(object, "headers", context)?,
        query: optional_string_map(object, "query", context)?,
        path: optional_string_map(object, "path", context)?,
        body: object.get("body").cloned(),
    })
}

fn parse_response(value: &Value, context: &str) -> Result<MockResponse> {
    let object = value
        .as_object()
        .with_context(|| format!("{context} must be an object"))?;
    reject_unknown(object, &["status", "headers", "body", "delay_ms"], context)?;
    let status = object
        .get("status")
        .with_context(|| format!("{context} requires status"))?
        .as_u64()
        .with_context(|| format!("{context}.status must be an integer"))?;
    if !(100..=599).contains(&status) {
        bail!("{context}.status must be between 100 and 599");
    }
    let delay_ms = object
        .get("delay_ms")
        .map(|value| {
            value
                .as_u64()
                .with_context(|| format!("{context}.delay_ms must be an integer"))
        })
        .transpose()?;
    if delay_ms.is_some_and(|delay| delay > MAX_DELAY_MS) {
        bail!("{context}.delay_ms must not exceed {MAX_DELAY_MS}");
    }
    Ok(MockResponse {
        status: status as u16,
        headers: optional_string_map(object, "headers", context)?,
        body: object.get("body").cloned(),
        delay_ms,
    })
}

fn required_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<&'a str> {
    object
        .get(key)
        .with_context(|| format!("{context} requires {key}"))?
        .as_str()
        .with_context(|| format!("{context}.{key} must be a string"))
}

fn optional_string_map(
    object: &serde_json::Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<BTreeMap<String, String>> {
    let Some(value) = object.get(key) else {
        return Ok(BTreeMap::new());
    };
    let map = value
        .as_object()
        .with_context(|| format!("{context}.{key} must be an object"))?;
    map.iter()
        .map(|(name, value)| {
            if name.is_empty() {
                bail!("{context}.{key} must not contain an empty name");
            }
            let value = value
                .as_str()
                .with_context(|| format!("{context}.{key}.{name} must be a string"))?;
            if (key == "headers") && (name.contains(['\r', '\n']) || value.contains(['\r', '\n'])) {
                bail!("{context}.{key}.{name} must not contain a line break");
            }
            Ok((name.clone(), value.to_owned()))
        })
        .collect()
}

fn reject_unknown(
    object: &serde_json::Map<String, Value>,
    known: &[&str],
    context: &str,
) -> Result<()> {
    if let Some(key) = object.keys().find(|key| !known.contains(&key.as_str())) {
        bail!("{context} contains unsupported field {key:?}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::{HttpMethod, Operation};

    fn operation(extension: Value) -> Operation {
        Operation {
            id: "listContacts".into(),
            method: HttpMethod::Get,
            path: "/v1/contacts/{contact_id}".into(),
            annotations: BTreeMap::from([(EXTENSION.into(), extension)]),
            ..Operation::default()
        }
    }

    #[test]
    fn extracts_typed_scenarios() {
        let scenarios = extract_operation_mock_scenarios(&operation(json!({
            "scenarios": [{
                "name": "rate-limited",
                "when": {
                    "headers": {"x-test-scenario": "rate-limited"},
                    "query": {"expand": "stats"},
                    "path": {"contact_id": "contact_123"},
                    "body": {"enabled": true}
                },
                "response": {
                    "status": 429,
                    "headers": {"retry-after": "1"},
                    "body": {"message": "Too many requests"},
                    "delay_ms": 25
                }
            }]
        })))
        .unwrap();
        assert_eq!(scenarios.len(), 1);
        assert_eq!(scenarios[0].operation_id, "listContacts");
        assert_eq!(scenarios[0].when.path["contact_id"], "contact_123");
        assert_eq!(scenarios[0].response.status, 429);
        assert_eq!(scenarios[0].response.delay_ms, Some(25));
    }

    #[test]
    fn rejects_invalid_and_ambiguous_scenarios() {
        for extension in [
            json!({"scenarios": [{"name": "bad", "response": {"status": 99}}]}),
            json!({"scenarios": [{"name": "bad", "when": {"header": {}}, "response": {"status": 200}}]}),
            json!({"scenarios": [{"name": "same", "response": {"status": 200}}, {"name": "same", "response": {"status": 201}}]}),
            json!({"scenarios": [{"name": "slow", "response": {"status": 200, "delay_ms": 600001}}]}),
        ] {
            assert!(extract_operation_mock_scenarios(&operation(extension)).is_err());
        }
    }

    #[test]
    fn ignores_operations_without_mock_extensions() {
        let api = Api {
            operations: vec![Operation::default()],
            ..Api::default()
        };
        assert!(extract_mock_scenarios(&api).unwrap().is_empty());
    }
}
