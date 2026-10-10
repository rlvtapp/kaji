use super::*;

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
            source.push_str(&format!("import type {{ {n} as __PoolsterModel{n} }} from '../models';\nexport const {n}Schema: z.ZodType<__PoolsterModel{n}> = {};\nexport type {n} = z.infer<typeof {n}Schema>;\n", render::render_zod(&s.value)));
            root.push_str(&format!("export {{ {n}Schema }} from './zod_chunks/schemas_{chunk:04}';\nexport type {{ {n} }} from './zod_chunks/schemas_{chunk:04}';\n"));
        }
        source.push_str("export const __poolsterSchemas = {\n");
        for &i in indices {
            let s = &api.schemas[i];
            let key = s
                .value
                .extensions
                .get("poolster.aux.schema_name")
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
            "import {{ __poolsterSchemas as schemas{chunk} }} from './zod_chunks/schemas_{chunk:04}';\n"
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
        render::render_zod_operation_registry(&mut source, &local, "__poolsterOperationSchemas");
        root.push_str(&format!("import {{ __poolsterOperationSchemas as operations{chunk} }} from './zod_chunks/operations_{chunk:04}';\n"));
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
    root.push_str(&format!("export const poolsterSchemas: {} = {{ {} }} as const;\nexport type PoolsterSchemaName = keyof typeof poolsterSchemas;\nexport type PoolsterSchema = (typeof poolsterSchemas)[PoolsterSchemaName];\nexport const getPoolsterSchema = <Name extends PoolsterSchemaName>(name: Name): (typeof poolsterSchemas)[Name] => poolsterSchemas[name];\n", if schema_registries.is_empty() { "Record<never, never>".into() } else { (0..schema_registries.len()).map(|i| format!("typeof schemas{i}")).collect::<Vec<_>>().join(" & ") }, schema_registries.join(", ")));
    root.push_str(&format!("export const poolsterOperationSchemas: {} = {{ {} }} as const;\nexport type PoolsterOperationId = keyof typeof poolsterOperationSchemas;\nexport type PoolsterOperationSchemas = typeof poolsterOperationSchemas;\nexport const getPoolsterOperationSchemas = <Operation extends PoolsterOperationId>(operation: Operation): PoolsterOperationSchemas[Operation] => poolsterOperationSchemas[operation];\n", if operation_registries.is_empty() { "Record<never, never>".into() } else { (0..operation_registries.len()).map(|i| format!("typeof operations{i}")).collect::<Vec<_>>().join(" & ") }, operation_registries.join(", ")));
    files.push(file(config, "zod.ts", root)?);
    Ok(files)
}
