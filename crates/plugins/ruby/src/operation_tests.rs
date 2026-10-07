use serde_json::Value;

use super::*;
use kaji_core::engine::{Meta, Plugin, PluginContext};
pub struct OperationTests {
    provider: Option<kaji_core::engine::Handle<crate::package::RubyModels>>,
    meta: Meta,
    options: kaji_core::samples::SampleOptions,
    limit: usize,
}
pub fn operation_tests() -> OperationTests {
    OperationTests {
        meta: Meta::new(),
        provider: None,
        options: Default::default(),
        limit: 128,
    }
}
impl OperationTests {
    pub fn using_models(
        mut self,
        models: kaji_core::engine::Handle<crate::package::RubyModels>,
    ) -> Self {
        self.provider = Some(models);
        self
    }
    pub fn models_from(self, sdk: &crate::package::Sdk) -> Self {
        self.using_models(sdk.models())
    }
    pub fn sample_options(mut self, options: kaji_core::samples::SampleOptions) -> Self {
        self.options = options;
        self
    }
    pub fn max_operations(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}
fn sample(
    api: &Api,
    value: &SchemaValue,
    options: kaji_core::samples::SampleOptions,
) -> std::result::Result<Value, String> {
    let report = kaji_core::samples::schema_samples(api, value, options);
    report
        .samples
        .into_iter()
        .next()
        .map(|x| x.value)
        .ok_or_else(|| report.diagnostics.join("; "))
}
fn fixture(
    api: &Api,
    op: &Operation,
    options: kaji_core::samples::SampleOptions,
) -> std::result::Result<Value, String> {
    if !op.security.is_empty() {
        return Err("authentication requires a fixture adapter".into());
    }
    let mut args = serde_json::Map::new();
    let mut path = op.path.clone();
    let mut query = serde_json::Map::new();
    let mut headers = serde_json::Map::new();
    for p in &op.parameters {
        let schema = p.schema.as_ref().ok_or("parameter schema unavailable")?;
        if schema.nullable
            || !matches!(
                schema.kind,
                SchemaKind::String | SchemaKind::Boolean | SchemaKind::Integer | SchemaKind::Number
            )
        {
            return Err("non-scalar parameters require an adapter".into());
        }
        let value = sample(api, schema, options)?;
        let text = if let Some(s) = value.as_str() {
            s.to_owned()
        } else {
            value.to_string()
        };
        match p.location.as_str() {
            "path" => {
                if !text
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-._~".contains(c))
                {
                    return Err("reserved path fixture needs an encoding adapter".into());
                }
                path = path.replace(&format!("{{{}}}", p.name), &text);
            }
            "query" => {
                query.insert(p.name.clone(), Value::String(text.clone()));
            }
            "header" => {
                headers.insert(p.name.clone(), Value::String(text));
            }
            _ => return Err("parameter location requires an adapter".into()),
        }
        args.insert(p.name.clone(), value);
    }
    let body = if let Some(body) = &op.request_body {
        let media = body
            .media_types
            .iter()
            .find(|m| m.content_type == "application/json" || m.content_type.ends_with("+json"))
            .ok_or("non-JSON request body requires an adapter")?;
        Some(sample(
            api,
            media.schema.as_ref().ok_or("request schema unavailable")?,
            options,
        )?)
    } else {
        None
    };
    let response = op
        .responses
        .iter()
        .find(|r| {
            r.status
                .parse::<u16>()
                .is_ok_and(|n| (200..300).contains(&n))
        })
        .ok_or("explicit successful response required")?;
    let payload = if response.media_types.is_empty() {
        None
    } else {
        let media = response
            .media_types
            .iter()
            .find(|m| m.content_type == "application/json" || m.content_type.ends_with("+json"))
            .ok_or("non-JSON response requires an adapter")?;
        Some(sample(
            api,
            media.schema.as_ref().ok_or("response schema unavailable")?,
            options,
        )?)
    };
    if body.as_ref().is_some_and(Value::is_null) || payload.as_ref().is_some_and(Value::is_null) {
        return Err("nullable root sample requires an adapter".into());
    }
    Ok(
        serde_json::json!({"args":args,"method":op.method.as_str(),"path":path,"query":query,"headers":headers,"body":body,"response":payload,"status":response.status}),
    )
}
impl Plugin<crate::Ruby> for OperationTests {
    fn kind(&self) -> &'static str {
        "ruby-operation-tests"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<kaji_core::engine::Requirement> {
        vec![kaji_core::engine::Requirement::on(self.provider)]
    }
    fn generate(&self, cx: &mut PluginContext<'_, crate::Ruby>) -> Result<()> {
        let models = cx.inputs.get::<crate::package::RubyModels>()?;
        let mut cases = Vec::new();
        let mut skipped = BTreeMap::new();
        for (index, op) in cx.api.operations.iter().enumerate() {
            let result = if index >= self.limit {
                Err("operation bound exhausted".into())
            } else {
                fixture(cx.api, op, self.options)
            };
            match result {
                Ok(mut value) => {
                    value["operation"] = Value::String(ruby_identifier(&op.id));
                    let args = value["args"]
                        .as_object()
                        .unwrap()
                        .iter()
                        .map(|(k, v)| (ruby_identifier(k), v.clone()))
                        .collect::<serde_json::Map<_, _>>();
                    value["args"] = Value::Object(args);
                    if op.request_body.is_some() {
                        value["args"]["body"] = value["body"].clone();
                    }
                    cases.push(value)
                }
                Err(reason) => {
                    skipped.insert(op.id.clone(), reason);
                }
            }
        }
        cx.files.emit(GeneratedFile::new(
            "test/operation-fixtures.json",
            serde_json::to_string_pretty(&serde_json::json!({"cases":cases,"skipped":skipped}))?,
        )?)?;
        let script = include_str!("operation_tests.rb.txt")
            .replace("__IMPORT__", &models.import)
            .replace("__MODULE__", &models.module);
        cx.files
            .emit(GeneratedFile::new("test/operation_tests.rb", script)?)?;
        cx.files.emit(GeneratedFile::new("OPERATION_TESTS.md","Run `ruby -Ilib test/operation_tests.rb`. Bounded structural fixtures use native public calls and a fake transport; no network. Method/path, scalar query/headers, JSON body and decoded response are checked. Unsupported operations are listed in test/operation-fixtures.json. This smoke suite does not prove full schema or service behavior.")?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_operation_tests_execute_native() {
        let mut api = Api {
            name: "Probe".into(),
            operations: vec![
                Operation {
                    id: "readThing".into(),
                    method: kaji_core::HttpMethod::Get,
                    path: "/thing".into(),
                    responses: vec![kaji_core::OperationResponse {
                        status: "200".into(),
                        description: None,
                        media_types: vec![kaji_core::OperationMediaType {
                            content_type: "application/json".into(),
                            schema: Some(SchemaValue::new(SchemaKind::String)),
                        }],
                    }],
                    ..Default::default()
                },
                Operation {
                    id: "deleteThing".into(),
                    method: kaji_core::HttpMethod::Delete,
                    path: "/thing".into(),
                    responses: vec![kaji_core::OperationResponse {
                        status: "204".into(),
                        description: None,
                        media_types: vec![],
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        api.operations.push(Operation {
            id: "createThing".into(),
            method: kaji_core::HttpMethod::Post,
            path: "/thing".into(),
            request_body: Some(kaji_core::OperationRequestBody::json(
                SchemaValue::new(SchemaKind::Boolean),
                true,
            )),
            parameters: vec![
                kaji_core::OperationParameter {
                    name: "flag".into(),
                    location: "query".into(),
                    required: true,
                    schema: Some(SchemaValue::new(SchemaKind::Boolean)),
                    description: None,
                    annotations: Default::default(),
                },
                kaji_core::OperationParameter {
                    name: "count".into(),
                    location: "header".into(),
                    required: true,
                    schema: Some(SchemaValue::new(SchemaKind::Integer)),
                    description: None,
                    annotations: Default::default(),
                },
            ],
            responses: vec![kaji_core::OperationResponse {
                status: "200".into(),
                description: None,
                media_types: vec![kaji_core::OperationMediaType {
                    content_type: "application/json".into(),
                    schema: Some(SchemaValue::new(SchemaKind::Boolean)),
                }],
            }],
            ..Default::default()
        });
        api.operations.push(Operation {
            id: "retryThing".into(),
            method: kaji_core::HttpMethod::Post,
            path: "/thing".into(),
            parameters: vec![kaji_core::OperationParameter {
                name: "X-Once".into(),
                location: "header".into(),
                required: false,
                schema: Some(SchemaValue::new(SchemaKind::String)),
                description: None,
                annotations: Default::default(),
            }],
            responses: vec![kaji_core::OperationResponse {
                status: "204".into(),
                description: None,
                media_types: vec![],
            }],
            annotations: std::collections::BTreeMap::from([(
                "x-kaji-idempotency".into(),
                serde_json::json!({"header":"X-Once","auto_generate":true}),
            )]),
            ..Default::default()
        });
        let tree = kaji_core::engine::Packages::new()
            .package(
                crate::package("sdk")
                    .with(crate::sdk())
                    .with(operation_tests()),
            )
            .generate(&api, None)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        std::fs::write(
            dir.path().join("sdk/test/retry_probe.rb"),
            include_str!("retry_probe.rb.txt"),
        )
        .unwrap();
        let output = std::process::Command::new("ruby")
            .args(["-Ilib", "test/operation_tests.rb"])
            .current_dir(dir.path().join("sdk"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let output = std::process::Command::new("ruby")
            .args(["-Ilib", "test/retry_probe.rb"])
            .current_dir(dir.path().join("sdk"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
