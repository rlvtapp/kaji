// Internal bounded fixture policy: examples/defaults/source extension values
// never enter emitted fixtures. Unsupported schemas receive explicit reasons.
use anyhow::{Result, bail, ensure};
use poolster_core::{AdditionalProperties, Api, Operation, SchemaKind, SchemaValue};
use serde_json::{Value, json};
pub(super) fn clean_api(api: &Api) -> Api {
    fn clean(value: &mut SchemaValue) {
        value.description = None;
        value.title = None;
        value.extensions.clear();
        value.default = None;
        value.constraints.remove("example");
        value.constraints.remove("examples");
        match &mut value.kind {
            SchemaKind::Object {
                fields,
                additional_properties,
            } => {
                for field in fields {
                    field.annotations.clear();
                    clean(&mut field.value);
                }
                if let AdditionalProperties::Schema { value } = additional_properties {
                    clean(value);
                }
            }
            SchemaKind::Array { items } => clean(items),
            SchemaKind::OneOf { variants }
            | SchemaKind::AnyOf { variants }
            | SchemaKind::AllOf { variants } => {
                for variant in variants {
                    clean(variant)
                }
            }
            _ => {}
        }
    }
    let mut api = api.clone();
    for schema in &mut api.schemas {
        clean(&mut schema.value);
    }
    for operation in &mut api.operations {
        for parameter in &mut operation.parameters {
            if let Some(schema) = &mut parameter.schema {
                clean(schema);
            }
        }
        if let Some(body) = &mut operation.request_body {
            for media in &mut body.media_types {
                if let Some(schema) = &mut media.schema {
                    clean(schema);
                }
            }
        }
        for response in &mut operation.responses {
            for media in &mut response.media_types {
                if let Some(schema) = &mut media.schema {
                    clean(schema);
                }
            }
        }
    }
    api
}
fn supported(api: &Api, value: &SchemaValue, seen: &mut Vec<String>) -> Result<()> {
    ensure!(
        value.constraints.is_empty()
            && value.enum_values.is_empty()
            && value.const_value.is_none()
            && value.format.is_none(),
        "constraints/enums/formats require explicit fixtures"
    );
    ensure!(
        !value.write_only,
        "readOnly/writeOnly projection requires explicit fixtures"
    );
    match &value.kind {
        SchemaKind::Reference { reference } => {
            ensure!(
                reference.starts_with("#/components/schemas/"),
                "only local component references supported"
            );
            let name = reference.rsplit('/').next().unwrap();
            ensure!(
                seen.len() < 12 && !seen.iter().any(|item| item == name),
                "recursive/deep references require explicit fixtures"
            );
            seen.push(name.into());
            supported(
                api,
                &api.schemas
                    .iter()
                    .find(|schema| schema.name == name)
                    .ok_or_else(|| anyhow::anyhow!("missing schema"))?
                    .value,
                seen,
            )?;
            seen.pop();
        }
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            ensure!(
                fields.len() <= 128,
                "object property count exceeds sample bound"
            );
            for field in fields {
                supported(api, &field.value, seen)?;
            }
            if let AdditionalProperties::Schema { value } = additional_properties {
                supported(api, value, seen)?;
            }
        }
        SchemaKind::Array { items } => supported(api, items, seen)?,
        SchemaKind::String
        | SchemaKind::Boolean
        | SchemaKind::Integer
        | SchemaKind::Number
        | SchemaKind::Null => {}
        _ => bail!("compositional/unknown schemas require explicit fixtures"),
    }
    Ok(())
}
pub(super) fn sample(api: &Api, schema: &SchemaValue) -> Result<Value> {
    supported(api, schema, &mut vec![])?;
    let report = poolster_core::samples::schema_samples(
        api,
        schema,
        poolster_core::samples::SampleOptions {
            max_depth: 12,
            max_samples: 16,
            max_array_items: 2,
        },
    );
    ensure!(
        report.diagnostics.is_empty(),
        "bounded schema sampling incomplete"
    );
    report
        .samples
        .iter()
        .find(|sample| sample.name == "full")
        .or(report.samples.first())
        .map(|sample| sample.value.clone())
        .ok_or_else(|| anyhow::anyhow!("bounded schema sample unavailable"))
}
pub(super) fn case(api: &Api, operation: &Operation) -> Result<Value> {
    ensure!(
        operation.parameters.len() <= 32,
        "parameter count exceeds sample bound"
    );
    ensure!(
        operation.method.as_str() != "HEAD",
        "HEAD requires separate non-body assertions"
    );
    let responses = operation
        .responses
        .iter()
        .filter(|response| response.status.starts_with('2'))
        .collect::<Vec<_>>();
    ensure!(
        responses.len() == 1,
        "exactly one declared success response required"
    );
    let response = responses[0];
    let status = response.status.parse::<u16>()?;
    ensure!(
        status != 204 && response.media_types.len() == 1,
        "one buffered JSON success required"
    );
    let media = &response.media_types[0];
    ensure!(
        media.content_type == "application/json" || media.content_type.ends_with("+json"),
        "streaming/binary representations require explicit fixtures"
    );
    let response = sample(
        api,
        media
            .schema
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("missing response schema"))?,
    )?;
    let mut parameters = vec![];
    for parameter in &operation.parameters {
        ensure!(
            matches!(parameter.location.as_str(), "path" | "query" | "header"),
            "cookie/unknown parameter location requires explicit fixtures"
        );
        let schema = parameter
            .schema
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("missing parameter schema"))?;
        ensure!(
            matches!(
                schema.kind,
                SchemaKind::String | SchemaKind::Integer | SchemaKind::Number | SchemaKind::Boolean
            ),
            "only direct scalar parameters supported"
        );
        let value = sample(api, schema)?;
        ensure!(
            !value.is_null(),
            "null parameter controls require explicit fixtures"
        );
        parameters.push(json!({"name":parameter.name,"location":parameter.location,"value":value}));
    }
    let body = if let Some(body) = &operation.request_body {
        ensure!(
            body.media_types.len() == 1,
            "one request representation required"
        );
        let media = &body.media_types[0];
        ensure!(
            media.content_type == "application/json" || media.content_type.ends_with("+json"),
            "nonJSON request requires explicit fixtures"
        );
        Some(sample(
            api,
            media
                .schema
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("missing request schema"))?,
        )?)
    } else {
        None
    };
    let case = json!({"operation":operation.id,"method":operation.method.as_str(),"path":operation.path,"parameters":parameters,"body":body,"has_body":operation.request_body.is_some(),"response":response,"status":status});
    ensure!(
        case.to_string().len() <= 65536,
        "wire fixture exceeds byte bound"
    );
    Ok(case)
}
pub(super) fn encoded_path(case: &Value) -> String {
    let mut path = case["path"].as_str().unwrap().to_owned();
    for parameter in case["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|parameter| parameter["location"] == "path")
    {
        let value = parameter["value"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| parameter["value"].to_string());
        let encoded = value
            .bytes()
            .map(|byte| {
                if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                    (byte as char).to_string()
                } else {
                    format!("%{byte:02X}")
                }
            })
            .collect::<String>();
        path = path.replace(
            &format!("{{{}}}", parameter["name"].as_str().unwrap()),
            &encoded,
        );
    }
    path
}
