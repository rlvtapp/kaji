use super::*;

pub(crate) fn split_operations(
    api: &Api,
    config: &ArtifactOptions,
    cypress: bool,
) -> Result<Vec<GeneratedFile>> {
    let mut unsplit = config.clone();
    unsplit.max_file_bytes = usize::MAX;
    unsplit.layout = Some(poolster_core::SourceLayout::SingleFile);
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

pub(crate) fn local_cypress_config(api: &Api, config: &ArtifactOptions) -> ArtifactOptions {
    let mut config = config.clone();
    config.cypress_options.operation_overrides.retain(|id, _| {
        api.operations.iter().any(|operation| {
            operation
                .annotations
                .get("poolster.aux.operation_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(&operation.id)
                == id
        })
    });
    config
}
