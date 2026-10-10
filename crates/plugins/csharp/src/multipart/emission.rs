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
        include_str!("../../templates/multipart.cs.tmpl").replace("__NAMESPACE__", namespace),
    )?)?;
    tree.insert(GeneratedFile::new(
        output_path(root, "OrderedMultipart.cs"),
        include_str!("../../templates/ordered_multipart.cs.tmpl")
            .replace("__NAMESPACE__", namespace),
    )?)?;
    for op in api.operations.iter().filter(|op| selected(op)) {
        let name = body_name(op);
        if let Some(plan) = ordered_plan(api, op) {
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
            properties.push_str("public System.Collections.Generic.Dictionary<string,object?>? PoolsterExtraParts {get;init;}\n");
            let names = fields
                .iter()
                .map(|field| format!("{:?}", field.name))
                .collect::<Vec<_>>()
                .join(",");
            encode.push_str(&format!("if(PoolsterExtraParts is not null)foreach(var entry in PoolsterExtraParts){{if(new string[]{{{names}}}.Contains(entry.Key))throw new ArgumentException(\"Extra multipart part collides with declared field\");AddExtra(content,entry.Key,entry.Value);}}\n"));
        }
        let source = format!(
            "using System;\nusing System.Net.Http;\nusing System.Linq;\nusing System.Text.Json;\nnamespace {namespace};\npublic sealed record {name} {{\n{properties}\nprivate static void AddExtra(MultipartFormDataContent content,string name,object? value){{if(value is null)return;if(value is MultipartFile file){{content.Add(file.ToContent(),name,file.FileName);return;}}if(value is System.Collections.Generic.IEnumerable<MultipartFile> files){{foreach(var item in files)content.Add(item.ToContent(),name,item.FileName);return;}}var element=System.Text.Json.JsonSerializer.SerializeToElement(value,new System.Text.Json.JsonSerializerOptions(System.Text.Json.JsonSerializerDefaults.Web));if(element.ValueKind==System.Text.Json.JsonValueKind.Array){{foreach(var item in element.EnumerateArray())AddExtra(content,name,item);return;}}bool scalar=element.ValueKind is System.Text.Json.JsonValueKind.String or System.Text.Json.JsonValueKind.Number or System.Text.Json.JsonValueKind.True or System.Text.Json.JsonValueKind.False;content.Add(new StringContent(scalar?Scalar(value):element.GetRawText(),System.Text.Encoding.UTF8,scalar?\"text/plain\":\"application/json\"),name);}}\nprivate static string Scalar(object? value){{var element=System.Text.Json.JsonSerializer.SerializeToElement(value,new System.Text.Json.JsonSerializerOptions(System.Text.Json.JsonSerializerDefaults.Web));return element.ValueKind==System.Text.Json.JsonValueKind.String?element.GetString()!:element.GetRawText();}}\npublic MultipartFormDataContent ToContent(){{{checks}\nvar content=new MultipartFormDataContent();try{{{encode}return content;}}catch{{content.Dispose();throw;}}}}\n}}\n"
        );
        tree.insert(GeneratedFile::new(
            output_path(root, &format!("{name}.cs")),
            source,
        )?)?;
    }
    tree.insert(GeneratedFile::new(output_path(root,"MULTIPART.md"), "Multipart/form-data operations accept a generated <Operation>MultipartBody record. File fields use new MultipartFile(bytes, fileName, contentType); bytes are copied. Scalars are encoded with UTF-8/invariant culture and native MultipartFormDataContent. Multipart/form-data with explicit raw or JSON alternatives and object roots are supported. Open roots expose optional PoolsterExtraParts/poolsterExtraParts maps, rejecting collisions with declared parts. Root object unions merge fields and retain only shared required members; branch-specific constraints remain server-validated. Mixed-media calls accept native JSON models or explicit MultipartBody.RawBody/PoolsterRawBody buffered media wrappers. Objects/unions/reference values use JSON parts; arrays repeat parts by default and explode=false joins scalar values. Optional parts with null values are omitted. Base64 byte format, streaming and custom per-part headers require adapters. Part names use ASCII letters/digits/dot/underscore/hyphen; filenames use printable ASCII excluding quotes/backslashes. Files are buffered; no streaming/file-system access is implied. JSON model APIs remain separate.")?)?;
    Ok(())
}
