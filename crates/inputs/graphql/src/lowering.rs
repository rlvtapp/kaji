use anyhow::{Result, anyhow, ensure};
use apollo_compiler::{
    ExecutableDocument, Schema,
    ast::{DirectiveList, OperationType, Type, Value},
    executable::{Selection, SelectionSet},
    schema::ExtendedType,
    validation::Valid,
};
use poolster_core::native::*;
use std::collections::BTreeMap;

pub fn lower(
    schema: &Valid<Schema>,
    schema_source: &str,
    source: &str,
) -> Result<GraphqlOperations> {
    lower_internal(schema, schema_source, source, false).map(|(definition, _)| definition)
}
pub fn lower_incremental(
    schema: &Valid<Schema>,
    schema_source: &str,
    source: &str,
) -> Result<GraphqlIncrementalOperations> {
    let (definition, selections) = lower_internal(schema, schema_source, source, true)?;
    ensure!(
        selections.values().any(|s| !s.is_empty()),
        "incremental input needs @defer or @stream selections"
    );
    Ok(GraphqlIncrementalOperations {
        dialect: GraphqlIncrementalDialect::DeferSpec20220824,
        definition,
        selections,
    })
}
fn lower_internal(
    schema: &Valid<Schema>,
    schema_source: &str,
    source: &str,
    incremental: bool,
) -> Result<(
    GraphqlOperations,
    BTreeMap<String, Vec<GraphqlIncrementalSelection>>,
)> {
    let doc = ExecutableDocument::parse_and_validate(schema, source, "operations.graphql")
        .map_err(|e| anyhow!("invalid GraphQL operations: {e}"))?;
    let mut input_objects = input_objects(schema);
    let mut operations = Vec::new();
    let mut incremental_selections = BTreeMap::new();
    for op in doc
        .operations
        .anonymous
        .iter()
        .chain(doc.operations.named.values())
    {
        ensure!(
            op.directives.0.is_empty(),
            "operation directives are not supported for generation"
        );
        let name = op
            .name
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "Anonymous".into());
        ensure!(
            op.variables.iter().all(|v| v.directives.0.is_empty()),
            "variable directives are unsupported for generation"
        );
        let variables = op
            .variables
            .iter()
            .map(|v| ModelField {
                name: v.name.to_string(),
                ty: typed(schema, &v.ty, None),
                optional: !v.ty.is_non_null() || v.default_value.is_some(),
                default_value: v.default_value.as_ref().map(ToString::to_string),
            })
            .collect();
        let kind = match op.operation_type {
            OperationType::Query => GraphqlOperationKind::Query,
            OperationType::Mutation => GraphqlOperationKind::Mutation,
            OperationType::Subscription => GraphqlOperationKind::Subscription,
        };
        // Each request sends one operation plus only its transitively referenced fragments.
        let mut fragments = BTreeMap::new();
        collect_fragments(&doc, &op.selection_set, &mut fragments);
        // Anonymous operations gain a stable executable name matching the client envelope.
        let mut executable = (**op).clone();
        if executable.name.is_none() {
            executable.name =
                Some(apollo_compiler::Name::new(&name).expect("valid generated operation name"));
        }
        let mut document = executable.to_string();
        for fragment in fragments.values() {
            document.push('\n');
            document.push_str(fragment);
        }
        let mut selections = Vec::new();
        incremental_metadata(&doc, &op.selection_set, &[], incremental, &mut selections)?;
        ensure!(
            selections.is_empty() || kind == GraphqlOperationKind::Query,
            "incremental delivery currently supports queries only"
        );
        let mut labels = std::collections::BTreeSet::new();
        for selection in &selections {
            if let Some(label) = &selection.label {
                ensure!(
                    labels.insert(label.clone()),
                    "duplicate incremental label {label}"
                );
            }
        }
        if incremental {
            validate_initial_unions(&selection_mode(schema, &doc, &op.selection_set, true)?)?;
        }
        incremental_selections.insert(name.clone(), selections);
        operations.push(GraphqlOperation {
            name,
            kind,
            document,
            variables,
            result: selection(schema, &doc, &op.selection_set)?,
        });
    }
    let mut reachable = std::collections::BTreeSet::new();
    for operation in &operations {
        for variable in &operation.variables {
            reachable_inputs(&variable.ty, &input_objects, &mut reachable);
        }
    }
    input_objects.retain(|name, _| reachable.contains(name));
    Ok((
        GraphqlOperations {
            schema_source: schema_source.into(),
            operation_source: source.into(),
            operations,
            input_objects,
        },
        incremental_selections,
    ))
}
fn collect_fragments(
    doc: &ExecutableDocument,
    set: &SelectionSet,
    out: &mut BTreeMap<String, String>,
) {
    for sel in &set.selections {
        match sel {
            Selection::Field(f) => collect_fragments(doc, &f.selection_set, out),
            Selection::InlineFragment(f) => collect_fragments(doc, &f.selection_set, out),
            Selection::FragmentSpread(f) => {
                let key = f.fragment_name.to_string();
                if let std::collections::btree_map::Entry::Vacant(entry) = out.entry(key) {
                    let frag = &doc.fragments[&f.fragment_name];
                    entry.insert(frag.to_string());
                    collect_fragments(doc, &frag.selection_set, out);
                }
            }
        }
    }
}
fn typed(schema: &Schema, ty: &Type, selected: Option<ModelType>) -> ModelType {
    match ty {
        Type::List(t) | Type::NonNullList(t) => ModelType {
            nullable: matches!(ty, Type::List(_)),
            kind: ModelKind::List(Box::new(typed(schema, t, selected))),
        },
        Type::Named(n) | Type::NonNullNamed(n) => ModelType {
            nullable: matches!(ty, Type::Named(_)),
            kind: selected
                .map(|s| s.kind)
                .unwrap_or_else(|| match &schema.types[n] {
                    ExtendedType::Enum(e) => {
                        ModelKind::Enum(e.values.keys().map(ToString::to_string).collect())
                    }
                    ExtendedType::InputObject(_) => ModelKind::Named(n.to_string()),
                    _ => ModelKind::Scalar(n.to_string()),
                }),
        },
    }
}
mod selection;
use selection::*;

fn reachable_inputs(
    ty: &ModelType,
    objects: &BTreeMap<String, Vec<ModelField>>,
    found: &mut std::collections::BTreeSet<String>,
) {
    match &ty.kind {
        ModelKind::Named(name) if found.insert(name.clone()) => {
            if let Some(fields) = objects.get(name) {
                for field in fields {
                    reachable_inputs(&field.ty, objects, found);
                }
            }
        }
        ModelKind::List(inner) => reachable_inputs(inner, objects, found),
        _ => {}
    }
}

pub(crate) fn input_objects(schema: &Schema) -> BTreeMap<String, Vec<ModelField>> {
    let mut input_objects = BTreeMap::new();
    for (name, ty) in &schema.types {
        if let ExtendedType::InputObject(obj) = ty {
            input_objects.insert(
                name.to_string(),
                obj.fields
                    .values()
                    .map(|f| ModelField {
                        name: f.name.to_string(),
                        ty: typed(schema, &f.ty, None),
                        optional: !f.ty.is_non_null() || f.default_value.is_some(),
                        default_value: f.default_value.as_ref().map(ToString::to_string),
                    })
                    .collect(),
            );
        }
    }
    input_objects
}

mod incremental;
use incremental::*;
