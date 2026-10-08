//! Allocate package symbols before rendering, keeping wire names and plugin keys intact.
use super::*;
use std::borrow::Cow;

fn hash(value: &str) -> u64 {
    value.bytes().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}
fn allocate(
    names: impl Iterator<Item = String>,
    reserved: BTreeSet<String>,
    suffix: &str,
) -> BTreeMap<String, String> {
    let names: BTreeSet<_> = names.collect();
    let mut counts = BTreeMap::new();
    for name in &names {
        *counts.entry(go_type_name(name)).or_insert(0) += 1;
    }
    // Preserve every unambiguous original symbol before allocating renamed ones.
    let mut used = reserved.clone();
    used.extend(names.iter().map(|name| go_type_name(name)));
    let mut result = BTreeMap::new();
    for name in names {
        let symbol = go_type_name(&name);
        if counts[&symbol] == 1 && !reserved.contains(&symbol) {
            continue;
        }
        let base = format!("{symbol}{suffix}{:016x}", hash(&name));
        let mut candidate = base.clone();
        let mut index = 2;
        while !used.insert(go_type_name(&candidate)) {
            candidate = format!("{base}{index}");
            index += 1;
        }
        result.insert(name, candidate);
    }
    result
}
fn model_names(api: &Api) -> BTreeMap<String, String> {
    let runtime = render_runtime(api, "sdk", SdkClientStyle::Namespaced);
    let mut reserved: BTreeSet<String> = runtime
        .lines()
        .filter_map(|line| {
            let declaration = line
                .strip_prefix("type ")
                .or_else(|| line.strip_prefix("func "))?;
            let name = declaration
                .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                .next()?;
            name.chars()
                .next()
                .filter(|ch| ch.is_ascii_uppercase())
                .map(|_| name.to_owned())
        })
        .collect();
    let resources = resource_operations(api);
    reserved.extend(
        resource_facade_names(&resources)
            .values()
            .map(|name| format!("{name}Service")),
    );
    for schema in &api.schemas {
        if matches!(schema.value.kind, SchemaKind::String) {
            let name = go_type_name(&schema.name);
            reserved.extend(schema.value.enum_values.iter().map(|value| {
                format!("{name}{}", go_type_name(value.as_str().unwrap_or_default()))
            }));
        }
    }
    allocate(
        api.schemas.iter().map(|schema| schema.name.clone()),
        reserved,
        "Model",
    )
}
fn operation_names(api: &Api) -> BTreeMap<String, String> {
    allocate(
        api.operations.iter().map(|operation| operation.id.clone()),
        BTreeSet::new(),
        "Operation",
    )
}
fn value(value: &mut SchemaValue, names: &BTreeMap<String, String>) {
    match &mut value.kind {
        SchemaKind::Reference { reference } => {
            if let Some((prefix, name)) = reference.rsplit_once('/') {
                let decoded = name.replace("~1", "/").replace("~0", "~");
                if let Some(new) = names.get(&decoded) {
                    *reference = format!("{prefix}/{}", new.replace('~', "~0").replace('/', "~1"));
                }
            } else if let Some(new) = names.get(reference) {
                *reference = new.clone();
            }
        }
        SchemaKind::Array { items } => self::value(items, names),
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            for field in fields {
                self::value(&mut field.value, names);
            }
            if let AdditionalProperties::Schema { value } = additional_properties {
                self::value(value, names);
            }
        }
        SchemaKind::OneOf { variants }
        | SchemaKind::AnyOf { variants }
        | SchemaKind::AllOf { variants } => {
            for variant in variants {
                self::value(variant, names);
            }
        }
        SchemaKind::Not { schema } => self::value(schema, names),
        _ => {}
    }
    if let Some(discriminator) = &mut value.discriminator {
        for reference in discriminator.mapping.values_mut() {
            if let Some((prefix, name)) = reference.rsplit_once('/') {
                if let Some(new) = names.get(&name.replace("~1", "/").replace("~0", "~")) {
                    *reference = format!("{prefix}/{}", new.replace('~', "~0").replace('/', "~1"));
                }
            }
        }
    }
}
pub(super) fn prepare(api: &Api) -> Cow<'_, Api> {
    let models = model_names(api);
    let operations = operation_names(api);
    if models.is_empty() && operations.is_empty() {
        return Cow::Borrowed(api);
    }
    let mut prepared = api.clone();
    for schema in &mut prepared.schemas {
        if let Some(name) = models.get(&schema.name) {
            schema.name = name.clone();
        }
        value(&mut schema.value, &models);
    }
    for operation in &mut prepared.operations {
        if let Some(name) = operations.get(&operation.id) {
            operation.id = name.clone();
        }
        for parameter in &mut operation.parameters {
            if let Some(schema) = &mut parameter.schema {
                value(schema, &models);
            }
        }
        if let Some(body) = &mut operation.request_body {
            for media in &mut body.media_types {
                if let Some(schema) = &mut media.schema {
                    value(schema, &models);
                }
            }
        }
        for response in &mut operation.responses {
            for media in &mut response.media_types {
                if let Some(schema) = &mut media.schema {
                    value(schema, &models);
                }
            }
        }
    }
    Cow::Owned(prepared)
}
pub(super) fn model_symbols(api: &Api) -> BTreeMap<String, String> {
    let names = model_names(api);
    api.schemas
        .iter()
        .map(|schema| {
            (
                schema.name.clone(),
                go_type_name(names.get(&schema.name).unwrap_or(&schema.name)),
            )
        })
        .collect()
}
pub(super) fn operation_symbols(api: &Api) -> BTreeMap<String, String> {
    let names = operation_names(api);
    api.operations
        .iter()
        .map(|operation| {
            (
                operation.id.clone(),
                go_type_name(names.get(&operation.id).unwrap_or(&operation.id)),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaji_core::{Field, HttpMethod, OperationMediaType, OperationResponse};
    #[test]
    fn allocated_models_and_operations_compile_through_typed_providers() {
        let object = |field: &str| {
            SchemaValue::new(SchemaKind::Object {
                fields: vec![Field {
                    name: field.into(),
                    value: SchemaValue::new(SchemaKind::String),
                    required: true,
                    annotations: Default::default(),
                }],
                additional_properties: AdditionalProperties::Forbidden,
            })
        };
        let mut api = Api {
            name: "Symbols".into(),
            version: "1".into(),
            ..Default::default()
        };
        api.schemas = vec![
            Schema::new("Client", object("wire_name")),
            Schema::new("HTTPHealthCheck", object("first")),
            Schema::new("HttpHealthCheck", object("second")),
        ];
        for (id, path, model) in [
            ("get_http", "/one", "HTTPHealthCheck"),
            ("getHTTP", "/two", "HttpHealthCheck"),
        ] {
            api.operations.push(Operation {
                id: id.into(),
                method: HttpMethod::Get,
                path: path.into(),
                responses: vec![OperationResponse {
                    status: "200".into(),
                    description: None,
                    media_types: vec![OperationMediaType {
                        content_type: "application/json".into(),
                        schema: Some(SchemaValue::reference(format!(
                            "#/components/schemas/{model}"
                        ))),
                    }],
                }],
                ..Operation::default()
            });
        }
        let mut opponent_type = SchemaValue::new(SchemaKind::String);
        opponent_type.enum_values = vec![serde_json::json!("player"), serde_json::json!("team")];
        api.schemas.extend([
            Schema::new("OpponentType", opponent_type),
            Schema::new("OpponentTypePlayer", SchemaValue::new(SchemaKind::String)),
            Schema::new("OneService", object("service_value")),
        ]);
        api.schemas.push(Schema::new(
            "OddWireNames",
            SchemaValue::new(SchemaKind::Object {
                fields: ["packageManager `npm`", "quote\"key", "a,b", "-", ""]
                    .into_iter()
                    .map(|name| Field {
                        name: name.into(),
                        value: SchemaValue::new(SchemaKind::String),
                        required: true,
                        annotations: Default::default(),
                    })
                    .collect(),
                additional_properties: AdditionalProperties::Forbidden,
            }),
        ));
        let symbols = model_symbols(&api);
        assert_ne!(symbols["HTTPHealthCheck"], symbols["HttpHealthCheck"]);
        assert_ne!(symbols["Client"], "Client");
        assert_ne!(symbols["OpponentTypePlayer"], "OpponentTypePlayer");
        assert_ne!(symbols["OneService"], "OneService");
        let methods = operation_symbols(&api);
        assert_ne!(methods["get_http"], methods["getHTTP"]);
        let mut reordered = api.clone();
        reordered.schemas.reverse();
        reordered.operations.reverse();
        assert_eq!(model_symbols(&reordered), symbols);
        assert_eq!(operation_symbols(&reordered), methods);
        let models = crate::models();
        let model_handle = models.models_handle();
        let transport = crate::transport();
        let transport_handle = transport.transport_handle();
        let operations = crate::operations()
            .using_models(model_handle)
            .using_transport(transport_handle);
        let operation_handle = operations.operations_handle();
        let client = crate::client().using_operations(operation_handle);
        let client_handle = client.client_handle();
        let tree = kaji_core::engine::Packages::new()
            .package(
                crate::package("sdk")
                    .with(models)
                    .with(transport)
                    .with(operations)
                    .with(client)
                    .with(crate::roundtrip_tests().using_models(model_handle))
                    .with(
                        crate::operation_tests()
                            .using_client(client_handle)
                            .using_operations(operation_handle),
                    ),
            )
            .generate(&api, None)
            .unwrap();
        let root = tempfile::tempdir().unwrap();
        tree.write_to(root.path()).unwrap();
        let output = std::process::Command::new("go")
            .args(["test", "./..."])
            .current_dir(root.path().join("sdk"))
            .env(
                "GOCACHE",
                std::env::var_os("GOCACHE")
                    .unwrap_or_else(|| root.path().join("cache").into_os_string()),
            )
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
