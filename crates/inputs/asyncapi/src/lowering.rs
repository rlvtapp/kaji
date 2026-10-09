mod support;
use support::{bindings, fixed_string};
pub(crate) use support::{location, pointer_segment, resolve, schema};

use super::{AsyncApiDocument, contracts::*};
use anyhow::{Context, Result, bail, ensure};
use poolster_core::{SchemaKind, input::InputOptions};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub fn lower(document: &AsyncApiDocument, options: &InputOptions) -> Result<EventOperations> {
    ensure!(
        options.operation_files.is_empty()
            && options.import_roots.is_empty()
            && options.workflow_sources.is_empty(),
        "AsyncAPI generation only supports broker input options"
    );
    let root = &document.source;
    ensure!(
        matches!(root["asyncapi"].as_str(), Some("3.0.0" | "3.1.0")),
        "Kafka generation supports AsyncAPI 3.0/3.1; 2.6 remains inspection-only"
    );
    let mut hosts = Vec::new();
    let mut server_bindings = json!({});
    if let Some(servers) = root["servers"].as_object() {
        for raw in servers.values() {
            let server = resolve(root, raw)?;
            ensure!(
                server["protocol"] == "kafka",
                "only plaintext Kafka servers are supported"
            );
            ensure!(
                server.get("security").is_none() && server.get("variables").is_none(),
                "server security and variables are unsupported"
            );
            server_bindings = bindings(server.get("bindings"), &["bindingVersion"])?;
            hosts.push(
                server["host"]
                    .as_str()
                    .context("Kafka server host missing")?
                    .to_owned(),
            );
        }
    }
    let (brokers, client_id) = if let Some(config) = &options.broker {
        let config = config
            .as_object()
            .context("Kafka broker configuration must be an object")?;
        ensure!(
            config
                .keys()
                .all(|k| matches!(k.as_str(), "kind" | "brokers" | "client_id")),
            "unknown Kafka broker option"
        );
        ensure!(
            config.get("kind").and_then(Value::as_str) == Some("kafka"),
            "broker.kind must be kafka"
        );
        let brokers = config
            .get("brokers")
            .and_then(Value::as_array)
            .context("broker.brokers must be an array")?
            .iter()
            .map(|b| {
                b.as_str()
                    .map(str::to_owned)
                    .context("broker address must be string")
            })
            .collect::<Result<Vec<_>>>()?;
        let client_id = config
            .get("client_id")
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .context("client_id must be string")
            })
            .transpose()?;
        (brokers, client_id)
    } else {
        (hosts, None)
    };
    ensure!(
        !brokers.is_empty()
            && brokers
                .iter()
                .all(|v| !v.is_empty() && !v.contains('{') && !v.contains("://")),
        "Kafka requires explicit broker host:port addresses"
    );
    ensure!(
        brokers.iter().all(
            |address| address.rsplit_once(':').is_some_and(
                |(host, port)| !host.is_empty() && port.parse::<u16>().is_ok_and(|p| p > 0)
            )
        ),
        "Kafka broker addresses require nonzero host:port"
    );
    let mut operations = Vec::new();
    for (name, raw) in root["operations"]
        .as_object()
        .context("AsyncAPI operations missing")?
    {
        let op = resolve(root, raw)?;
        ensure!(
            op.get("traits").is_none() && op.get("reply").is_none() && op.get("security").is_none(),
            "operation traits, replies and security unsupported"
        );
        let action = match op["action"].as_str() {
            Some("send") => EventAction::Send,
            Some("receive") => EventAction::Receive,
            _ => bail!("unsupported event action"),
        };
        let channel_ref = op["channel"]["$ref"]
            .as_str()
            .context("operation channel must be local reference")?;
        let channel = resolve(root, &op["channel"])?;
        ensure!(
            channel.get("parameters").is_none(),
            "dynamic channel parameters unsupported"
        );
        let address = channel["address"]
            .as_str()
            .context("Kafka channel needs static address")?;
        ensure!(
            !address.is_empty()
                && address.len() <= 249
                && address
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                && !matches!(address, "." | ".."),
            "unsupported Kafka topic address {address}"
        );
        let channel_bindings = bindings(channel.get("bindings"), &["topic", "bindingVersion"])?;
        if let Some(topic) = channel_bindings["topic"].as_str() {
            ensure!(
                topic == address,
                "Kafka binding topic differs from channel address"
            );
        }
        let op_bindings = bindings(
            op.get("bindings"),
            &["groupId", "clientId", "bindingVersion"],
        )?;
        let group_id = fixed_string(op_bindings.get("groupId"))?;
        let op_client_id = fixed_string(op_bindings.get("clientId"))?;
        let channel_location = location(root, &op["channel"], channel_ref)?;
        let candidates = if let Some(messages) = op["messages"].as_array() {
            messages
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    (
                        format!("#/operations/{}/messages/{index}", pointer_segment(name)),
                        value,
                    )
                })
                .collect::<Vec<_>>()
        } else {
            channel["messages"]
                .as_object()
                .context("channel messages missing")?
                .iter()
                .map(|(key, value)| {
                    (
                        format!("{channel_location}/messages/{}", pointer_segment(key)),
                        value,
                    )
                })
                .collect()
        };
        ensure!(
            candidates.len() == 1,
            "Kafka generation requires exactly one message per operation"
        );
        let message_location = location(root, candidates[0].1, &candidates[0].0)?;
        let message = resolve(root, candidates[0].1)?;
        ensure!(
            message.get("traits").is_none() && message.get("correlationId").is_none(),
            "message traits and correlation identifiers unsupported"
        );
        let format = message.get("schemaFormat").and_then(Value::as_str);
        ensure!(
            format.is_none()
                || matches!(
                    format,
                    Some(
                        "application/schema+json;version=draft-07"
                            | "application/vnd.aai.asyncapi+json;version=3.0.0"
                            | "application/vnd.aai.asyncapi+json;version=3.1.0"
                    )
                ),
            "unsupported message schema format"
        );
        ensure!(
            message
                .get("contentType")
                .or_else(|| root.get("defaultContentType"))
                .is_none_or(|v| v == "application/json"),
            "only JSON messages supported"
        );
        let message_bindings = bindings(message.get("bindings"), &["key", "bindingVersion"])?;
        let (payload, payload_schema) = schema(
            root,
            message.get("payload").context("message payload missing")?,
            &mut BTreeSet::new(),
        )?;
        let headers = message
            .get("headers")
            .map(|v| schema(root, v, &mut BTreeSet::new()))
            .transpose()?;
        if let Some((_, json)) = &headers {
            ensure!(
                json["type"] == "object"
                    && json["properties"]
                        .as_object()
                        .is_none_or(|p| p.values().all(|v| v["type"] == "string")),
                "Kafka headers require string object properties"
            );
        }
        let key = message_bindings
            .get("key")
            .map(|v| schema(root, v, &mut BTreeSet::new()))
            .transpose()?;
        if let Some((value, _)) = &key {
            ensure!(
                matches!(value.kind, SchemaKind::String) && !value.nullable,
                "Kafka key currently supports non-null string schema only"
            );
        }
        operations.push(EventOperation {
            name: name.clone(),
            action,
            channel: channel_ref.into(),
            topic: address.into(),
            message: EventMessage {
                name: message["name"]
                    .as_str()
                    .unwrap_or_else(|| message_location.rsplit('/').next().unwrap_or(name))
                    .into(),
                location: message_location,
                payload,
                payload_schema,
                headers: headers.as_ref().map(|v| v.0.clone()),
                headers_schema: headers.map(|v| v.1),
                key: key.as_ref().map(|v| v.0.clone()),
                key_schema: key.map(|v| v.1),
                bindings: message.get("bindings").cloned().unwrap_or(json!({})),
            },
            group_id,
            client_id: op_client_id,
            bindings: op_bindings,
            channel_bindings,
        });
    }
    ensure!(
        !operations.is_empty(),
        "Kafka generation requires operations"
    );
    Ok(EventOperations {
        source: root.clone(),
        broker: KafkaBroker {
            brokers,
            client_id,
            bindings: server_bindings,
        },
        operations,
    })
}
