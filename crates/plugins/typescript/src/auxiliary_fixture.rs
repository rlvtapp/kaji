//! Lossless AST descriptors for seeded, constraint-checked fixture construction.
use crate::render::{self, ArtifactOptions};
use anyhow::{Result, ensure};
use poolster_core::{Api, GeneratedFile, SchemaValue, SourceLayout, SourceUnit};
use serde_json::{Value, json};

pub(crate) fn literal(value: &Value) -> Value {
    match value {
        Value::Null => json!({"kind":"null","value":null}),
        Value::Bool(value) => json!({"kind":"boolean","value":value}),
        Value::String(value) => json!({"kind":"string","value":value}),
        Value::Number(value) => json!({"kind":"number","value":value.to_string()}),
        Value::Array(values) => {
            json!({"kind":"array","value":values.iter().map(literal).collect::<Vec<_>>()})
        }
        Value::Object(values) => {
            json!({"kind":"object","value":values.iter().map(|(key,value)|(key.clone(),literal(value))).collect::<serde_json::Map<_,_>>()})
        }
    }
}
pub(crate) fn descriptor(schema: &SchemaValue) -> Value {
    let mut value = serde_json::to_value(schema).expect("schema serialization is infallible");
    fn rewrite(value: &mut Value) {
        match value {
            Value::Object(object)
                if object.contains_key("kind") && object.contains_key("nullable") =>
            {
                for key in [
                    "description",
                    "title",
                    "deprecated",
                    "read_only",
                    "write_only",
                ] {
                    object.remove(key);
                }
                if let Some(Value::Object(extensions)) = object.get_mut("extensions") {
                    extensions.retain(|key, _| key == "x-kaji-integer");
                }
                for key in ["const_value", "default"] {
                    if let Some(value) = object.get_mut(key) {
                        *value = literal(value);
                    }
                }
                if let Some(Value::Array(values)) = object.get_mut("enum_values") {
                    for value in values {
                        *value = literal(value);
                    }
                }
                if let Some(Value::Object(constraints)) = object.get_mut("constraints") {
                    for value in constraints.values_mut() {
                        if let Value::Number(number) = value {
                            *value = Value::String(number.to_string());
                        }
                    }
                }
                if let Some(kind) = object.get_mut("kind") {
                    rewrite(kind);
                }
            }
            Value::Object(object) => {
                for value in object.values_mut() {
                    rewrite(value);
                }
            }
            Value::Array(values) => {
                for value in values {
                    rewrite(value);
                }
            }
            _ => {}
        }
    }
    rewrite(&mut value);
    value
}
fn file(config: &ArtifactOptions, path: &str, source: String) -> Result<GeneratedFile> {
    GeneratedFile::new(
        render::extra_output_path(config, "typescript", path),
        source,
    )
}

pub(crate) fn generate(api: &Api, config: &ArtifactOptions) -> Result<Vec<GeneratedFile>> {
    let options = &config.fixture_options;
    ensure!(
        options.max_depth > 0 && options.max_depth <= 64,
        "fixture max_depth must be between1 and64"
    );
    ensure!(
        options.max_attempts > 0 && options.max_attempts <= 1024,
        "fixture max_attempts must be between1 and1024"
    );
    ensure!(
        options.seed.is_none_or(|seed| seed <= u32::MAX as u64),
        "fixture seed must fit an unsigned32-bit integer"
    );
    let descriptors = api
        .schemas
        .iter()
        .map(|schema| descriptor(&schema.value))
        .collect::<Vec<_>>();
    let units = descriptors
        .iter()
        .map(|value| SourceUnit {
            bytes: value.to_string().len() + 256,
            resource: Some("models"),
        })
        .collect::<Vec<_>>();
    let layout = config
        .layout
        .clone()
        .unwrap_or_else(|| SourceLayout::chunked(config.max_file_bytes));
    let groups = layout.groups(&units, 1024)?;
    let split = layout.uses_modules(&units, 1024)?;
    let mut overrides = serde_json::Map::new();
    for (key, value) in &options.overrides {
        let schema = api.schemas.iter().find(|schema| {
            schema
                .value
                .extensions
                .get("poolster.aux.schema_name")
                .and_then(Value::as_str)
                .unwrap_or(&schema.name)
                == key
        });
        ensure!(
            schema.is_some(),
            "fixture override refers to unknown component {key}"
        );
        overrides.insert(schema.unwrap().name.clone(), literal(value));
    }
    let settings = json!({"seed":options.seed,"max_depth":options.max_depth,"max_attempts":options.max_attempts,"overrides":overrides});
    let settings = if options.seed.is_none() {
        let mut v = settings;
        v.as_object_mut().unwrap().remove("seed");
        v
    } else {
        settings
    };
    let runtime = include_str!("fixture_runtime.ts.txt");
    let factory = |indices: &[usize], prefix: &str| {
        let mut source = String::new();
        for &index in indices {
            let schema = &api.schemas[index];
            let name = render::type_identifier(&schema.name);
            source.push_str(&format!("import type {{ {name} }} from '{prefix}models';\nexport function create{name}(__depth = 0): {name} {{ return __kajiFixtures.create({}, __depth) as {name}; }}\n",render::js_string(&schema.name)));
        }
        source
    };
    if !split {
        let mut source = runtime.to_owned();
        source.push_str(&format!("\nconst __kajiSchemas: Record<string, FixtureSchema> = {{ {} }};\nconst __kajiFixtures = createFixtureRuntime(__kajiSchemas, {settings});\nexport const seedPoolsterFixtures = (seed: number | number[]) => __kajiFixtures.seed(seed);\n",api.schemas.iter().zip(&descriptors).map(|(schema,value)|format!("{}: {value}",render::js_string(&schema.name))).collect::<Vec<_>>().join(",")));
        source.push_str(&factory(&(0..api.schemas.len()).collect::<Vec<_>>(), "./"));
        return Ok(vec![file(config, "faker.ts", source)?]);
    }
    let mut files = Vec::new();
    let mut root = "export { seedPoolsterFixtures } from './faker_chunks/runtime';\n".to_owned();
    let mut schemas = "import type { FixtureSchema } from './runtime';\n".to_owned();
    let mut registries = Vec::new();
    for (chunk, indices) in groups.iter().enumerate() {
        let shapes = format!(
            "import type {{ FixtureSchema }} from './runtime';\nexport const shapes: Record<string, FixtureSchema> = {{ {} }};\n",
            indices
                .iter()
                .map(|&index| format!(
                    "{}: {}",
                    render::js_string(&api.schemas[index].name),
                    descriptors[index]
                ))
                .collect::<Vec<_>>()
                .join(",")
        );
        files.push(file(
            config,
            &format!("faker_chunks/shapes_{chunk:04}.ts"),
            shapes,
        )?);
        let source = format!(
            "import {{ __kajiFixtures }} from './runtime';\n{}",
            factory(indices, "../")
        );
        files.push(file(
            config,
            &format!("faker_chunks/chunk_{chunk:04}.ts"),
            source,
        )?);
        schemas.push_str(&format!(
            "import {{ shapes as shapes{chunk} }} from './shapes_{chunk:04}';\n"
        ));
        registries.push(format!("...shapes{chunk}"));
        root.push_str(&format!(
            "export * from './faker_chunks/chunk_{chunk:04}';\n"
        ));
    }
    schemas.push_str(&format!(
        "export const schemas: Record<string, FixtureSchema> = {{ {} }};\n",
        registries.join(",")
    ));
    files.push(file(config, "faker_chunks/schemas.ts", schemas)?);
    files.push(file(config,"faker_chunks/runtime.ts",format!("{runtime}\nimport {{ schemas }} from './schemas';\nexport const __kajiFixtures = createFixtureRuntime(schemas, {settings});\nexport const seedPoolsterFixtures = (seed: number | number[]) => __kajiFixtures.seed(seed);\n"))?);
    files.push(file(config, "faker.ts", root)?);
    Ok(files)
}
