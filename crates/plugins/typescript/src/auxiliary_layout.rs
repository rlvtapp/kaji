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

fn group_indices(lengths: impl Iterator<Item = usize>, budget: usize) -> Vec<Vec<usize>> {
    let mut result = Vec::new();
    let mut current = Vec::new();
    let mut bytes = 1024;
    for (index, length) in lengths.enumerate() {
        if bytes + length > budget && !current.is_empty() {
            result.push(std::mem::take(&mut current));
            bytes = 1024;
        }
        current.push(index);
        bytes += length;
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

fn references(value: &SchemaValue, names: &mut BTreeSet<String>) {
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

fn bounded_faker(value: &SchemaValue) -> String {
    if value.const_value.is_some() || !value.enum_values.is_empty() {
        return render::render_faker(value);
    }
    let expression = match &value.kind {
        SchemaKind::Reference { reference } => format!(
            "create{}(__depth + 1)",
            name(reference.rsplit('/').next().unwrap_or(reference))
        ),
        SchemaKind::Array { items } => format!(
            "Array.from({{ length: __depth >= 4 ? 0 : 2 }}, () => {})",
            bounded_faker(items)
        ),
        SchemaKind::Object { fields, .. } => format!(
            "{{ {} }}",
            fields
                .iter()
                .map(|field| {
                    let entry = format!(
                        "{}: {}",
                        render::js_string(&field.name),
                        bounded_faker(&field.value)
                    );
                    if field.required {
                        entry
                    } else {
                        format!("...(__depth >= 4 ? {{}} : {{ {entry} }})")
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
        SchemaKind::OneOf { variants }
        | SchemaKind::AnyOf { variants }
        | SchemaKind::AllOf { variants } => variants
            .first()
            .map(bounded_faker)
            .unwrap_or_else(|| "undefined".into()),
        _ => render::render_faker(value),
    };
    if value.nullable {
        format!("(__depth >= 4 ? null : {expression})")
    } else {
        expression
    }
}

pub(crate) fn faker(api: &Api, config: &ArtifactOptions) -> Result<Vec<GeneratedFile>> {
    let recursive = has_cycles(api);
    let groups = group_indices(
        api.schemas
            .iter()
            .map(|s| render::render_faker(&s.value).len() + s.name.len() * 4 + 160),
        config.max_file_bytes,
    );
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
    for (chunk, indices) in groups.iter().enumerate() {
        let mut source = "import { faker } from '@faker-js/faker';\n".to_owned();
        let mut dependencies = BTreeSet::new();
        for &i in indices {
            references(&api.schemas[i].value, &mut dependencies);
        }
        for dependency in dependencies {
            let target = *membership
                .get(&dependency)
                .ok_or_else(|| anyhow::anyhow!("Faker reference has no component: {dependency}"))?;
            if target != chunk {
                source.push_str(&format!(
                    "import {{ create{} }} from './chunk_{target:04}';\n",
                    name(&dependency)
                ));
            }
        }
        for &i in indices {
            let schema = &api.schemas[i];
            let n = name(&schema.name);
            let expression = if recursive {
                bounded_faker(&schema.value)
            } else {
                render::render_faker(&schema.value)
            };
            let parameter = if recursive { "__depth = 0" } else { "" };
            let guard = if recursive {
                "if (__depth > 8) throw new RangeError('Cannot construct finite data for a required recursive schema'); "
            } else {
                ""
            };
            source.push_str(&format!("import type {{ {n} }} from '../models';\nexport function create{n}({parameter}): {n} {{ {guard}return {expression} as {n}; }}\n"));
        }
        root.push_str(&format!(
            "export * from './faker_chunks/chunk_{chunk:04}';\n"
        ));
        files.push(file(
            config,
            &format!("faker_chunks/chunk_{chunk:04}.ts"),
            source,
        )?);
    }
    files.push(file(config, "faker.ts", root)?);
    Ok(files)
}

pub(crate) fn zod(api: &Api, config: &ArtifactOptions) -> Result<Vec<GeneratedFile>> {
    let groups = group_indices(
        api.schemas
            .iter()
            .map(|s| render::render_zod(&s.value).len() + s.name.len() * 8 + 300),
        config.max_file_bytes,
    );
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
        config.max_file_bytes,
    );
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
                "import {{ {}Schema }} from './schemas_{target:04}';\n",
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
    root.push_str(&format!("export const kajiSchemas = {{ {} }} as const;\nexport type KajiSchemaName = keyof typeof kajiSchemas;\nexport type KajiSchema = (typeof kajiSchemas)[KajiSchemaName];\nexport const getKajiSchema = <Name extends KajiSchemaName>(name: Name): (typeof kajiSchemas)[Name] => kajiSchemas[name];\n", schema_registries.join(", ")));
    root.push_str(&format!("export const kajiOperationSchemas = {{ {} }} as const;\nexport type KajiOperationId = keyof typeof kajiOperationSchemas;\nexport type KajiOperationSchemas = typeof kajiOperationSchemas;\nexport const getKajiOperationSchemas = <Operation extends KajiOperationId>(operation: Operation): KajiOperationSchemas[Operation] => kajiOperationSchemas[operation];\n", operation_registries.join(", ")));
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
    let lengths = api
        .operations
        .iter()
        .map(|o| -> Result<usize> {
            let local = Api {
                operations: vec![o.clone()],
                ..api.clone()
            };
            let files = if cypress {
                render::TypeScriptCypress.generate(&local, &unsplit)
            } else {
                render::TypeScriptMsw.generate(&local, &unsplit)
            }?;
            Ok(files[0].contents.len())
        })
        .collect::<Result<Vec<_>>>()?;
    let groups = group_indices(lengths.into_iter(), config.max_file_bytes);
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
            render::TypeScriptCypress.generate(&local, &unsplit)?
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
