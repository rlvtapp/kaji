//! AsyncAPI inputs retain their original version and native protocol model.
use anyhow::{Context, Result, bail};
use poolster_core::input::{InputOperation as OperationSummary, InputSummary as ContractSummary};
use roas_asyncapi::validation::{Validate, ValidationOptions};
use serde_json::Value;

#[derive(Debug)]
pub enum AsyncApiModel {
    V2_6(roas_asyncapi::v2_6::Document),
    V3_0(roas_asyncapi::v3_0::Document),
    V3_1(roas_asyncapi::v3_1::Document),
}

#[derive(Debug)]
pub struct AsyncApiDocument {
    pub model: AsyncApiModel,
    /// Original document preserves schema dialects, bindings and extensions.
    pub source: Value,
}

pub fn parse(source: &str) -> Result<AsyncApiDocument> {
    let source: Value = serde_yaml_ng::from_str(source).context("invalid AsyncAPI JSON/YAML")?;
    let version = source["asyncapi"]
        .as_str()
        .context("AsyncAPI requires a string asyncapi version")?;
    let options = ValidationOptions::ErrorOnExternalReference.into();
    let model = match version {
        "2.6.0" => {
            let doc: roas_asyncapi::v2_6::Document =
                serde_json::from_value(source.clone()).context("invalid AsyncAPI 2.6 document")?;
            doc.validate(options)
                .context("AsyncAPI validation failed")?;
            AsyncApiModel::V2_6(doc)
        }
        "3.0.0" => {
            let doc: roas_asyncapi::v3_0::Document =
                serde_json::from_value(source.clone()).context("invalid AsyncAPI 3.0 document")?;
            doc.validate(options)
                .context("AsyncAPI validation failed")?;
            AsyncApiModel::V3_0(doc)
        }
        "3.1.0" => {
            let doc: roas_asyncapi::v3_1::Document =
                serde_json::from_value(source.clone()).context("invalid AsyncAPI 3.1 document")?;
            doc.validate(options)
                .context("AsyncAPI validation failed")?;
            AsyncApiModel::V3_1(doc)
        }
        _ => bail!("unsupported AsyncAPI version {version}; supported: 2.6.0, 3.0.0, 3.1.0"),
    };
    Ok(AsyncApiDocument { model, source })
}

impl AsyncApiDocument {
    pub fn summary(&self) -> ContractSummary {
        let mut operations = Vec::new();
        if let Some(entries) = self.source["operations"].as_object() {
            for (name, operation) in entries {
                let operation = resolve(&self.source, operation);
                operations.push(OperationSummary {
                    name: name.clone(),
                    kind: operation["action"].as_str().unwrap_or("operation").into(),
                });
            }
        } else if let Some(channels) = self.source["channels"].as_object() {
            for (channel, value) in channels {
                let value = resolve(&self.source, value);
                for kind in ["publish", "subscribe"] {
                    if let Some(operation) = value.get(kind) {
                        operations.push(OperationSummary {
                            name: operation["operationId"]
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| format!("{channel}:{kind}")),
                            kind: kind.into(),
                        });
                    }
                }
            }
        }
        let mut types: Vec<String> = self.source["components"]["schemas"]
            .as_object()
            .map(|v| v.keys().cloned().collect())
            .unwrap_or_default();
        types.sort();
        operations.sort_by(|a, b| a.name.cmp(&b.name));
        ContractSummary {
            format: "asyncapi".into(),
            title: self.source["info"]["title"]
                .as_str()
                .unwrap_or_default()
                .into(),
            version: self.source["info"]["version"].as_str().map(str::to_owned),
            types,
            operations,
        }
    }
}

// Validators reject unresolved/cyclic references; this read-only view follows
// local operation aliases for summary names without modifying the native tree.
fn resolve<'a>(root: &'a Value, mut value: &'a Value) -> &'a Value {
    let mut seen = std::collections::HashSet::new();
    while let Some(reference) = value["$ref"].as_str() {
        if !seen.insert(reference) {
            break;
        }
        match reference
            .strip_prefix('#')
            .and_then(|pointer| root.pointer(pointer))
        {
            Some(target) => value = target,
            None => break,
        }
    }
    value
}

impl poolster_core::engine::Contract for AsyncApiDocument {
    const NAME: &'static str = "poolster.asyncapi";
}

/// Native asyncapi input provider.
pub struct AsyncApiInput;
impl poolster_core::input::InputPlugin for AsyncApiInput {
    fn id(&self) -> &str {
        "asyncapi.roas"
    }
    fn format(&self) -> &str {
        "asyncapi"
    }
    fn load(&self, path: &std::path::Path) -> anyhow::Result<poolster_core::input::InputContract> {
        let document = parse(
            &std::fs::read_to_string(path)
                .with_context(|| format!("cannot read contract {}", path.display()))?,
        )?;
        let mut input = poolster_core::input::InputContract::new(document.summary());

        input.publish(document)?;
        Ok(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const EXAMPLE: &str = include_str!("../tests/fixtures/events.yaml");
    #[test]
    fn retains_channels_messages_and_bindings() {
        let doc = parse(EXAMPLE).unwrap();
        assert_eq!(
            doc.source["channels"]["orders"]["bindings"]["kafka"]["topic"],
            "orders"
        );
        assert_eq!(
            doc.source["channels"]["orders"]["messages"]["created"]["payload"]["properties"]["id"]
                ["type"],
            "string"
        );
        assert_eq!(doc.summary().operations[0].kind, "send");
        assert!(matches!(doc.model, AsyncApiModel::V3_0(_)));
    }
    #[test]
    fn rejects_external_and_wrong_kind_references() {
        let external = EXAMPLE.replace("#/channels/orders", "other.yaml#/channels/orders");
        assert!(format!("{:#}", parse(&external).unwrap_err()).contains("external"));
        let wrong = EXAMPLE.replace("#/channels/orders", "#/components/schemas/Order");
        assert!(parse(&wrong).is_err());
    }
    #[test]
    fn supports_all_declared_versions() {
        for version in ["2.6.0", "3.0.0", "3.1.0"] {
            let input = format!(
                "asyncapi: {version}\ninfo:\n  title: Events\n  version: '1.0'\nchannels: {{}}\n"
            );
            parse(&input).unwrap();
        }
        assert!(parse("asyncapi: 4.0.0").is_err());
        assert!(parse("asyncapi: 3.0.0\ninfo: {title: '', version: '1'}").is_err());
    }
}
