use super::*;

pub(super) fn selection(
    schema: &Schema,
    doc: &ExecutableDocument,
    set: &SelectionSet,
) -> Result<ModelType> {
    selection_mode(schema, doc, set, false)
}
pub(super) fn selection_mode(
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
pub(super) fn condition(directives: &DirectiveList, initial: bool) -> Result<Option<bool>> {
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
pub(super) fn applies(schema: &Schema, condition: &str, concrete: &str) -> bool {
    condition == concrete || schema.is_subtype(condition, concrete)
}
pub(super) fn fields_for(
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
pub(super) fn merge_type(old: &mut ModelType, new: &ModelType) {
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

pub(super) fn conditional_properties(ty: &mut ModelType) {
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
