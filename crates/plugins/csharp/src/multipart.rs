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
fn fields<'a>(api: &'a Api, operation: &Operation) -> Result<&'a [kaji_core::Field]> {
    let body = operation.request_body.as_ref().unwrap();
    anyhow::ensure!(
        body.media_types.len() == 1
            && body.media_types[0]
                .content_type
                .eq_ignore_ascii_case("multipart/form-data"),
        "multipart operation '{}' requires exactly one multipart/form-data media type",
        operation.id
    );
    let schema = body.media_types[0].schema.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "multipart operation '{}' requires a named object schema",
            operation.id
        )
    })?;
    anyhow::ensure!(
        !schema.nullable && !schema.nullish,
        "multipart operation '{}' cannot use a nullable root",
        operation.id
    );
    let SchemaKind::Reference { reference } = &schema.kind else {
        anyhow::bail!(
            "multipart operation '{}' requires a named object reference",
            operation.id
        )
    };
    let name = reference.rsplit('/').next().unwrap_or(reference);
    let schema = api.schemas.iter().find(|s| s.name == name).ok_or_else(|| {
        anyhow::anyhow!(
            "multipart operation '{}' references an unknown schema",
            operation.id
        )
    })?;
    anyhow::ensure!(
        !schema.value.nullable && !schema.value.nullish,
        "multipart operation '{}' resolves to a nullable object",
        operation.id
    );
    let SchemaKind::Object {
        fields,
        additional_properties,
    } = &schema.value.kind
    else {
        anyhow::bail!(
            "multipart operation '{}' requires a direct object schema",
            operation.id
        )
    };
    anyhow::ensure!(
        matches!(additional_properties, AdditionalProperties::Forbidden),
        "multipart operation '{}' requires a closed object; extra parts need an adapter",
        operation.id
    );
    for field in fields {
        anyhow::ensure!(
            !field.value.nullable
                && !field.value.nullish
                && matches!(
                    field.value.kind,
                    SchemaKind::String
                        | SchemaKind::Boolean
                        | SchemaKind::Integer
                        | SchemaKind::Number
                ),
            "multipart operation '{}' field '{}' requires a direct nonnullable scalar or binary string; arrays, references and unions need an adapter",
            operation.id,
            field.name
        );
        anyhow::ensure!(
            !field.name.is_empty()
                && field
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)),
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
        fields(api, op)?;
    }
    Ok(())
}
fn binary(field: &kaji_core::Field) -> bool {
    field.value.format.as_deref() == Some("binary")
        && matches!(field.value.kind, SchemaKind::String)
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
    for op in api.operations.iter().filter(|op| selected(op)) {
        let name = format!("{}MultipartBody", pascal_case(&op.id));
        anyhow::ensure!(
            !api.schemas.iter().any(|s| pascal_case(&s.name) == name),
            "multipart body name collides with a schema"
        );
        let fields = fields(api, op)?;
        let mut properties = String::new();
        let mut encode = String::new();
        let mut checks = String::new();
        for field in fields {
            let property = pascal_case(&field.name);
            anyhow::ensure!(
                !matches!(
                    property.as_str(),
                    "ToContent" | "Equals" | "GetHashCode" | "ToString" | "EqualityContract"
                ),
                "multipart field collides with ToContent method"
            );
            let ty = if binary(field) {
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
            let part = if binary(field) {
                format!(
                    "var part={property}.ToContent();content.Add(part,{:?},{property}.FileName);",
                    field.name
                )
            } else {
                let value = if matches!(field.value.kind, SchemaKind::Boolean) {
                    format!("{property}.ToString().ToLowerInvariant()")
                } else {
                    format!(
                        "Convert.ToString({property},System.Globalization.CultureInfo.InvariantCulture)!"
                    )
                };
                format!(
                    "content.Add(new StringContent({value},System.Text.Encoding.UTF8),{:?});",
                    field.name
                )
            };
            if field.required {
                encode.push_str(&format!("{{{part}}}\n"));
            } else {
                encode.push_str(&format!("if({property} is not null){{{part}}}\n"));
            }
        }
        let source = format!(
            "using System;\nusing System.Net.Http;\nnamespace {namespace};\npublic sealed record {name} {{\n{properties}\npublic MultipartFormDataContent ToContent(){{{checks}\nvar content=new MultipartFormDataContent();try{{{encode}return content;}}catch{{content.Dispose();throw;}}}}\n}}\n"
        );
        tree.insert(GeneratedFile::new(
            output_path(root, &format!("{name}.cs")),
            source,
        )?)?;
    }
    tree.insert(GeneratedFile::new(output_path(root,"MULTIPART.md"), "Multipart/form-data operations accept a generated <Operation>MultipartBody record. File fields use new MultipartFile(bytes, fileName, contentType); bytes are copied. Scalars are encoded with UTF-8/invariant culture and native MultipartFormDataContent. Only one multipart/form-data media type and a named closed object with direct nonnullable scalar/binary fields are supported. Optional parts with null values are omitted. Arrays, nested/reference fields, unions, extra parts, base64 byte format and custom per-part encoding require adapters. Part names use ASCII letters/digits/dot/underscore/hyphen; filenames use printable ASCII excluding quotes/backslashes. Files are buffered; no streaming/file-system access is implied. JSON model APIs remain separate.")?)?;
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
        assert!(
            validate(&source)
                .unwrap_err()
                .to_string()
                .contains("arrays")
        );
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
        assert!(
            validate(&source)
                .unwrap_err()
                .to_string()
                .contains("exactly one")
        );
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
}
