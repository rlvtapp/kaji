use super::*;
pub(crate) fn render(
    api: &Api,
    dir: &str,
    name: Option<&str>,
    style: SdkClientStyle,
    open_enums: bool,
    preserve: bool,
) -> Result<GeneratedTree> {
    let prepared = prepare_api(api);
    let api = &prepared;
    let mut tree = render_sdk_with_policy(api, dir, name, style, open_enums)?;
    if !preserve {
        return Ok(tree);
    }
    let root = normalized_root(dir)?;
    let package = name
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{}-sdk", kebab_case(&api.name)));
    let namespace = dotnet_namespace(&package);
    anyhow::ensure!(
        !api.schemas.iter().any(|s| matches!(
            pascal_case(&s.name).as_str(),
            "Presence" | "PresenceJsonConverterFactory"
        )),
        "Presence model name reserved by preserve_presence"
    );
    for (index, schema) in api.schemas.iter().enumerate() {
        if let SchemaKind::Object { fields, .. } = &schema.value.kind {
            let name = pascal_case(&schema.name);
            let names =
                native_names::field_names(fields, pascal_case, &[&name, "EqualityContract"]);
            for (filename, mut source) in render_model_parts(schema, &namespace, open_enums, index)?
            {
                for field in fields.iter().filter(|f| !f.required) {
                    let ty = csharp_type(&field.value, true);
                    let property = names[&field.name].clone();
                    source = source.replace(&format!("    public {ty} {property} {{ get; init; }}"), &format!("    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingDefault)]\n    public Presence<{ty}> {property} {{ get; init; }}"));
                }
                tree.replace(GeneratedFile::new(
                    output_path(&root, &format!("./Models/{filename}")),
                    source,
                )?)?;
            }
        }
    }
    tree.insert(GeneratedFile::new(
        output_path(&root, "Presence.cs"),
        include_str!("presence.cs.txt").replace("__NAMESPACE__", &namespace),
    )?)?;
    tree.insert(GeneratedFile::new(output_path(&root,"PRESENCE.md"), "Optional fields use Presence<T> when preserve_presence is enabled. default(Presence<T>) means omitted; Presence<T>.Present(null) sends explicit JSON null; Presence<T>.Present(value) sends a value. Required fields retain their native types. Generated System.Text.Json converters and property omission attributes preserve these distinctions. This opt-in changes optional property types; default SDK ABI is unchanged. Unknown string enums require the separate open_enums setting. Union aliases retain raw JSON rather than promising complete variant validation.")?)?;
    Ok(tree)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn api() -> Api {
        let mut note = SchemaValue::new(SchemaKind::String);
        note.nullable = true;
        Api {
            name: "Presence".into(),
            schemas: vec![Schema::new(
                "Event",
                SchemaValue::new(SchemaKind::Object {
                    fields: vec![
                        kaji_core::Field {
                            name: "note".into(),
                            value: note,
                            required: false,
                            annotations: Default::default(),
                        },
                        kaji_core::Field {
                            name: "count".into(),
                            value: SchemaValue::new(SchemaKind::Integer),
                            required: false,
                            annotations: Default::default(),
                        },
                    ],
                    additional_properties: AdditionalProperties::Any,
                }),
            )],
            ..Default::default()
        }
    }
    #[test]
    fn opt_in_presence_changes_only_optional_fields() {
        let tree = render(
            &api(),
            ".",
            Some("Kaji.Presence"),
            SdkClientStyle::Flat,
            false,
            true,
        )
        .unwrap();
        let source = tree
            .get(format!("./Models/{}", bounded_filename("Event", 0, "cs")))
            .unwrap();
        assert!(source.contains("Presence<string?> Note"));
        assert!(source.contains("Presence<long?> Count"));
        assert!(source.contains("WhenWritingDefault"));
        let default = render(
            &api(),
            ".",
            Some("Kaji.Presence"),
            SdkClientStyle::Flat,
            false,
            false,
        )
        .unwrap();
        assert!(default.get("./Presence.cs").is_none());
    }
    #[test]
    fn presence_policy_uses_collision_safe_property_names() {
        let mut api = api();
        api.schemas[0].name = "Note".into();
        let tree = render(
            &api,
            ".",
            Some("Kaji.Presence"),
            SdkClientStyle::Flat,
            true,
            true,
        )
        .unwrap();
        let source = tree
            .get(format!("./Models/{}", bounded_filename("Note", 0, "cs")))
            .unwrap();
        assert!(source.contains("Presence<string?> NoteValue"));
    }
    #[test]
    #[ignore = "requires .NET8"]
    fn native_omitted_and_null_presence_roundtrip() {
        let tree = kaji_core::engine::Packages::new()
            .package(
                crate::package("sdk")
                    .name("Kaji.Presence")
                    .with(crate::sdk().preserve_presence(true))
                    .with(crate::operation_tests()),
            )
            .generate(&api(), None)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        std::fs::write(dir.path().join("sdk/tests/OperationTests/Program.cs"),r#"using System.Text.Json;using Kaji.KajiPresence;class Probe{static void Main(){foreach(var wire in new[]{"{}","{\"note\":null}","{\"note\":\"future\",\"count\":0}","{\"note\":null,\"future\":[false,0,null]}"}){var model=JsonSerializer.Deserialize<EventValue>(wire)!;if(JsonSerializer.Serialize(model)!=wire)throw new Exception("presence wire");}if(JsonSerializer.Deserialize<EventValue>("{}")!.Note.IsPresent)throw new Exception("omitted");var present=JsonSerializer.Deserialize<EventValue>("{\"note\":null}")!.Note;if(!present.IsPresent||present.Value!=null)throw new Exception("null");}}"#).unwrap();
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
