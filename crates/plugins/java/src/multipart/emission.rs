use super::*;
pub(super) fn part(
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
        include_str!("../../templates/multipart.java.tmpl").replace("__PACKAGE__", package),
    )?)?;
    tree.insert(GeneratedFile::new(
        format!("{prefix}src/main/java/{path}/OrderedMultipart.java"),
        include_str!("../../templates/ordered_multipart.java.tmpl").replace("__PACKAGE__", package),
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
