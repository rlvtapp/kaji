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
fn ordered_plan(operation: &Operation) -> Option<serde_json::Value> {
    let content = kaji_core::openapi32::request_content(operation)
        .ok()
        .and_then(|definitions| {
            definitions
                .into_iter()
                .find(|item| item.content_type.starts_with("multipart/"))
        });
    let media = operation
        .request_body
        .as_ref()?
        .media_types
        .iter()
        .find(|media| media.content_type.starts_with("multipart/"))?;
    fn advanced(encoding: &kaji_core::openapi32::Encoding) -> bool {
        !encoding.headers.is_empty()
            || !encoding.encoding.is_empty()
            || !encoding.prefix_encoding.is_empty()
            || encoding.item_encoding.is_some()
    }
    let needed = media.content_type != "multipart/form-data"
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
    let name = format!("{}MultipartBody", pascal_case(&operation.id));
    let collision=operation.request_body.as_ref().is_some_and(|body|body.media_types.iter().filter_map(|media|media.schema.as_ref()).any(|value|matches!(&value.kind,SchemaKind::Reference{reference} if pascal_case(reference.rsplit('/').next().unwrap_or(reference))==name)));
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

fn root_fields(api: &Api, value: &SchemaValue, depth: usize) -> Result<Vec<kaji_core::Field>> {
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
            let mut fields = std::collections::BTreeMap::<String, kaji_core::Field>::new();
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
fn fields(api: &Api, operation: &Operation) -> Result<Vec<kaji_core::Field>> {
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
        if ordered_plan(op).is_none() {
            fields(api, op)?;
        }
    }
    Ok(())
}
fn binary(field: &kaji_core::Field) -> bool {
    field.value.format.as_deref() == Some("binary")
        && matches!(field.value.kind, SchemaKind::String)
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
        .get("kaji.request_body_encodings")?
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
        format!("Scalar({property})")
    } else {
        format!(
            "System.Text.Json.JsonSerializer.Serialize({property},new System.Text.Json.JsonSerializerOptions(System.Text.Json.JsonSerializerDefaults.Web))"
        )
    };
    Ok(format!(
        "content.Add(new StringContent({content},System.Text.Encoding.UTF8,{content_type:?}),{name:?});"
    ))
}
pub(crate) fn emit(api: &Api, root: &str, namespace: &str, tree: &mut GeneratedTree) -> Result<()> {
    if !api.operations.iter().any(selected) {
        return Ok(());
    }
    anyhow::ensure!(
        !api.schemas
            .iter()
            .any(|s| pascal_case(&s.name) == "MultipartFile"),
        "MultipartFile model name reserved by multipart encoding"
    );
    tree.insert(GeneratedFile::new(
        output_path(root, "MultipartFile.cs"),
        include_str!("multipart.cs.txt").replace("__NAMESPACE__", namespace),
    )?)?;
    tree.insert(GeneratedFile::new(
        output_path(root, "OrderedMultipart.cs"),
        include_str!("ordered_multipart.cs.txt").replace("__NAMESPACE__", namespace),
    )?)?;
    for op in api.operations.iter().filter(|op| selected(op)) {
        let name = body_name(op);
        if let Some(plan) = ordered_plan(op) {
            let definition = serde_json::to_string(&plan)?;
            tree.insert(GeneratedFile::new(output_path(root,&format!("{name}.cs")),format!("namespace {namespace};\npublic sealed record {name} {{public required System.Collections.Generic.IReadOnlyList<OrderedMultipartPart> Parts {{get;init;}} public System.Net.Http.HttpContent ToContent()=>OrderedMultipart.Encode(Parts,{definition:?});}}\n"))?)?;
            continue;
        }
        anyhow::ensure!(
            !api.schemas.iter().any(|s| pascal_case(&s.name) == name),
            "multipart body name collides with a schema"
        );
        let fields = fields(api, op)?;
        let mut properties = String::new();
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

            let property = pascal_case(&field.name);
            anyhow::ensure!(
                !matches!(
                    property.as_str(),
                    "ToContent" | "Equals" | "GetHashCode" | "ToString" | "EqualityContract"
                ),
                "multipart field collides with ToContent method"
            );
            let ty = if file_shape(api, &field.value, 0) == Some(true) {
                if field.required {
                    "System.Collections.Generic.List<MultipartFile>"
                } else {
                    "System.Collections.Generic.List<MultipartFile>?"
                }
                .into()
            } else if file_shape(api, &field.value, 0) == Some(false) {
                if field.required {
                    "MultipartFile"
                } else {
                    "MultipartFile?"
                }
                .into()
            } else {
                csharp_type(&field.value, !field.required)
            };
            let required = if field.required { "required " } else { "" };
            properties.push_str(&format!("public {required}{ty} {property} {{get;init;}}\n"));
            if field.required && (binary(field) || matches!(field.value.kind, SchemaKind::String)) {
                checks.push_str(&format!("ArgumentNullException.ThrowIfNull({property});\n"));
            }
            if matches!(field.value.kind, SchemaKind::Number) {
                let value = if field.required {
                    property.clone()
                } else {
                    format!("{property}.Value")
                };
                let guard = if field.required {
                    String::new()
                } else {
                    format!("{property} is not null && ")
                };
                checks.push_str(&format!("if({guard}!double.IsFinite({value}))throw new ArgumentException(\"Nonfinite multipart number\");\n"));
            }
            let encoded = if file_shape(api, &field.value, 0) == Some(true) {
                format!(
                    "foreach(var item in {property}){{content.Add(item.ToContent(),{:?},item.FileName);}}",
                    field.name
                )
            } else if let SchemaKind::Array { items } = &resolved(api, &field.value, 0).kind {
                if encoding(op, &field.name)
                    .and_then(|x| x.get("explode"))
                    .and_then(|x| x.as_bool())
                    == Some(false)
                {
                    format!(
                        "content.Add(new StringContent(string.Join(\",\",{property}.Select(item=>Scalar(item))),System.Text.Encoding.UTF8),{:?});",
                        field.name
                    )
                } else {
                    let item_part = part(api, op, &field.name, items, "item")?;
                    format!("foreach(var item in {property}){{{item_part}}}")
                }
            } else if file_shape(api, &field.value, 0) == Some(false) {
                format!(
                    "var part={property}.ToContent();content.Add(part,{:?},{property}.FileName);",
                    field.name
                )
            } else {
                part(api, op, &field.name, &field.value, &property)?
            };
            if field.required && !field.value.nullable && !field.value.nullish {
                encode.push_str(&format!("{{{encoded}}}\n"));
            } else {
                encode.push_str(&format!("if({property} is not null){{{encoded}}}\n"));
            }
        }
        if extra_parts(api, op) {
            properties.push_str("public System.Collections.Generic.Dictionary<string,object?>? KajiExtraParts {get;init;}\n");
            let names = fields
                .iter()
                .map(|field| format!("{:?}", field.name))
                .collect::<Vec<_>>()
                .join(",");
            encode.push_str(&format!("if(KajiExtraParts is not null)foreach(var entry in KajiExtraParts){{if(new string[]{{{names}}}.Contains(entry.Key))throw new ArgumentException(\"Extra multipart part collides with declared field\");AddExtra(content,entry.Key,entry.Value);}}\n"));
        }
        let source = format!(
            "using System;\nusing System.Net.Http;\nusing System.Linq;\nusing System.Text.Json;\nnamespace {namespace};\npublic sealed record {name} {{\n{properties}\nprivate static void AddExtra(MultipartFormDataContent content,string name,object? value){{if(value is null)return;if(value is MultipartFile file){{content.Add(file.ToContent(),name,file.FileName);return;}}if(value is System.Collections.Generic.IEnumerable<MultipartFile> files){{foreach(var item in files)content.Add(item.ToContent(),name,item.FileName);return;}}var element=System.Text.Json.JsonSerializer.SerializeToElement(value,new System.Text.Json.JsonSerializerOptions(System.Text.Json.JsonSerializerDefaults.Web));if(element.ValueKind==System.Text.Json.JsonValueKind.Array){{foreach(var item in element.EnumerateArray())AddExtra(content,name,item);return;}}bool scalar=element.ValueKind is System.Text.Json.JsonValueKind.String or System.Text.Json.JsonValueKind.Number or System.Text.Json.JsonValueKind.True or System.Text.Json.JsonValueKind.False;content.Add(new StringContent(scalar?Scalar(value):element.GetRawText(),System.Text.Encoding.UTF8,scalar?\"text/plain\":\"application/json\"),name);}}\nprivate static string Scalar(object? value){{var element=System.Text.Json.JsonSerializer.SerializeToElement(value,new System.Text.Json.JsonSerializerOptions(System.Text.Json.JsonSerializerDefaults.Web));return element.ValueKind==System.Text.Json.JsonValueKind.String?element.GetString()!:element.GetRawText();}}\npublic MultipartFormDataContent ToContent(){{{checks}\nvar content=new MultipartFormDataContent();try{{{encode}return content;}}catch{{content.Dispose();throw;}}}}\n}}\n"
        );
        tree.insert(GeneratedFile::new(
            output_path(root, &format!("{name}.cs")),
            source,
        )?)?;
    }
    tree.insert(GeneratedFile::new(output_path(root,"MULTIPART.md"), "Multipart/form-data operations accept a generated <Operation>MultipartBody record. File fields use new MultipartFile(bytes, fileName, contentType); bytes are copied. Scalars are encoded with UTF-8/invariant culture and native MultipartFormDataContent. Multipart/form-data with explicit raw or JSON alternatives and object roots are supported. Open roots expose optional KajiExtraParts/kajiExtraParts maps, rejecting collisions with declared parts. Root object unions merge fields and retain only shared required members; branch-specific constraints remain server-validated. Mixed-media calls accept native JSON models or explicit MultipartBody.RawBody/KajiRawBody buffered media wrappers. Objects/unions/reference values use JSON parts; arrays repeat parts by default and explode=false joins scalar values. Optional parts with null values are omitted. Base64 byte format, streaming and custom per-part headers require adapters. Part names use ASCII letters/digits/dot/underscore/hyphen; filenames use printable ASCII excluding quotes/backslashes. Files are buffered; no streaming/file-system access is implied. JSON model APIs remain separate.")?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn api() -> Api {
        let mut file = SchemaValue::new(SchemaKind::String);
        file.format = Some("binary".into());
        let field = |name: &str, value, required| kaji_core::Field {
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
                method: kaji_core::HttpMethod::Post,
                path: "/upload".into(),
                request_body: Some(kaji_core::OperationRequestBody {
                    required: true,
                    description: None,
                    media_types: vec![kaji_core::OperationMediaType {
                        content_type: "multipart/form-data".into(),
                        schema: Some(SchemaValue::reference("#/components/schemas/Upload")),
                    }],
                }),
                responses: vec![kaji_core::OperationResponse {
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
            .push(kaji_core::OperationMediaType {
                content_type: "application/json".into(),
                schema: Some(SchemaValue::new(SchemaKind::String)),
            });
        validate(&source).unwrap();
    }
    #[test]
    fn emits_native_typed_multipart_in_public_and_resource_calls() {
        let tree = kaji_core::engine::Packages::new()
            .package(
                crate::package("sdk")
                    .name("Kaji.Multipart")
                    .with(crate::sdk()),
            )
            .generate(&api(), None)
            .unwrap();
        let dto = tree.get("sdk/UploadThingMultipartBody.cs").unwrap();
        assert!(dto.contains("required MultipartFile File"));
        assert!(dto.contains("new MultipartFormDataContent()"));
        let client = tree
            .get("sdk/Operations/KajiClientOperations000.cs")
            .unwrap();
        assert!(client.contains("UploadThingMultipartBody body"));
        assert!(client.contains("request.Content = body.ToContent()"));
        assert!(!client.contains("UploadThingAsync(byte[]"));
    }
    #[test]
    #[ignore = "requires .NET8; parses native HTTP multipart request bytes without network"]
    fn native_multipart_http_bytes_preserve_unicode_falsy_and_binary() {
        let tree = kaji_core::engine::Packages::new()
            .package(
                crate::package("sdk")
                    .name("Kaji.Multipart")
                    .with(crate::sdk())
                    .with(crate::operation_tests()),
            )
            .generate(&api(), None)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        std::fs::write(
            dir.path().join("sdk/tests/OperationTests/Program.cs"),
            include_str!("multipart_probe.cs.txt"),
        )
        .unwrap();
        let output = std::process::Command::new("dotnet")
            .args([
                "run",
                "--project",
                "tests/OperationTests/OperationTests.csproj",
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
        fields.push(kaji_core::Field {
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
        fields.push(kaji_core::Field {
            name: "timestamp_granularities[]".into(),
            value: SchemaValue::new(SchemaKind::Array {
                items: Box::new(SchemaValue::new(SchemaKind::String)),
            }),
            required: false,
            annotations: Default::default(),
        });
        let mut binary = SchemaValue::new(SchemaKind::String);
        binary.format = Some("binary".into());
        fields.push(kaji_core::Field {
            name: "files".into(),
            value: SchemaValue::new(SchemaKind::Array {
                items: Box::new(binary),
            }),
            required: false,
            annotations: Default::default(),
        });
        source.operations[0].annotations.insert("kaji.request_body_encodings".into(),serde_json::json!({"multipart/form-data":{"chunking_strategy":{"contentType":"application/json"}}}));
        source
    }

    #[test]
    #[ignore = "requires .NET8; native complex multipart wire probe"]
    fn native_multipart_complex_json_and_repeated_arrays() {
        let tree = kaji_core::engine::Packages::new()
            .package(
                crate::package("sdk")
                    .name("Kaji.Multipart")
                    .with(crate::sdk())
                    .with(crate::operation_tests()),
            )
            .generate(&complex_api(), None)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        std::fs::write(
            dir.path().join("sdk/tests/OperationTests/Program.cs"),
            include_str!("multipart_complex_probe.cs.txt"),
        )
        .unwrap();
        let output = std::process::Command::new("dotnet")
            .args([
                "run",
                "--project",
                "tests/OperationTests/OperationTests.csproj",
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
