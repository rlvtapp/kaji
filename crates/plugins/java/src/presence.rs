use super::*;
pub(crate) fn render(
    api: &Api,
    dir: &str,
    name: Option<&str>,
    style: SdkClientStyle,
    open_enums: bool,
    preserve: bool,
) -> Result<GeneratedTree> {
    let mut tree = render_sdk_with_policy(api, dir, name, style, open_enums)?;
    if !preserve {
        return Ok(tree);
    }
    let root = normalized_output_dir(dir)?;
    let root = if root.is_empty() || root == "." {
        String::new()
    } else {
        format!("{root}/")
    };
    let package =
        java_package_name(name.unwrap_or(&format!("io.kaji.{}", package_segment(&api.name))));
    let path = package.replace('.', "/");
    anyhow::ensure!(
        !api.schemas.iter().any(|s| type_name(&s.name) == "Presence"),
        "Presence model name reserved by preserve_presence"
    );
    for schema in &api.schemas {
        if let SchemaKind::Object { fields, .. } = &schema.value.kind {
            let mut source = render_model(schema, &package, open_enums);
            for field in fields.iter().filter(|f| !f.required) {
                source = source.replace(
                    &format!(
                        "@JsonProperty({:?}) {} {}",
                        field.name,
                        java_type(&field.value),
                        field_name(&field.name)
                    ),
                    &format!(
                        "@JsonProperty({:?}) Presence<{}> {}",
                        field.name,
                        java_type(&field.value),
                        field_name(&field.name)
                    ),
                );
            }
            // Convenience constructors must accept the same presence types as record components.
            for field in fields.iter().filter(|f| !f.required) {
                source = source.replace(
                    &format!("{} {}", java_type(&field.value), field_name(&field.name)),
                    &format!(
                        "Presence<{}> {}",
                        java_type(&field.value),
                        field_name(&field.name)
                    ),
                );
            }
            tree.replace(GeneratedFile::new(
                format!(
                    "{root}src/main/java/{path}/model/{}.java",
                    type_name(&schema.name)
                ),
                source,
            )?)?;
        }
    }
    tree.insert(GeneratedFile::new(
        format!("{root}src/main/java/{path}/model/Presence.java"),
        include_str!("presence.java.txt").replace("__PACKAGE__", &package),
    )?)?;
    tree.insert(GeneratedFile::new(format!("{root}PRESENCE.md"), "Optional fields use Presence<T> when preserve_presence is enabled. A null wrapper means omitted; Presence.of(null) sends explicit JSON null; Presence.of(value) sends a value. Required fields retain their native types. Deserialize with the generated Jackson annotations, then serialize with the same mapper to retain missing/null distinctions. This opt-in changes optional accessor/constructor types; default SDK ABI is unchanged. Unknown string enums require the separate open_enums setting. Union aliases retain raw JSON rather than promising complete variant validation.")?)?;
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
            Some("io.kaji.presence"),
            SdkClientStyle::Flat,
            false,
            true,
        )
        .unwrap();
        let source = tree
            .get("src/main/java/io/kaji/presence/model/Event.java")
            .unwrap();
        assert!(source.contains("Presence<String> note"));
        assert!(source.contains("Presence<Long> count"));
        assert!(source.contains("public Event(Presence<String> note, Presence<Long> count)"));
        assert!(
            tree.get("src/main/java/io/kaji/presence/model/Presence.java")
                .unwrap()
                .contains("getAbsentValue")
        );
        let default = render(
            &api(),
            ".",
            Some("io.kaji.presence"),
            SdkClientStyle::Flat,
            false,
            false,
        )
        .unwrap();
        assert!(
            !default
                .get("src/main/java/io/kaji/presence/model/Event.java")
                .unwrap()
                .contains("Presence<")
        );
    }
    #[test]
    #[ignore = "requires JDK17+Maven"]
    fn native_omitted_and_null_presence_roundtrip() {
        let tree = kaji_core::engine::Packages::new()
            .package(
                crate::package("sdk")
                    .name("io.kaji.presence")
                    .with(crate::sdk().preserve_presence(true))
                    .with(crate::operation_tests()),
            )
            .generate(&api(), None)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        std::fs::write(dir.path().join("sdk/src/test/java/io/kaji/presence/PresenceProbe.java"),r#"package io.kaji.presence;import io.kaji.presence.model.*;import com.fasterxml.jackson.databind.*;public class PresenceProbe{public static void main(String[]args)throws Exception{var mapper=new ObjectMapper();for(var wire:new String[]{"{}","{\"note\":null}","{\"note\":\"future\",\"count\":0}","{\"note\":null,\"future\":[false,0,null]}"}){var model=mapper.readValue(wire,Event.class);if(!mapper.readTree(wire).equals(mapper.readTree(mapper.writeValueAsString(model))))throw new AssertionError("presence wire");}var absent=mapper.readValue("{}",Event.class);if(absent.note()!=null)throw new AssertionError("omitted");var present=mapper.readValue("{\"note\":null}",Event.class);if(present.note()==null||present.note().value()!=null)throw new AssertionError("null");}}"#).unwrap();
        let output = std::process::Command::new("mvn")
            .args([
                "-q",
                "test-compile",
                "org.codehaus.mojo:exec-maven-plugin:3.5.0:java",
                "-Dexec.mainClass=io.kaji.presence.PresenceProbe",
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
