use super::*;
pub(crate) fn selected(operation: &Operation) -> bool {
    operation.request_body.as_ref().is_some_and(|body| {
        body.media_types.iter().any(|media| {
            media
                .content_type
                .to_ascii_lowercase()
                .starts_with("multipart/")
        })
    })
}
fn ordered_plan(api: &Api, operation: &Operation) -> Option<serde_json::Value> {
    let content = poolster_core::openapi32::request_content(operation)
        .ok()
        .and_then(|definitions| {
            definitions.into_iter().find(|item| {
                item.content_type
                    .to_ascii_lowercase()
                    .starts_with("multipart/")
            })
        });
    let media = operation
        .request_body
        .as_ref()?
        .media_types
        .iter()
        .find(|media| {
            media
                .content_type
                .to_ascii_lowercase()
                .starts_with("multipart/")
        })?;
    fn advanced(encoding: &poolster_core::openapi32::Encoding) -> bool {
        encoding
            .style
            .as_deref()
            .is_some_and(|style| style != "form")
            || !encoding.headers.is_empty()
            || !encoding.encoding.is_empty()
            || !encoding.prefix_encoding.is_empty()
            || encoding.item_encoding.is_some()
    }
    let needed = fields(api, operation).is_err()
        || !media
            .content_type
            .eq_ignore_ascii_case("multipart/form-data")
        || media.schema.as_ref().is_none_or(|schema| {
            schema.nullable
                || schema.nullish
                || matches!(
                    schema.kind,
                    SchemaKind::Any
                        | SchemaKind::Null
                        | SchemaKind::String
                        | SchemaKind::Number
                        | SchemaKind::Integer
                        | SchemaKind::Boolean
                )
        })
        || media
            .schema
            .as_ref()
            .is_some_and(|schema| matches!(schema.kind, SchemaKind::Array { .. }))
        || content.as_ref().is_some_and(|item| {
            !item.prefix_encoding.is_empty()
                || item.item_encoding.is_some()
                || item.encoding.values().any(advanced)
        });
    if !needed {
        return None;
    }
    Some(
        content
            .and_then(|item| serde_json::to_value(item).ok())
            .unwrap_or_else(|| serde_json::json!({"content_type":media.content_type})),
    )
}
pub(crate) fn body_name(operation: &Operation) -> String {
    let name = format!("{}MultipartBody", type_name(&operation.id));
    let collision=operation.request_body.as_ref().is_some_and(|body|body.media_types.iter().filter_map(|media|media.schema.as_ref()).any(|value|matches!(&value.kind,SchemaKind::Reference{reference} if type_name(reference.rsplit('/').next().unwrap_or(reference))==name)));
    if collision {
        format!("{name}Wire")
    } else {
        name
    }
}
pub(crate) fn mixed(operation: &Operation) -> bool {
    selected(operation)
        && operation
            .request_body
            .as_ref()
            .is_some_and(|body| body.media_types.len() > 1)
}

fn root_fields(api: &Api, value: &SchemaValue, depth: usize) -> Result<Vec<poolster_core::Field>> {
    anyhow::ensure!(
        depth < 12,
        "Multipart root references exceed supported depth"
    );
    let value = resolved(api, value, 0);
    match &value.kind {
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            let _ = additional_properties;
            Ok(fields.clone())
        }
        SchemaKind::OneOf { variants } | SchemaKind::AnyOf { variants } if !variants.is_empty() => {
            let variants = variants
                .iter()
                .map(|value| root_fields(api, value, depth + 1))
                .collect::<Result<Vec<_>>>()?;
            let mut fields = std::collections::BTreeMap::<String, poolster_core::Field>::new();
            for variant in &variants {
                for field in variant {
                    if let Some(existing) = fields.get_mut(&field.name) {
                        if existing.value != field.value {
                            existing.value = SchemaValue::new(SchemaKind::Any);
                        }
                    } else {
                        fields.insert(field.name.clone(), field.clone());
                    }
                }
            }
            for field in fields.values_mut() {
                field.required = variants.iter().all(|variant| {
                    variant
                        .iter()
                        .any(|member| member.name == field.name && member.required)
                });
            }
            Ok(fields.into_values().collect())
        }
        _ => anyhow::bail!("Multipart root must be an object or union of closed objects"),
    }
}
fn open_root(api: &Api, value: &SchemaValue, depth: usize) -> bool {
    if depth > 12 {
        return false;
    }
    match &resolved(api, value, 0).kind {
        SchemaKind::Object {
            additional_properties,
            ..
        } => !matches!(additional_properties, AdditionalProperties::Forbidden),
        SchemaKind::OneOf { variants } | SchemaKind::AnyOf { variants } => variants
            .iter()
            .any(|value| open_root(api, value, depth + 1)),
        _ => false,
    }
}
fn extra_parts(api: &Api, operation: &Operation) -> bool {
    operation
        .request_body
        .as_ref()
        .and_then(|body| {
            body.media_types.iter().find(|media| {
                media
                    .content_type
                    .eq_ignore_ascii_case("multipart/form-data")
            })
        })
        .and_then(|media| media.schema.as_ref())
        .is_some_and(|value| open_root(api, value, 0))
}
fn fields(api: &Api, operation: &Operation) -> Result<Vec<poolster_core::Field>> {
    let body = operation.request_body.as_ref().unwrap();
    anyhow::ensure!(
        body.media_types
            .iter()
            .all(|media| !media.content_type.starts_with("multipart/")
                || media
                    .content_type
                    .eq_ignore_ascii_case("multipart/form-data")),
        "Only multipart/form-data multipart alternatives are supported"
    );
    let media = body
        .media_types
        .iter()
        .find(|media| {
            media
                .content_type
                .eq_ignore_ascii_case("multipart/form-data")
        })
        .ok_or_else(|| anyhow::anyhow!("Missing multipart/form-data media"))?;
    let schema = media
        .schema
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Multipart root schema is required"))?;
    anyhow::ensure!(
        !schema.nullable && !schema.nullish,
        "Multipart root cannot be nullable"
    );
    let fields = root_fields(api, schema, 0)?;
    for field in &fields {
        anyhow::ensure!(
            !field.name.is_empty()
                && field
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-[]".contains(&b)),
            "multipart operation '{}' has an unsupported part name",
            operation.id
        );
        anyhow::ensure!(
            field.value.format.as_deref() != Some("byte"),
            "multipart operation '{}' field '{}' uses ambiguous base64 byte format; use binary for raw file bytes",
            operation.id,
            field.name
        );
    }
    Ok(fields)
}
pub(crate) fn validate(api: &Api) -> Result<()> {
    for op in api.operations.iter().filter(|op| selected(op)) {
        if ordered_plan(api, op).is_none() {
            fields(api, op)?;
        }
    }
    Ok(())
}
fn resolved<'a>(api: &'a Api, value: &'a SchemaValue, depth: usize) -> &'a SchemaValue {
    if depth < 12 {
        if let SchemaKind::Reference { reference } = &value.kind {
            if let Some(schema) = api
                .schemas
                .iter()
                .find(|s| s.name == reference.rsplit('/').next().unwrap_or(reference))
            {
                return resolved(api, &schema.value, depth + 1);
            }
        }
    }
    value
}
fn file_shape(api: &Api, value: &SchemaValue, depth: usize) -> Option<bool> {
    if depth > 12 {
        return None;
    }
    let value = resolved(api, value, 0);
    match &value.kind {
        SchemaKind::String if value.format.as_deref() == Some("binary") => Some(false),
        SchemaKind::Array { items } if file_shape(api, items, depth + 1) == Some(false) => {
            Some(true)
        }
        SchemaKind::OneOf { variants } | SchemaKind::AnyOf { variants } if !variants.is_empty() => {
            let shapes = variants
                .iter()
                .map(|value| file_shape(api, value, depth + 1))
                .collect::<Option<Vec<_>>>()?;
            Some(shapes.into_iter().any(|array| array))
        }
        _ => None,
    }
}
fn encoding<'a>(operation: &'a Operation, name: &str) -> Option<&'a serde_json::Value> {
    operation
        .annotations
        .get("poolster.request_body_encodings")?
        .get("multipart/form-data")?
        .get(name)
}
fn part(
    api: &Api,
    operation: &Operation,
    name: &str,
    value: &SchemaValue,
    property: &str,
) -> Result<String> {
    let value = resolved(api, value, 0);
    let scalar = matches!(
        value.kind,
        SchemaKind::String | SchemaKind::Boolean | SchemaKind::Integer | SchemaKind::Number
    );
    let settings = encoding(operation, name);
    anyhow::ensure!(
        settings
            .and_then(|x| x.get("headers"))
            .and_then(|x| x.as_object())
            .is_none_or(|x| x.is_empty()),
        "Multipart per-part headers require an adapter"
    );
    let content_type = settings
        .and_then(|x| x.get("contentType"))
        .and_then(|x| x.as_str())
        .unwrap_or(if scalar {
            "text/plain"
        } else {
            "application/json"
        });
    anyhow::ensure!(
        content_type
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/!#$&^_.+-".contains(&b))
            && content_type.matches('/').count() == 1,
        "Unsupported multipart content type"
    );
    let content = if scalar {
        format!("MultipartBody.scalar({property})")
    } else {
        format!("MultipartBody.json({property})")
    };
    Ok(format!(
        "new MultipartBody.Part({name:?},{content},null,{content_type:?})"
    ))
}
pub(crate) fn emit(api: &Api, root: &str, package: &str, tree: &mut GeneratedTree) -> Result<()> {
    if !api.operations.iter().any(selected) {
        return Ok(());
    }
    let path = package.replace('.', "/");
    let prefix = if root.is_empty() || root == "." {
        String::new()
    } else {
        format!("{root}/")
    };
    anyhow::ensure!(
        !api.schemas
            .iter()
            .any(|s| type_name(&s.name) == "MultipartBody"),
        "MultipartBody model name is reserved by multipart encoding"
    );
    tree.insert(GeneratedFile::new(
        format!("{prefix}src/main/java/{path}/MultipartBody.java"),
        include_str!("../templates/multipart.java.tmpl").replace("__PACKAGE__", package),
    )?)?;
    tree.insert(GeneratedFile::new(
        format!("{prefix}src/main/java/{path}/OrderedMultipart.java"),
        include_str!("../templates/ordered_multipart.java.tmpl").replace("__PACKAGE__", package),
    )?)?;
    for op in api.operations.iter().filter(|op| selected(op)) {
        let name = body_name(op);
        if let Some(plan) = ordered_plan(api, op) {
            let definition = serde_json::to_string(&plan)?;
            tree.insert(GeneratedFile::new(format!("{prefix}src/main/java/{path}/{name}.java"),format!("package {package};\npublic record {name}(java.util.List<OrderedMultipart.Part> parts) implements MultipartBody {{ public {name} {{ parts=java.util.List.copyOf(parts); }} public MultipartBody.Encoded encode() {{return OrderedMultipart.encode(parts,{definition:?});}} }}\n"))?)?;
            continue;
        }
        anyhow::ensure!(
            !api.schemas.iter().any(|s| type_name(&s.name) == name),
            "multipart body name collides with a schema"
        );
        let fields = fields(api, op)?;
        let mut components = Vec::new();
        let mut encode = String::new();
        let mut checks = String::new();
        for field in &fields {
            let settings = encoding(op, &field.name);
            anyhow::ensure!(
                settings
                    .and_then(|v| v.get("headers"))
                    .and_then(|v| v.as_object())
                    .is_none_or(|v| v.is_empty()),
                "Multipart per-part headers require an adapter"
            );
            anyhow::ensure!(
                settings
                    .and_then(|v| v.get("style"))
                    .and_then(|v| v.as_str())
                    .is_none_or(|style| style == "form"),
                "Multipart encoding style requires an adapter"
            );
            if settings
                .and_then(|v| v.get("explode"))
                .and_then(|v| v.as_bool())
                == Some(false)
            {
                if let SchemaKind::Array { items } = &resolved(api, &field.value, 0).kind {
                    anyhow::ensure!(
                        matches!(
                            resolved(api, items, 0).kind,
                            SchemaKind::String
                                | SchemaKind::Boolean
                                | SchemaKind::Integer
                                | SchemaKind::Number
                        ) && file_shape(api, &field.value, 0).is_none(),
                        "Multipart joined complex or binary arrays require an adapter"
                    );
                }
            }

            let property = field_name(&field.name);
            anyhow::ensure!(
                !matches!(
                    property.as_str(),
                    "encode"
                        | "clone"
                        | "finalize"
                        | "getClass"
                        | "hashCode"
                        | "notify"
                        | "notifyAll"
                        | "toString"
                        | "wait"
                ),
                "multipart field collides with encode method"
            );
            let ty = if file_shape(api, &field.value, 0) == Some(true) {
                "java.util.List<MultipartBody.FilePart>".into()
            } else if file_shape(api, &field.value, 0) == Some(false) {
                "MultipartBody.FilePart".into()
            } else {
                java_type(&field.value)
            };
            components.push(format!("{ty} {property}"));
            if field.required {
                checks.push_str(&format!(
                    "java.util.Objects.requireNonNull({property}, \"required multipart field\");\n"
                ));
            }
            if matches!(field.value.kind, SchemaKind::Number) {
                checks.push_str(&format!("if({property}!=null&&!Double.isFinite({property}))throw new IllegalArgumentException(\"nonfinite multipart number\");\n"));
            }
            if file_shape(api, &field.value, 0) == Some(true) {
                encode.push_str(&format!("if({property}!=null)for(var item:{property})parts.add(new MultipartBody.Part({:?},null,item));\n",field.name));
            } else if let SchemaKind::Array { items } = &resolved(api, &field.value, 0).kind {
                let item_part = part(api, op, &field.name, items, "item")?;
                if encoding(op, &field.name)
                    .and_then(|x| x.get("explode"))
                    .and_then(|x| x.as_bool())
                    == Some(false)
                {
                    let joined = format!(
                        "{property}.stream().map(MultipartBody::scalar).collect(java.util.stream.Collectors.joining(\",\"))"
                    );
                    encode.push_str(&format!("if({property}!=null)parts.add(new MultipartBody.Part({:?},{joined},null));\n",field.name));
                } else {
                    encode.push_str(&format!(
                        "if({property}!=null)for(var item:{property})parts.add({item_part});\n"
                    ));
                }
            } else {
                let part = if file_shape(api, &field.value, 0) == Some(false) {
                    format!("new MultipartBody.Part({:?},null,{property})", field.name)
                } else {
                    part(api, op, &field.name, &field.value, &property)?
                };
                encode.push_str(&format!("if({property}!=null)parts.add({part});\n"));
            }
        }
        if extra_parts(api, op) {
            components.push("java.util.Map<String,Object> poolsterExtraParts".into());
            let names = fields
                .iter()
                .map(|field| format!("{:?}", field.name))
                .collect::<Vec<_>>()
                .join(",");
            encode.push_str(&format!("if(poolsterExtraParts!=null)for(var entry:poolsterExtraParts.entrySet()){{if(java.util.Set.of({names}).contains(entry.getKey()))throw new IllegalArgumentException(\"Extra multipart part collides with declared field\");MultipartBody.addExtra(parts,entry.getKey(),entry.getValue());}}\n"));
        }
        let source = format!(
            "package {package};\nimport {package}.model.*;\nimport java.util.*;\nimport java.time.*;\nimport java.math.*;\nimport com.fasterxml.jackson.databind.JsonNode;\npublic record {name}({}) implements MultipartBody {{\npublic {name} {{ {checks} }}\n@Override public MultipartBody.Encoded encode(){{var parts=new java.util.ArrayList<MultipartBody.Part>();{encode}return MultipartBody.encodeParts(parts);}}\n}}\n",
            components.join(",")
        );
        tree.insert(GeneratedFile::new(
            format!("{prefix}src/main/java/{path}/{name}.java"),
            source,
        )?)?;
    }
    tree.insert(GeneratedFile::new(format!("{prefix}MULTIPART.md"), "Multipart/form-data operations accept a generated <Operation>MultipartBody record through their native request record. File fields use MultipartBody.FilePart(filename, contentType, bytes), or MultipartBody.FilePart.bytes(bytes). File bytes are copied; scalars use UTF-8. Multipart/form-data with explicit raw or JSON alternatives and object roots are supported. Open roots expose optional PoolsterExtraParts/poolsterExtraParts maps, rejecting collisions with declared parts. Root object unions merge fields and retain only shared required members; branch-specific constraints remain server-validated. Mixed-media calls accept native JSON models or explicit MultipartBody.RawBody/PoolsterRawBody buffered media wrappers. Objects/unions/reference values use JSON parts; arrays repeat parts by default and explode=false joins scalar values. Optional parts with null values are omitted. Base64 byte format, streaming and custom per-part headers require adapters. Part names use ASCII letters/digits/dot/underscore/hyphen; filenames use printable ASCII excluding quotes/backslashes. Files are buffered; no streaming/file-system access is implied. Java JSON model APIs remain separate.")?)?;
    Ok(())
}
pub(crate) fn runtime(source: String) -> String {
    source.replace("} else {\n                var requestMedia=", "} else if (body instanceof MultipartBody multipart) {\n                var encoded=multipart.encode(); builder.setHeader(\"Content-Type\",encoded.contentType()); builder.method(method,HttpRequest.BodyPublishers.ofByteArray(encoded.bytes()));\n            } else {\n                var requestMedia=").replace("} else {\n                builder.header(\"Content-Type\", \"application/json\");", "} else if (body instanceof MultipartBody multipart) {\n                var encoded=multipart.encode(); builder.setHeader(\"Content-Type\",encoded.contentType()); builder.method(method,HttpRequest.BodyPublishers.ofByteArray(encoded.bytes()));\n            } else {\n                builder.header(\"Content-Type\", \"application/json\");")
 .replace("else if (body instanceof byte[] bytes)", "else if (body instanceof MultipartBody multipart) { var encoded=multipart.encode();builder.setHeader(\"Content-Type\",encoded.contentType());builder.method(method,HttpRequest.BodyPublishers.ofByteArray(encoded.bytes())); }\n                else if (body instanceof byte[] bytes)")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn api() -> Api {
        let mut file = SchemaValue::new(SchemaKind::String);
        file.format = Some("binary".into());
        let field = |name: &str, value, required| poolster_core::Field {
            name: name.into(),
            value,
            required,
            annotations: Default::default(),
        };
        Api {
            name: "Multipart".into(),
            schemas: vec![Schema::new(
                "Upload",
                SchemaValue::new(SchemaKind::Object {
                    fields: vec![
                        field("title", SchemaValue::new(SchemaKind::String), true),
                        field("flag", SchemaValue::new(SchemaKind::Boolean), true),
                        field("count", SchemaValue::new(SchemaKind::Integer), true),
                        field("file", file, true),
                        field("missing", SchemaValue::new(SchemaKind::String), false),
                    ],
                    additional_properties: AdditionalProperties::Forbidden,
                }),
            )],
            operations: vec![Operation {
                id: "uploadThing".into(),
                method: poolster_core::HttpMethod::Post,
                path: "/upload".into(),
                request_body: Some(poolster_core::OperationRequestBody {
                    required: true,
                    description: None,
                    media_types: vec![poolster_core::OperationMediaType {
                        content_type: "multipart/form-data".into(),
                        schema: Some(SchemaValue::reference("#/components/schemas/Upload")),
                    }],
                }),
                responses: vec![poolster_core::OperationResponse {
                    status: "204".into(),
                    description: None,
                    media_types: vec![],
                }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }
    #[test]
    fn rejects_unsupported_shapes_before_emission() {
        let mut source = api();
        let SchemaKind::Object { fields, .. } = &mut source.schemas[0].value.kind else {
            unreachable!()
        };
        fields[0].value = SchemaValue::new(SchemaKind::Array {
            items: Box::new(SchemaValue::new(SchemaKind::String)),
        });
        validate(&source).unwrap();
        let mut source = api();
        source.operations[0]
            .request_body
            .as_mut()
            .unwrap()
            .media_types
            .push(poolster_core::OperationMediaType {
                content_type: "application/json".into(),
                schema: Some(SchemaValue::new(SchemaKind::String)),
            });
        validate(&source).unwrap();
    }
    #[test]
    fn emits_native_typed_multipart_in_all_response_drivers() {
        let tree = poolster_core::engine::Packages::new()
            .package(
                crate::package("sdk")
                    .name("io.poolster.multipart")
                    .with(crate::sdk()),
            )
            .generate(&api(), None)
            .unwrap();
        let client = tree
            .get("sdk/src/main/java/io/poolster/multipart/ClientBase.java")
            .unwrap();
        assert_eq!(
            client
                .matches("body instanceof MultipartBody multipart")
                .count(),
            4
        );
        let dto = tree
            .get("sdk/src/main/java/io/poolster/multipart/UploadThingMultipartBody.java")
            .unwrap();
        assert!(dto.contains("MultipartBody.FilePart file"));
        assert!(dto.contains("MultipartBody.scalar(flag)"));
        let operation = tree
            .iter()
            .find(|(_, source)| source.contains("public record UploadThingRequest"))
            .unwrap()
            .1;
        assert!(operation.contains("UploadThingMultipartBody body"));
    }
    #[test]
    #[ignore = "requires JDK17+Maven; parses native HTTP multipart request bytes without network"]
    fn native_multipart_http_bytes_preserve_unicode_falsy_and_binary() {
        let tree = poolster_core::engine::Packages::new()
            .package(
                crate::package("sdk")
                    .name("io.poolster.multipart")
                    .with(crate::sdk())
                    .with(crate::operation_tests()),
            )
            .generate(&api(), None)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        let mut source = include_str!("../tests/fixtures/operation_driver.java")
            .replace("__PACKAGE__", "io.poolster.multipart")
            .replace(
                "__CASES__",
                include_str!("../tests/fixtures/multipart_probe_main.java"),
            );
        let start = source.find("        void assertRequest(").unwrap();
        let end = source[start..]
            .find("        public <T> HttpResponse<T> send(")
            .unwrap()
            + start;
        source.replace_range(
            start..end,
            include_str!("../tests/fixtures/multipart_probe_assert.java"),
        );
        std::fs::write(
            dir.path()
                .join("sdk/src/test/java/io/poolster/multipart/PoolsterOperationTests.java"),
            source,
        )
        .unwrap();
        let output = std::process::Command::new("mvn")
            .args([
                "-q",
                "test-compile",
                "org.codehaus.mojo:exec-maven-plugin:3.5.0:java",
                "-Dexec.mainClass=io.poolster.multipart.PoolsterOperationTests",
                "-Dexec.classpathScope=test",
            ])
            .current_dir(dir.path().join("sdk"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fn complex_api() -> Api {
        let mut source = api();
        let SchemaKind::Object {
            fields,
            additional_properties,
        } = &mut source.schemas[0].value.kind
        else {
            unreachable!()
        };
        *additional_properties = AdditionalProperties::Any;
        fields.push(poolster_core::Field {
            name: "chunking_strategy".into(),
            value: SchemaValue::new(SchemaKind::OneOf {
                variants: vec![
                    SchemaValue::new(SchemaKind::String),
                    SchemaValue::new(SchemaKind::Object {
                        fields: vec![],
                        additional_properties: AdditionalProperties::Any,
                    }),
                ],
            }),
            required: false,
            annotations: Default::default(),
        });
        fields.push(poolster_core::Field {
            name: "timestamp_granularities[]".into(),
            value: SchemaValue::new(SchemaKind::Array {
                items: Box::new(SchemaValue::new(SchemaKind::String)),
            }),
            required: false,
            annotations: Default::default(),
        });
        let mut binary = SchemaValue::new(SchemaKind::String);
        binary.format = Some("binary".into());
        fields.push(poolster_core::Field {
            name: "files".into(),
            value: SchemaValue::new(SchemaKind::Array {
                items: Box::new(binary),
            }),
            required: false,
            annotations: Default::default(),
        });
        source.operations[0].annotations.insert("poolster.request_body_encodings".into(),serde_json::json!({"multipart/form-data":{"chunking_strategy":{"contentType":"application/json"}}}));
        source
    }

    #[test]
    #[ignore = "requires JDK17+Maven; native complex multipart wire probe"]
    fn native_multipart_complex_json_and_repeated_arrays() {
        let tree = poolster_core::engine::Packages::new()
            .package(
                crate::package("sdk")
                    .name("io.poolster.multipart")
                    .with(crate::sdk())
                    .with(crate::operation_tests()),
            )
            .generate(&complex_api(), None)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        let main=include_str!("../tests/fixtures/multipart_probe_main.java").replace("file,null);","file,null,MAPPER.valueToTree(Map.of(\"type\",\"server_vad\",\"label\",\"café雪\",\"threshold\",0.5)),List.of(\"word\",\"segment\"),List.of(file,file),Map.of(\"extra\",Map.of(\"snow\",\"雪\")));");
        let mut source = include_str!("../tests/fixtures/operation_driver.java")
            .replace("__PACKAGE__", "io.poolster.multipart")
            .replace("__CASES__", &main);
        let start = source.find("        void assertRequest(").unwrap();
        let end = start
            + source[start..]
                .find("        public <T> HttpResponse<T> send(")
                .unwrap();
        source.replace_range(
            start..end,
            include_str!("../tests/fixtures/multipart_complex_assert.java"),
        );
        std::fs::write(
            dir.path()
                .join("sdk/src/test/java/io/poolster/multipart/PoolsterOperationTests.java"),
            source,
        )
        .unwrap();
        let output = std::process::Command::new("mvn")
            .args([
                "-q",
                "test-compile",
                "org.codehaus.mojo:exec-maven-plugin:3.5.0:java",
                "-Dexec.mainClass=io.poolster.multipart.PoolsterOperationTests",
                "-Dexec.classpathScope=test",
            ])
            .current_dir(dir.path().join("sdk"))
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
