//! Opt-in bounded operation tests. Run with the SDK's normal native dependencies installed.
use super::*;
use anyhow::{bail, ensure};
use poolster_core::engine::{Meta, Plugin, PluginContext};
use serde_json::{Value, json};
pub struct OperationTests {
    meta: Meta,
}
pub fn operation_tests() -> OperationTests {
    OperationTests { meta: Meta::new() }
}
impl Plugin<crate::Php> for OperationTests {
    fn kind(&self) -> &'static str {
        "php-operation-tests"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, crate::Php>) -> Result<()> {
        let package = cx
            .settings
            .package_name
            .clone()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| format!("poolster/{}-sdk", package_slug(&cx.api.name)));
        let module = namespace_for_package(&package);
        let mut api = crate::symbols::prepare(cx.api).into_owned();
        for schema in &mut api.schemas {
            sanitize(&mut schema.value)
        }
        let mut cases = vec![];
        let mut diagnostics = vec![];
        for operation in &api.operations {
            if cases.len() >= 256 {
                diagnostics.push(json!({"operation":operation.id,"status":"skipped","reason":"bounded operation limit (256)"}));
                continue;
            }
            match case(&api, operation) {
                Ok(value) => cases.push(value),
                Err(error) => diagnostics.push(
                    json!({"operation":operation.id,"status":"skipped","reason":error.to_string()}),
                ),
            }
        }
        cx.files.emit(GeneratedFile::new(
            "tests/operation-fixtures.json",
            serde_json::to_string_pretty(&json!({"cases":cases}))?,
        )?)?;
        cx.files.emit(GeneratedFile::new(".poolster/operation-test-diagnostics.json",serde_json::to_string_pretty(&json!({"scope":"bounded buffered JSON success and malformed JSON; native fake transport; not live acceptance", "covered_operations":cases.len(),"diagnostics":diagnostics}))?)?)?;
        cx.files.emit(GeneratedFile::new(
            "tests/operations.php",
            include_str!("../templates/operation_tests.php.tmpl").replace("__MODULE__", &module),
        )?)
    }
}
fn sanitize(schema: &mut SchemaValue) {
    schema.default = None;
    schema.extensions.clear();
    schema.description = None;
    schema.title = None;
    match &mut schema.kind {
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            for field in fields {
                sanitize(&mut field.value)
            }
            if let poolster_core::AdditionalProperties::Schema { value } = additional_properties {
                sanitize(value)
            }
        }
        SchemaKind::Array { items } => sanitize(items),
        SchemaKind::OneOf { variants }
        | SchemaKind::AnyOf { variants }
        | SchemaKind::AllOf { variants } => {
            for value in variants {
                sanitize(value)
            }
        }
        _ => {}
    }
}
fn supported(api: &Api, schema: &SchemaValue, seen: &mut Vec<String>) -> Result<()> {
    ensure!(
        schema.constraints.is_empty()
            && schema.enum_values.is_empty()
            && schema.const_value.is_none()
            && schema.format.is_none(),
        "schema constraints/enums/formats need dedicated fixtures"
    );
    ensure!(
        !schema.write_only,
        "writeOnly schema needs explicit response fixture projection"
    );
    match &schema.kind {
        SchemaKind::Reference { reference } => {
            ensure!(
                reference.starts_with("#/components/schemas/"),
                "external/noncomponent reference unsupported"
            );
            let name = reference.rsplit('/').next().unwrap();
            ensure!(
                !seen.iter().any(|n| n == name) && seen.len() < 12,
                "recursive/deep reference unsupported"
            );
            seen.push(name.into());
            supported(
                api,
                &api.schemas
                    .iter()
                    .find(|s| s.name == name)
                    .ok_or_else(|| anyhow::anyhow!("missing referenced schema"))?
                    .value,
                seen,
            )?;
            seen.pop();
        }
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            for field in fields {
                supported(api, &field.value, seen)?
            }
            if let poolster_core::AdditionalProperties::Schema { value } = additional_properties {
                supported(api, value, seen)?
            }
        }
        SchemaKind::Array { items } => supported(api, items, seen)?,
        SchemaKind::Boolean
        | SchemaKind::String
        | SchemaKind::Integer
        | SchemaKind::Number
        | SchemaKind::Null => {}
        _ => bail!("unknown/compositional schemas need dedicated fixtures"),
    }
    Ok(())
}
fn sample(api: &Api, schema: &SchemaValue) -> Result<Value> {
    supported(api, schema, &mut vec![])?;
    let mut clean = schema.clone();
    sanitize(&mut clean);
    let report = poolster_core::samples::schema_samples(
        api,
        &clean,
        poolster_core::samples::SampleOptions {
            max_samples: 16,
            max_depth: 12,
            max_array_items: 2,
        },
    );
    ensure!(
        report.diagnostics.is_empty(),
        "bounded fixture could not completely represent schema: {}",
        report.diagnostics.join("; ")
    );
    report
        .samples
        .into_iter()
        .find(|s| s.name == "full")
        .or_else(|| {
            poolster_core::samples::schema_samples(api, &clean, Default::default())
                .samples
                .into_iter()
                .next()
        })
        .map(|s| s.value)
        .ok_or_else(|| anyhow::anyhow!("no bounded sample"))
}

fn case(api: &Api, operation: &Operation) -> Result<Value> {
    ensure!(
        operation.method.as_str() != "HEAD",
        "HEAD has no buffered response"
    );
    ensure!(
        idempotency_annotation(operation).is_none(),
        "auto idempotency requires replay-specific fixtures"
    );
    let responses = operation
        .responses
        .iter()
        .filter(|r| r.status.starts_with('2'))
        .collect::<Vec<_>>();
    ensure!(
        responses.len() == 1,
        "exactly one declared success response required"
    );
    let response = responses[0];
    let status = response.status.parse::<u16>()?;
    ensure!(
        status != 204 && response.media_types.len() == 1,
        "single buffered JSON representation required"
    );
    let media = &response.media_types[0];
    ensure!(
        media.content_type == "application/json" || media.content_type.ends_with("+json"),
        "non-JSON/stream response unsupported"
    );
    let schema = media
        .schema
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("response schema missing"))?;
    ensure!(
        !matches!(schema.kind, SchemaKind::Null),
        "null-only response has no decoding assertion"
    );
    let result = sample(api, schema)?;
    let mut parameters = vec![];
    let mut names = std::collections::BTreeSet::new();
    let mut ordered = operation.parameters.clone();
    ordered.sort_by_key(|p| !p.required);
    for parameter in &ordered {
        ensure!(
            matches!(parameter.location.as_str(), "path" | "query" | "header"),
            "cookie/unknown parameter unsupported"
        );
        ensure!(
            !parameter.annotations.contains_key("style")
                && !parameter.annotations.contains_key("explode")
                && !parameter.annotations.contains_key("allowReserved"),
            "serialization overrides need explicit fixtures"
        );
        let schema = parameter
            .schema
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("parameter schema missing"))?;
        ensure!(
            !schema.nullable
                && !schema.nullish
                && matches!(
                    schema.kind,
                    SchemaKind::String
                        | SchemaKind::Boolean
                        | SchemaKind::Integer
                        | SchemaKind::Number
                ),
            "only nonnullable scalar parameters supported"
        );
        ensure!(
            !matches!(
                parameter.name.to_ascii_lowercase().as_str(),
                "authorization" | "cookie" | "set-cookie" | "x-api-key"
            ) && !schema.write_only,
            "sensitive parameter requires explicit fixture"
        );
        let argument = property_name(&parameter.name);
        ensure!(
            !matches!(
                argument.as_str(),
                "path" | "query" | "headers" | "contents" | "data"
            ),
            "parameter collides with generated operation locals; requires explicit renderer fixture"
        );
        ensure!(
            names.insert(argument.clone()),
            "normalized parameter collision"
        );
        parameters.push(json!({"name":parameter.name,"argument":argument,"location":parameter.location,"value":sample(api,schema)?}));
    }
    let mut body = None;
    let mut body_model: Option<String> = None;
    if let Some(request) = &operation.request_body {
        ensure!(
            request.media_types.len() == 1
                && request.media_types[0].content_type == "application/json",
            "request needs single JSON representation"
        );
        let schema = request.media_types[0]
            .schema
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("request schema missing"))?;
        body = Some(sample(api, schema)?);
        ensure!(
            body.as_ref().is_some_and(|value| !value.is_null()),
            "null request body needs explicit native representation fixture"
        );
        if let SchemaKind::Reference { reference } = &schema.kind {
            let name = reference.rsplit('/').next().unwrap();
            let target = api
                .schemas
                .iter()
                .find(|item| item.name == name)
                .ok_or_else(|| anyhow::anyhow!("request model missing"))?;
            ensure!(
                matches!(target.value.kind, SchemaKind::Object { .. }),
                "request reference must be object model"
            );
            body_model = Some(type_name(name));
        }
    }
    let body_required = operation
        .request_body
        .as_ref()
        .is_some_and(|request| request.required);
    let first_optional = ordered
        .iter()
        .position(|parameter| !parameter.required)
        .unwrap_or(ordered.len());
    Ok(
        json!({"operation":operation.id,"method_name":method_name(&operation.id),"method":operation.method.as_str(),"path":operation.path,"status":status,"content_type":media.content_type,"parameters":parameters,"result":result,"result_json":serde_json::to_string(&result)?,"body":body,"body_model":body_model,"body_required":body_required,"first_optional":first_optional}),
    )
}

#[cfg(test)]
mod tests;
