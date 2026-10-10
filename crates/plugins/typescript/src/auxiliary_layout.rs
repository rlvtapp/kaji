//! AST-driven auxiliary splitting. Declarations are atomic; imports and public
//! entrypoints are rebuilt rather than slicing generated TypeScript text.
use crate::render::{self, ArtifactOptions};
use anyhow::Result;
use poolster_core::{AdditionalProperties, Api, GeneratedFile, SchemaKind, SchemaValue};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn prepare(api: &Api) -> Api {
    let mut prepared = crate::symbols::prepare(api);
    for (value, original) in prepared.schemas.iter_mut().zip(&api.schemas) {
        value
            .value
            .extensions
            .entry("poolster.aux.schema_name".into())
            .or_insert_with(|| original.name.clone().into());
    }
    for (value, original) in prepared.operations.iter_mut().zip(&api.operations) {
        value
            .annotations
            .entry("poolster.aux.operation_id".into())
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
        .map(|(index, bytes)| poolster_core::SourceUnit {
            bytes,
            resource: resources.get(index).map(String::as_str),
        })
        .collect::<Vec<_>>();
    config
        .layout
        .clone()
        .unwrap_or_else(|| poolster_core::SourceLayout::chunked(config.max_file_bytes))
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
        .map(|_| poolster_core::SourceUnit {
            bytes: bytes / count.max(1),
            resource: None,
        })
        .collect::<Vec<_>>();
    config
        .layout
        .clone()
        .unwrap_or_else(|| poolster_core::SourceLayout::chunked(config.max_file_bytes))
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

mod zod;
pub(crate) use zod::*;

mod operations;
pub(crate) use operations::*;

mod models;
pub(crate) use models::*;
