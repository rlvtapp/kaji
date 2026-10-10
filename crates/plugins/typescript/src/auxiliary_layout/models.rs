use super::*;

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
        .unwrap_or_else(|| poolster_core::SourceLayout::chunked(config.max_file_bytes));
    let units = declarations
        .iter()
        .map(|source| poolster_core::SourceUnit {
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
