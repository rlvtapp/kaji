//! Allocate public names before rendering, including case-insensitive paths.
use poolster_core::{Api, SchemaKind};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn identifier(value: &str) -> String {
    let mut out = String::new();
    let mut upper = true;
    for c in value.chars() {
        if c.is_ascii_alphanumeric() {
            if upper {
                out.extend(c.to_uppercase());
            } else {
                out.push(c);
            }
            upper = false;
        } else {
            upper = true;
        }
    }
    if out.is_empty() {
        out.push_str("Operation");
    }
    if out.as_bytes()[0].is_ascii_digit() {
        out.insert_str(0, "Value");
    }
    out
}

pub(crate) fn camel(value: &str) -> String {
    let name = identifier(value);
    let mut chars = name.chars();
    let result = chars.next().unwrap().to_lowercase().collect::<String>() + chars.as_str();
    if matches!(
        result.as_str(),
        "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "new"
            | "null"
            | "return"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
            | "let"
            | "static"
            | "implements"
            | "interface"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "await"
    ) {
        format!("{result}Operation")
    } else {
        result
    }
}

fn allocate(values: impl Iterator<Item = String>) -> BTreeMap<String, String> {
    allocate_by(values, str::to_owned)
}

fn allocate_by(
    values: impl Iterator<Item = String>,
    key: fn(&str) -> String,
) -> BTreeMap<String, String> {
    let values = values.collect::<BTreeSet<_>>();
    let mut used = BTreeSet::new();
    let mut result = BTreeMap::new();
    for value in values {
        let base = identifier(&value);
        let mut name = base.clone();
        let mut suffix = 2;
        while !used.insert(key(&name).to_ascii_lowercase()) {
            name = format!("{base}{suffix}");
            suffix += 1;
        }
        result.insert(value, name);
    }
    result
}

pub(crate) fn prepare(api: &Api) -> Api {
    let schemas = allocate(api.schemas.iter().map(|s| s.name.clone()));
    let mut operations = allocate(api.operations.iter().map(|o| o.id.clone()));
    let mut used = schemas
        .values()
        .map(|v| v.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut functions = BTreeSet::new();
    for name in operations.values_mut() {
        let base = name.clone();
        let mut suffix = 2;
        while used.contains(&name.to_ascii_lowercase())
            || functions.contains(&camel(name).to_ascii_lowercase())
        {
            *name = format!("{base}Operation{suffix}");
            suffix += 1;
        }
        used.insert(name.to_ascii_lowercase());
        functions.insert(camel(name).to_ascii_lowercase());
    }
    let mut result = api.clone();
    for schema in &mut result.schemas {
        schema.name = schemas[&schema.name].clone();
    }
    for operation in &mut result.operations {
        operation.id = operations[&operation.id].clone();
    }
    crate::json::visit_api(&mut result, &mut |value| {
        if let SchemaKind::Reference { reference } = &mut value.kind {
            if let Some(name) = schemas.get(reference.rsplit('/').next().unwrap_or(reference)) {
                *reference = format!("#/components/schemas/{name}");
            }
        }
    });
    let tags = allocate_by(
        result
            .operations
            .iter()
            .flat_map(|o| {
                o.annotations
                    .get("tags")
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
            })
            .filter_map(serde_json::Value::as_str)
            .map(str::to_owned),
        camel,
    );
    for operation in &mut result.operations {
        if let Some(serde_json::Value::Array(values)) = operation.annotations.get_mut("tags") {
            for value in values {
                if let Some(name) = value.as_str().and_then(|n| tags.get(n)) {
                    *value = serde_json::Value::String(name.clone());
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use poolster_core::{Operation, OperationResponse, Schema, SchemaValue};

    #[test]
    fn allocations_preserve_distinct_wire_contracts_and_resolve_references() {
        let api = Api {
            schemas: vec![
                Schema::new("HTTPHealthCheck", SchemaValue::new(SchemaKind::String)),
                Schema::new("HttpHealthCheck", SchemaValue::new(SchemaKind::Integer)),
                Schema::new(
                    "401 error",
                    SchemaValue::reference("#/components/schemas/HttpHealthCheck"),
                ),
            ],
            operations: vec![
                Operation {
                    id: "export".into(),
                    path: "/Export".into(),
                    responses: vec![OperationResponse::json(
                        "200",
                        SchemaValue::reference("#/components/schemas/HTTPHealthCheck"),
                    )],
                    ..Default::default()
                },
                Operation {
                    id: "Export".into(),
                    path: "/export".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let result = prepare(&api);
        assert_eq!(result.schemas[0].name, "HTTPHealthCheck");
        assert_eq!(result.schemas[1].name, "HttpHealthCheck2");
        assert_eq!(result.schemas[2].name, "Value401Error");
        assert_eq!(
            result.schemas[2].value.kind.reference_name(),
            Some("HttpHealthCheck2")
        );
        assert_eq!(camel(&result.operations[0].id), "export2");
        assert_eq!(camel(&result.operations[1].id), "exportOperation");
        assert_eq!(result.operations[0].path, "/Export");
        assert_eq!(prepare(&result).schemas[1].name, result.schemas[1].name);
    }

    #[test]
    #[ignore = "requires Node and KAJI_TSC_JS"]
    fn collision_heavy_sdk_compiles_with_original_provider_contract_keys() {
        use poolster_core::engine::Packages;
        use poolster_core::{HttpMethod, OperationMediaType, OperationRequestBody};
        let long = "LongComponent".repeat(15);
        let api = Api {
            name: "Collision API".into(),
            schemas: vec![
                Schema::new("UserName", SchemaValue::new(SchemaKind::String)),
                Schema::new("Username", SchemaValue::new(SchemaKind::Integer)),
                Schema::new(
                    &long,
                    SchemaValue::reference("#/components/schemas/Username"),
                ),
                Schema::new("401 error", SchemaValue::new(SchemaKind::Boolean)),
                Schema::new(
                    "UnionArray",
                    SchemaValue::new(SchemaKind::Array {
                        items: Box::new(SchemaValue {
                            enum_values: vec![serde_json::json!("a"), serde_json::json!("b")],
                            ..SchemaValue::new(SchemaKind::String)
                        }),
                    }),
                ),
            ],
            operations: ["export", "Export", "3d model", "exportOperation"]
                .into_iter()
                .enumerate()
                .map(|(index, id)| Operation {
                    id: id.into(),
                    method: HttpMethod::Post,
                    path: format!("/wire-{index}"),
                    annotations: [(
                        "tags".into(),
                        serde_json::json!([if index == 0 {
                            "default"
                        } else if index == 3 {
                            "DefaultOperation"
                        } else {
                            "Default"
                        }]),
                    )]
                    .into_iter()
                    .collect(),
                    request_body: Some(OperationRequestBody {
                        required: true,
                        description: None,
                        media_types: vec![
                            OperationMediaType {
                                content_type: "".into(),
                                schema: Some(SchemaValue::new(SchemaKind::String)),
                            },
                            OperationMediaType {
                                content_type: "application/json".into(),
                                schema: Some(SchemaValue::reference(format!(
                                    "#/components/schemas/{long}"
                                ))),
                            },
                        ],
                    }),
                    responses: vec![OperationResponse {
                        status: "200".into(),
                        description: None,
                        media_types: vec![
                            OperationMediaType {
                                content_type: "application/xml".into(),
                                schema: Some(SchemaValue::new(SchemaKind::String)),
                            },
                            OperationMediaType {
                                content_type: "text/xml".into(),
                                schema: Some(SchemaValue::new(SchemaKind::Integer)),
                            },
                            OperationMediaType {
                                content_type: "*/*".into(),
                                schema: Some(SchemaValue::new(SchemaKind::Boolean)),
                            },
                            OperationMediaType {
                                content_type: "application/json; charset=utf-8".into(),
                                schema: Some(SchemaValue::new(SchemaKind::Integer)),
                            },
                            OperationMediaType {
                                content_type: "application/json;charset=utf-8".into(),
                                schema: Some(SchemaValue::new(SchemaKind::Boolean)),
                            },
                        ],
                    }],
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        let tree = Packages::new()
            .package(crate::package("sdk").with(crate::sdk()))
            .generate(&api, None)
            .unwrap();
        let temporary =
            std::env::temp_dir().join(format!("kaji-ts-symbols-{}", std::process::id()));
        tree.write_to(&temporary).unwrap();
        std::fs::write(temporary.join("sdk/consumer.ts"), "import type { UnionArray } from './models/UnionArray';\nconst valid: UnionArray = ['a', 'b'];\n// @ts-expect-error: array item unions must not allow a bare scalar.\nconst invalid: UnionArray = 'a';\nconsole.log(valid, invalid);\n").unwrap();
        let output = std::process::Command::new("node")
            .arg(std::env::var("KAJI_TSC_JS").unwrap())
            .arg("-p")
            .arg(temporary.join("sdk/tsconfig.json"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::remove_dir_all(temporary).unwrap();
    }
}
