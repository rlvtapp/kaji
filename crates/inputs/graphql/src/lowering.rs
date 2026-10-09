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
    let doc = ExecutableDocument::parse_and_validate(schema, source, "operations.graphql")
        .map_err(|e| anyhow!("invalid GraphQL operations: {e}"))?;
    let mut input_objects = input_objects(schema);
    let mut operations = Vec::new();
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
    Ok(GraphqlOperations {
        schema_source: schema_source.into(),
        operation_source: source.into(),
        operations,
        input_objects,
    })
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
fn selection(schema: &Schema, doc: &ExecutableDocument, set: &SelectionSet) -> Result<ModelType> {
    let mut possible: Vec<String> = match &schema.types[&set.ty] {
        ExtendedType::Union(u) => u.members.iter().map(|n| n.to_string()).collect(),
        ExtendedType::Interface(_) => schema
            .implementers_map()
            .get(&set.ty)
            .map(|i| i.objects.iter().map(ToString::to_string).collect())
            .unwrap_or_default(),
        _ => vec![set.ty.to_string()],
    };
    possible.sort();
    let mut variants = Vec::new();
    for concrete in possible {
        let mut fields = BTreeMap::new();
        fields_for(schema, doc, set, &concrete, false, &mut fields)?;
        variants.push(ModelType {
            nullable: false,
            kind: ModelKind::Object(fields.into_values().collect()),
        });
    }
    Ok(if variants.len() == 1 {
        variants.remove(0)
    } else {
        ModelType {
            nullable: false,
            kind: ModelKind::Union(variants),
        }
    })
}
/// Returns None for statically excluded selections, otherwise conditional presence.
fn condition(directives: &DirectiveList) -> Result<Option<bool>> {
    let mut optional = false;
    for d in &directives.0 {
        ensure!(
            matches!(d.name.as_str(), "skip" | "include"),
            "unsupported executable directive @{}",
            d.name
        );
        let value = &d
            .arguments
            .iter()
            .find(|a| a.name == "if")
            .ok_or_else(|| anyhow!("directive missing if argument"))?
            .value;
        match &**value {
            Value::Boolean(b) if (*b && d.name == "skip") || (!*b && d.name == "include") => {
                return Ok(None);
            }
            Value::Variable(_) => optional = true,
            _ => {}
        }
    }
    Ok(Some(optional))
}
fn applies(schema: &Schema, condition: &str, concrete: &str) -> bool {
    condition == concrete || schema.is_subtype(condition, concrete)
}
fn fields_for(
    schema: &Schema,
    doc: &ExecutableDocument,
    set: &SelectionSet,
    concrete: &str,
    inherited: bool,
    fields: &mut BTreeMap<String, ModelField>,
) -> Result<()> {
    for sel in &set.selections {
        match sel {
            Selection::Field(f) => {
                let Some(conditional) = condition(&f.directives)? else {
                    continue;
                };
                let name = f.alias.as_ref().unwrap_or(&f.name).to_string();
                let selected = if f.name == "__typename" {
                    Some(ModelType {
                        nullable: false,
                        kind: ModelKind::Literal(concrete.into()),
                    })
                } else if f.selection_set.selections.is_empty() {
                    None
                } else {
                    Some(selection(schema, doc, &f.selection_set)?)
                };
                let mut field = ModelField {
                    name: name.clone(),
                    ty: typed(schema, &f.definition.ty, selected),
                    optional: inherited || conditional,
                    default_value: None,
                };
                if let Some(old) = fields.get_mut(&name) {
                    // A conditional occurrence may add properties absent from the other occurrence.
                    if old.optional {
                        conditional_properties(&mut old.ty);
                    }
                    if field.optional {
                        conditional_properties(&mut field.ty);
                    }
                    merge_type(&mut old.ty, &field.ty);
                    old.optional &= field.optional;
                } else {
                    fields.insert(name, field);
                }
            }
            Selection::InlineFragment(f) => {
                let Some(c) = condition(&f.directives)? else {
                    continue;
                };
                if f.type_condition
                    .as_ref()
                    .is_none_or(|t| applies(schema, t.as_str(), concrete))
                {
                    fields_for(
                        schema,
                        doc,
                        &f.selection_set,
                        concrete,
                        inherited || c,
                        fields,
                    )?;
                }
            }
            Selection::FragmentSpread(f) => {
                let Some(c) = condition(&f.directives)? else {
                    continue;
                };
                let frag = &doc.fragments[&f.fragment_name];
                ensure!(
                    frag.directives.0.is_empty(),
                    "fragment definition directives are unsupported"
                );
                if applies(schema, frag.selection_set.ty.as_str(), concrete) {
                    fields_for(
                        schema,
                        doc,
                        &frag.selection_set,
                        concrete,
                        inherited || c,
                        fields,
                    )?;
                }
            }
        }
    }
    Ok(())
}
fn merge_type(old: &mut ModelType, new: &ModelType) {
    match (&mut old.kind, &new.kind) {
        (ModelKind::Object(a), ModelKind::Object(b)) => {
            for f in b {
                if let Some(o) = a.iter_mut().find(|o| o.name == f.name) {
                    let mut incoming = f.ty.clone();
                    if o.optional {
                        conditional_properties(&mut o.ty);
                    }
                    if f.optional {
                        conditional_properties(&mut incoming);
                    }
                    merge_type(&mut o.ty, &incoming);
                    o.optional &= f.optional;
                } else {
                    a.push(f.clone());
                }
            }
        }
        (ModelKind::List(a), ModelKind::List(b)) => merge_type(a, b),
        (ModelKind::Union(a), ModelKind::Union(b)) => {
            for (x, y) in a.iter_mut().zip(b) {
                merge_type(x, y)
            }
        }
        _ => {}
    }
}

fn conditional_properties(ty: &mut ModelType) {
    match &mut ty.kind {
        ModelKind::Object(fields) => {
            for field in fields {
                field.optional = true;
            }
        }
        ModelKind::List(inner) => conditional_properties(inner),
        ModelKind::Union(variants) => {
            for variant in variants {
                conditional_properties(variant);
            }
        }
        _ => {}
    }
}
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
