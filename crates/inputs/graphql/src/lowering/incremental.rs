use super::*;

pub(super) fn incremental_metadata(
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

pub(super) fn validate_initial_unions(ty: &ModelType) -> Result<()> {
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
