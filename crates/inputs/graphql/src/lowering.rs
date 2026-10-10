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
fn selection(schema: &Schema, doc: &ExecutableDocument, set: &SelectionSet) -> Result<ModelType> {
    selection_mode(schema, doc, set, false)
}
fn selection_mode(
    schema: &Schema,
    doc: &ExecutableDocument,
    set: &SelectionSet,
    initial: bool,
) -> Result<ModelType> {
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
        fields_for(schema, doc, set, &concrete, (false, initial), &mut fields)?;
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
fn condition(directives: &DirectiveList, initial: bool) -> Result<Option<bool>> {
    let mut optional = false;
    for d in &directives.0 {
        if matches!(d.name.as_str(), "defer" | "stream") {
            if initial
                && d.name == "defer"
                && !d
                    .arguments
                    .iter()
                    .any(|arg| arg.name == "if" && matches!(&*arg.value, Value::Boolean(false)))
            {
                optional = true;
            }
            continue;
        }
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
    presence: (bool, bool),
    fields: &mut BTreeMap<String, ModelField>,
) -> Result<()> {
    let (inherited, initial) = presence;
    for sel in &set.selections {
        match sel {
            Selection::Field(f) => {
                let Some(conditional) = condition(&f.directives, initial)? else {
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
                    Some(selection_mode(schema, doc, &f.selection_set, initial)?)
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
                let Some(c) = condition(&f.directives, initial)? else {
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
                        (inherited || c, initial),
                        fields,
                    )?;
                }
            }
            Selection::FragmentSpread(f) => {
                let Some(c) = condition(&f.directives, initial)? else {
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
                        (inherited || c, initial),
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

fn incremental_metadata(
    doc: &ExecutableDocument,
    set: &SelectionSet,
    path: &[String],
    enabled: bool,
    out: &mut Vec<GraphqlIncrementalSelection>,
) -> Result<()> {
    for selection in &set.selections {
        let (directives, nested, mut next, is_field) = match selection {
            Selection::Field(f) => {
                let mut p = path.to_vec();
                p.push(f.alias.as_ref().unwrap_or(&f.name).to_string());
                (&f.directives, &f.selection_set, p, true)
            }
            Selection::InlineFragment(f) => (&f.directives, &f.selection_set, path.to_vec(), false),
            Selection::FragmentSpread(f) => (
                &f.directives,
                &doc.fragments[&f.fragment_name].selection_set,
                path.to_vec(),
                false,
            ),
        };
        for d in &directives.0 {
            if !matches!(d.name.as_str(), "defer" | "stream") {
                continue;
            }
            ensure!(
                enabled,
                "incremental @{} requires explicit incremental input and transport",
                d.name
            );
            ensure!(
                (d.name == "stream") == is_field,
                "@defer belongs on fragments and @stream belongs on list fields"
            );
            if let Selection::Field(f) = selection {
                ensure!(
                    matches!(f.definition.ty, Type::List(_) | Type::NonNullList(_)),
                    "@stream requires a list field"
                );
            }
            let argument = |name: &str| {
                d.arguments
                    .iter()
                    .find(|a| a.name.as_str() == name)
                    .map(|a| &*a.value)
            };
            let condition = match argument("if") {
                Some(Value::Variable(v)) => GraphqlIncrementalCondition::Variable(v.to_string()),
                Some(Value::Boolean(false)) => GraphqlIncrementalCondition::Never,
                None | Some(Value::Boolean(true)) => GraphqlIncrementalCondition::Always,
                _ => return Err(anyhow!("invalid incremental condition")),
            };
            let label = match argument("label") {
                Some(Value::String(v)) => Some(v.clone()),
                None => None,
                _ => return Err(anyhow!("incremental labels must be literal strings")),
            };
            let initial_count = argument("initialCount").map(ToString::to_string);
            ensure!(
                initial_count
                    .as_ref()
                    .is_none_or(|count| !count.starts_with('-')),
                "@stream initialCount must be nonnegative"
            );
            out.push(GraphqlIncrementalSelection {
                kind: if d.name == "defer" {
                    GraphqlIncrementalKind::Defer
                } else {
                    GraphqlIncrementalKind::Stream
                },
                path: next.clone(),
                label,
                condition,
                initial_count,
            });
        }
        if let Selection::Field(f) = selection {
            if matches!(f.definition.ty, Type::List(_) | Type::NonNullList(_)) {
                next.push("*".into());
            }
        }
        incremental_metadata(doc, nested, &next, enabled, out)?;
    }
    Ok(())
}

#[cfg(test)]
mod incremental_tests {
    use super::*;
    const SDL: &str = "directive @defer(if:Boolean! = true,label:String) on FRAGMENT_SPREAD | INLINE_FRAGMENT\ndirective @stream(if:Boolean! = true,label:String,initialCount:Int! = 0) on FIELD\ntype Query { users:[User!]! } type User { id:ID! name:String! }";
    #[test]
    fn incremental_is_opt_in_preserves_final_types_and_coordinates() {
        let schema = Schema::parse_and_validate(SDL, "schema.graphql").unwrap();
        let source = "query Users($later:Boolean! = true) { people:users @stream(initialCount:1,label:\"users\") { id ... @defer(if:$later,label:\"details\") { name } } }";
        assert!(lower(&schema, SDL, source).is_err());
        let out = lower_incremental(&schema, SDL, source).unwrap();
        assert_eq!(out.selections["Users"][0].path, ["people"]);
        assert_eq!(out.selections["Users"][1].path, ["people", "*"]);
        assert_eq!(
            out.selections["Users"][1].condition,
            GraphqlIncrementalCondition::Variable("later".into())
        );
        assert!(out.definition.operations[0].document.contains("@defer"));
        let ModelKind::Object(fields) = &out.definition.operations[0].result.kind else {
            panic!()
        };
        let ModelKind::List(item) = &fields[0].ty.kind else {
            panic!()
        };
        let ModelKind::Object(fields) = &item.kind else {
            panic!()
        };
        assert!(!fields.iter().find(|f| f.name == "name").unwrap().optional);
    }
    #[test]
    fn incremental_rejects_invalid_locations_and_missing_capability() {
        let schema = Schema::parse_and_validate(SDL, "schema.graphql").unwrap();
        assert!(lower_incremental(&schema, SDL, "query X { users { name @stream } }").is_err());
        assert!(lower_incremental(&schema, SDL, "query X { users { id } }").is_err());
    }
}

fn validate_initial_unions(ty: &ModelType) -> Result<()> {
    match &ty.kind {
        ModelKind::Union(variants) => {
            let mut common: Option<std::collections::BTreeSet<String>> = None;
            for variant in variants {
                let ModelKind::Object(fields) = &variant.kind else {
                    return Err(anyhow!(
                        "incremental abstract selection must expose objects"
                    ));
                };
                let names = fields
                    .iter()
                    .filter(|f| !f.optional && matches!(f.ty.kind, ModelKind::Literal(_)))
                    .map(|f| f.name.clone())
                    .collect::<std::collections::BTreeSet<_>>();
                common = Some(match common {
                    None => names,
                    Some(previous) => previous.intersection(&names).cloned().collect(),
                });
            }
            ensure!(
                common.is_some_and(|names| !names.is_empty()),
                "incremental abstract selections require an unconditional, non-deferred __typename discriminator (aliases supported)"
            );
            for variant in variants {
                validate_initial_unions(variant)?;
            }
        }
        ModelKind::Object(fields) => {
            for field in fields {
                validate_initial_unions(&field.ty)?;
            }
        }
        ModelKind::List(item) => validate_initial_unions(item)?,
        _ => {}
    };
    Ok(())
}

#[cfg(test)]
mod incremental_discriminator_tests {
    use super::*;
    const SDL: &str = "directive @defer(if:Boolean! = true) on FRAGMENT_SPREAD | INLINE_FRAGMENT interface Node{id:ID!} type A implements Node{id:ID! name:String} type B implements Node{id:ID! code:String} type Query{node:Node}";
    #[test]
    fn deferred_or_conditional_discriminator_is_explicitly_unsupported() {
        let schema = Schema::parse_and_validate(SDL, "schema.graphql").unwrap();
        let source = "query X{node{id ... @defer{kind:__typename}}}";
        assert!(
            lower_incremental(&schema, SDL, source)
                .unwrap_err()
                .to_string()
                .contains("non-deferred __typename")
        );
        assert!(lower(&schema, SDL, "query X{node{id kind:__typename}}").is_ok());
        let source = "query X($show:Boolean!){node{id kind:__typename @include(if:$show) ... @defer{... on A{name}}}}";
        assert!(lower_incremental(&schema, SDL, source).is_err());
    }
    #[test]
    fn unconditional_aliased_tag_and_duplicate_selections_are_supported() {
        let schema = Schema::parse_and_validate(SDL, "schema.graphql").unwrap();
        for source in [
            "query X{node{kind:__typename id ... @defer{... on A{name} ... on B{code}}}}",
            "query X{node{kind:__typename} node{id ... @defer{... on A{name}}}}",
            "query X{node{... @defer(if:false){kind:__typename} ... @defer{... on A{name}}}}",
        ] {
            assert!(lower_incremental(&schema, SDL, source).is_ok(), "{source}");
        }
    }
}
