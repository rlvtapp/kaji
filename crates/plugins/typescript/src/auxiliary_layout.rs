//! AST-driven auxiliary splitting. Declarations are atomic; imports and public
//! entrypoints are rebuilt rather than slicing generated TypeScript text.
use crate::render::{self, ArtifactOptions};
use anyhow::Result;
use kaji_core::{AdditionalProperties, Api, GeneratedFile, SchemaKind, SchemaValue};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn prepare(api: &Api) -> Api {
    let mut prepared = crate::symbols::prepare(api);
    for (value, original) in prepared.schemas.iter_mut().zip(&api.schemas) {
        value
            .value
            .extensions
            .entry("kaji.aux.schema_name".into())
            .or_insert_with(|| original.name.clone().into());
    }
    for (value, original) in prepared.operations.iter_mut().zip(&api.operations) {
        value
            .annotations
            .entry("kaji.aux.operation_id".into())
            .or_insert_with(|| original.id.clone().into());
    }
    prepared
}

fn group_indices(
    lengths: impl Iterator<Item = usize>,
    config: &ArtifactOptions,
    resources: &[String],
) -> Result<Vec<Vec<usize>>> {
    let units = lengths
        .enumerate()
        .map(|(index, bytes)| kaji_core::SourceUnit {
            bytes,
            resource: resources.get(index).map(String::as_str),
        })
        .collect::<Vec<_>>();
    config
        .layout
        .clone()
        .unwrap_or_else(|| kaji_core::SourceLayout::chunked(config.max_file_bytes))
        .groups(&units, 1024)
}
pub(crate) fn uses_modules(
    api: &Api,
    config: &ArtifactOptions,
    bytes: usize,
    operations: bool,
) -> Result<bool> {
    let count = if operations {
        api.operations.len()
    } else {
        api.schemas.len() + api.operations.len()
    };
    let units = (0..count)
        .map(|_| kaji_core::SourceUnit {
            bytes: bytes / count.max(1),
            resource: None,
        })
        .collect::<Vec<_>>();
    config
        .layout
        .clone()
        .unwrap_or_else(|| kaji_core::SourceLayout::chunked(config.max_file_bytes))
        .uses_modules(&units, 0)
}
fn operation_resources(api: &Api) -> Vec<String> {
    api.operations
        .iter()
        .map(|operation| {
            operation
                .annotations
                .get("tags")
                .and_then(serde_json::Value::as_array)
                .and_then(|tags| tags.first())
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    operation
                        .path
                        .split('/')
                        .find(|s| !s.is_empty() && !s.starts_with('{'))
                        .unwrap_or("default")
                        .into()
                })
        })
        .collect()
}

pub(crate) fn references(value: &SchemaValue, names: &mut BTreeSet<String>) {
    match &value.kind {
        SchemaKind::Reference { reference } => {
            names.insert(reference.rsplit('/').next().unwrap_or(reference).into());
        }
        SchemaKind::Array { items } => references(items, names),
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            for field in fields {
                references(&field.value, names);
            }
            if let AdditionalProperties::Schema { value } = additional_properties {
                references(value, names);
            }
        }
        SchemaKind::OneOf { variants }
        | SchemaKind::AnyOf { variants }
        | SchemaKind::AllOf { variants } => {
            for value in variants {
                references(value, names);
            }
        }
        SchemaKind::Not { schema } => references(schema, names),
        _ => {}
    }
}

pub(crate) fn has_cycles(api: &Api) -> bool {
    let graph = api
        .schemas
        .iter()
        .map(|s| {
            let mut refs = BTreeSet::new();
            references(&s.value, &mut refs);
            (s.name.clone(), refs)
        })
        .collect::<BTreeMap<_, _>>();
    let mut remaining = graph.keys().cloned().collect::<BTreeSet<_>>();
    loop {
        let leaves = remaining
            .iter()
            .filter(|n| !graph[*n].iter().any(|r| remaining.contains(r)))
            .cloned()
            .collect::<Vec<_>>();
        if leaves.is_empty() {
            return !remaining.is_empty();
        }
        for n in leaves {
            remaining.remove(&n);
        }
    }
}

fn name(value: &str) -> String {
    render::type_identifier(value)
}
fn file(config: &ArtifactOptions, path: &str, source: String) -> Result<GeneratedFile> {
    GeneratedFile::new(
        render::extra_output_path(config, "typescript", path),
        source,
    )
}

pub(crate) fn faker(api: &Api, config: &ArtifactOptions) -> Result<Vec<GeneratedFile>> {
    crate::auxiliary_fixture::generate(api, config)
}

pub(crate) fn zod(api: &Api, config: &ArtifactOptions) -> Result<Vec<GeneratedFile>> {
    let groups = group_indices(
        api.schemas
            .iter()
            .map(|s| render::render_zod(&s.value).len() + s.name.len() * 8 + 300),
        config,
        &[],
    )?;
    let membership = groups
        .iter()
        .enumerate()
        .flat_map(|(chunk, indices)| {
            indices
                .iter()
                .map(move |&i| (api.schemas[i].name.clone(), chunk))
        })
        .collect::<BTreeMap<_, _>>();
    let mut files = Vec::new();
    let mut root = String::new();
    let mut schema_registries = Vec::new();
    for (chunk, indices) in groups.iter().enumerate() {
        let mut source = "import { z } from 'zod';\n".to_owned();
        let mut dependencies = BTreeSet::new();
        for &i in indices {
            references(&api.schemas[i].value, &mut dependencies);
        }
        for dependency in dependencies {
            let target = *membership
                .get(&dependency)
                .ok_or_else(|| anyhow::anyhow!("Zod reference has no component: {dependency}"))?;
            if target != chunk {
                source.push_str(&format!(
                    "import {{ {}Schema }} from './schemas_{target:04}';\n",
                    name(&dependency)
                ));
            }
        }
        for &i in indices {
            let s = &api.schemas[i];
            let n = name(&s.name);
            source.push_str(&format!("import type {{ {n} as __KajiModel{n} }} from '../models';\nexport const {n}Schema: z.ZodType<__KajiModel{n}> = {};\nexport type {n} = z.infer<typeof {n}Schema>;\n", render::render_zod(&s.value)));
            root.push_str(&format!("export {{ {n}Schema }} from './zod_chunks/schemas_{chunk:04}';\nexport type {{ {n} }} from './zod_chunks/schemas_{chunk:04}';\n"));
        }
        source.push_str("export const __kajiSchemas = {\n");
        for &i in indices {
            let s = &api.schemas[i];
            let key = s
                .value
                .extensions
                .get("kaji.aux.schema_name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(&s.name);
            source.push_str(&format!(
                "{}: {}Schema,\n",
                render::js_string(key),
                name(&s.name)
            ));
        }
        source.push_str("} as const;\n");
        root.push_str(&format!(
            "import {{ __kajiSchemas as schemas{chunk} }} from './zod_chunks/schemas_{chunk:04}';\n"
        ));
        schema_registries.push(format!("...schemas{chunk}"));
        files.push(file(
            config,
            &format!("zod_chunks/schemas_{chunk:04}.ts"),
            source,
        )?);
    }
    let operation_groups = group_indices(
        api.operations.iter().map(|o| {
            let local = Api {
                operations: vec![o.clone()],
                ..Default::default()
            };
            let mut text = String::new();
            render::render_zod_operation_schemas(&mut text, &local);
            text.len() + o.id.len() * 4 + 300
        }),
        config,
        &operation_resources(api),
    )?;
    let mut operation_registries = Vec::new();
    for (chunk, indices) in operation_groups.iter().enumerate() {
        let local = Api {
            operations: indices.iter().map(|&i| api.operations[i].clone()).collect(),
            ..Default::default()
        };
        let mut dependencies = BTreeSet::new();
        let mut scanned = local.clone();
        crate::json::visit_api(&mut scanned, &mut |value| {
            if let Some(n) = value.kind.reference_name() {
                dependencies.insert(n.to_owned());
            }
        });
        let mut source = "import { z } from 'zod';\n".to_owned();
        for dependency in dependencies {
            let target = *membership.get(&dependency).ok_or_else(|| {
                anyhow::anyhow!("Zod operation reference has no component: {dependency}")
            })?;
            source.push_str(&format!(
                "import {{ {}Schema, type {} }} from './schemas_{target:04}';\n",
                name(&dependency),
                name(&dependency)
            ));
        }
        render::render_zod_operation_schemas(&mut source, &local);
        render::render_zod_operation_registry(&mut source, &local, "__kajiOperationSchemas");
        root.push_str(&format!("import {{ __kajiOperationSchemas as operations{chunk} }} from './zod_chunks/operations_{chunk:04}';\n"));
        operation_registries.push(format!("...operations{chunk}"));
        for operation in &local.operations {
            let n = name(&operation.id);
            if operation
                .request_body
                .as_ref()
                .is_some_and(|b| b.media_types.iter().any(|m| m.schema.is_some()))
            {
                root.push_str(&format!("export {{ {n}RequestBodySchemas }} from './zod_chunks/operations_{chunk:04}';\n"));
            }
            if operation
                .responses
                .iter()
                .any(|r| r.media_types.iter().any(|m| m.schema.is_some()))
            {
                root.push_str(&format!(
                    "export {{ {n}ResponseSchemas }} from './zod_chunks/operations_{chunk:04}';\n"
                ));
            }
        }
        files.push(file(
            config,
            &format!("zod_chunks/operations_{chunk:04}.ts"),
            source,
        )?);
    }
    root.push_str(&format!("export const kajiSchemas: {} = {{ {} }} as const;\nexport type KajiSchemaName = keyof typeof kajiSchemas;\nexport type KajiSchema = (typeof kajiSchemas)[KajiSchemaName];\nexport const getKajiSchema = <Name extends KajiSchemaName>(name: Name): (typeof kajiSchemas)[Name] => kajiSchemas[name];\n", if schema_registries.is_empty() { "Record<never, never>".into() } else { (0..schema_registries.len()).map(|i| format!("typeof schemas{i}")).collect::<Vec<_>>().join(" & ") }, schema_registries.join(", ")));
    root.push_str(&format!("export const kajiOperationSchemas: {} = {{ {} }} as const;\nexport type KajiOperationId = keyof typeof kajiOperationSchemas;\nexport type KajiOperationSchemas = typeof kajiOperationSchemas;\nexport const getKajiOperationSchemas = <Operation extends KajiOperationId>(operation: Operation): KajiOperationSchemas[Operation] => kajiOperationSchemas[operation];\n", if operation_registries.is_empty() { "Record<never, never>".into() } else { (0..operation_registries.len()).map(|i| format!("typeof operations{i}")).collect::<Vec<_>>().join(" & ") }, operation_registries.join(", ")));
    files.push(file(config, "zod.ts", root)?);
    Ok(files)
}

fn split_operations(
    api: &Api,
    config: &ArtifactOptions,
    cypress: bool,
) -> Result<Vec<GeneratedFile>> {
    let mut unsplit = config.clone();
    unsplit.max_file_bytes = usize::MAX;
    unsplit.layout = Some(kaji_core::SourceLayout::SingleFile);
    let lengths = api
        .operations
        .iter()
        .map(|o| -> Result<usize> {
            let local = Api {
                operations: vec![o.clone()],
                ..api.clone()
            };
            let files = if cypress {
                render::TypeScriptCypress.generate(&local, &local_cypress_config(&local, &unsplit))
            } else {
                render::TypeScriptMsw.generate(&local, &unsplit)
            }?;
            Ok(files[0].contents.len())
        })
        .collect::<Result<Vec<_>>>()?;
    let groups = group_indices(lengths.into_iter(), config, &operation_resources(api))?;
    let kind = if cypress { "cypress" } else { "msw" };
    let mut files = Vec::new();
    let mut root = String::new();
    let mut handlers = Vec::new();
    for (chunk, indices) in groups.iter().enumerate() {
        let local = Api {
            operations: indices.iter().map(|&i| api.operations[i].clone()).collect(),
            ..api.clone()
        };
        let generated = if cypress {
            render::TypeScriptCypress.generate(&local, &local_cypress_config(&local, &unsplit))?
        } else {
            render::TypeScriptMsw.generate(&local, &unsplit)?
        };
        let source = generated[0].contents.clone();
        if cypress {
            root.push_str(&format!("import './{kind}_chunks/chunk_{chunk:04}';\n"));
        } else {
            root.push_str(&format!("import {{ handlers as handlers{chunk} }} from './{kind}_chunks/chunk_{chunk:04}';\n"));
            handlers.push(format!("...handlers{chunk}"));
        }
        files.push(GeneratedFile::new(
            render::extra_output_path(
                config,
                if cypress { "cypress/e2e" } else { "typescript" },
                &format!("{kind}_chunks/chunk_{chunk:04}.ts"),
            ),
            source,
        )?);
    }
    if !cypress {
        root.push_str(&format!("import type {{ HttpHandler }} from 'msw';\nexport const handlers: HttpHandler[] = [{}];\n", handlers.join(", ")));
    }
    files.push(GeneratedFile::new(
        render::extra_output_path(
            config,
            if cypress { "cypress/e2e" } else { "typescript" },
            if cypress { "api.cy.ts" } else { "msw.ts" },
        ),
        root,
    )?);
    Ok(files)
}
pub(crate) fn msw(api: &Api, config: &ArtifactOptions) -> Result<Vec<GeneratedFile>> {
    split_operations(api, config, false)
}
pub(crate) fn cypress(api: &Api, config: &ArtifactOptions) -> Result<Vec<GeneratedFile>> {
    split_operations(api, config, true)
}

fn local_cypress_config(api: &Api, config: &ArtifactOptions) -> ArtifactOptions {
    let mut config = config.clone();
    config.cypress_options.operation_overrides.retain(|id, _| {
        api.operations.iter().any(|operation| {
            operation
                .annotations
                .get("kaji.aux.operation_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(&operation.id)
                == id
        })
    });
    config
}

/// Standalone models share the same declaration budget as auxiliary artifacts.
pub(crate) fn models(api: &Api, config: &ArtifactOptions) -> Result<Vec<GeneratedFile>> {
    let api = prepare(api);
    let declarations = api
        .schemas
        .iter()
        .map(|schema| {
            let mut source = String::new();
            render::render_description(&mut source, schema.value.description.as_deref());
            source.push_str(&format!(
                "export type {} = {};\n",
                render::type_identifier(&schema.name),
                render::render_value(&schema.value)
            ));
            source
        })
        .collect::<Vec<_>>();
    let groups = group_indices(declarations.iter().map(String::len), config, &[])?;
    let layout = config
        .layout
        .clone()
        .unwrap_or_else(|| kaji_core::SourceLayout::chunked(config.max_file_bytes));
    let units = declarations
        .iter()
        .map(|source| kaji_core::SourceUnit {
            bytes: source.len(),
            resource: None,
        })
        .collect::<Vec<_>>();
    if !layout.uses_modules(&units, 1024)? {
        return Ok(vec![GeneratedFile::new(
            render::output_path(config, "models.ts"),
            declarations.join("\n"),
        )?]);
    }
    let mut membership = BTreeMap::new();
    for (chunk, indices) in groups.iter().enumerate() {
        for &i in indices {
            membership.insert(api.schemas[i].name.clone(), chunk);
        }
    }
    let mut files = Vec::new();
    let mut root = String::new();
    for (chunk, indices) in groups.iter().enumerate() {
        let mut source = String::new();
        let mut dependencies = BTreeSet::new();
        for &i in indices {
            references(&api.schemas[i].value, &mut dependencies);
        }
        for dependency in dependencies {
            let target = *membership
                .get(&dependency)
                .ok_or_else(|| anyhow::anyhow!("Model reference has no component: {dependency}"))?;
            if target != chunk {
                source.push_str(&format!(
                    "import type {{ {} }} from './chunk_{target:04}';\n",
                    render::type_identifier(&dependency)
                ));
            }
        }
        for &i in indices {
            source.push_str(&declarations[i]);
        }
        files.push(GeneratedFile::new(
            render::output_path(config, &format!("models_chunks/chunk_{chunk:04}.ts")),
            source,
        )?);
        root.push_str(&format!(
            "export type * from './models_chunks/chunk_{chunk:04}';\n"
        ));
    }
    files.push(GeneratedFile::new(
        render::output_path(config, "models.ts"),
        root,
    )?);
    Ok(files)
}
