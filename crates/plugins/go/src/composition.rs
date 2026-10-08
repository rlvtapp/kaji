//! Flatten object inheritance for named Go models without expanding property
//! references (which can be recursive). Non-object intersections stay lossless
//! raw JSON rather than silently discarding variants.
use super::*;

pub(super) fn models(api: &Api) -> Option<Vec<Schema>> {
    if !api
        .schemas
        .iter()
        .any(|s| matches!(s.value.kind, SchemaKind::AllOf { .. }))
    {
        return None;
    }
    let index: BTreeMap<_, _> = api
        .schemas
        .iter()
        .map(|schema| (schema.name.as_str(), &schema.value))
        .collect();
    let schemas = api
        .schemas
        .iter()
        .map(|schema| {
            let mut schema = schema.clone();
            if matches!(schema.value.kind, SchemaKind::AllOf { .. }) {
                if let Some(kind) = object(&schema.value, &index, &mut BTreeSet::new()) {
                    schema.value.kind = kind;
                }
            }
            schema
        })
        .collect();
    Some(schemas)
}

fn object(
    value: &SchemaValue,
    index: &BTreeMap<&str, &SchemaValue>,
    visiting: &mut BTreeSet<String>,
) -> Option<SchemaKind> {
    if visiting.len() > 64 {
        return None;
    }
    match &value.kind {
        SchemaKind::Object { .. } => Some(value.kind.clone()),
        SchemaKind::Reference { reference } => {
            let name = reference
                .strip_prefix("#/components/schemas/")?
                .replace("~1", "/")
                .replace("~0", "~");
            if !visiting.insert(name.clone()) {
                return None;
            }
            let result = index
                .get(name.as_str())
                .and_then(|value| object(value, index, visiting));
            visiting.remove(&name);
            result
        }
        SchemaKind::AllOf { variants } if !variants.is_empty() => {
            let mut fields: Vec<poolster_core::Field> = Vec::new();
            let mut additional_properties = AdditionalProperties::Unspecified;
            for variant in variants {
                let SchemaKind::Object {
                    fields: next,
                    additional_properties: additional,
                } = object(variant, index, visiting)?
                else {
                    return None;
                };
                for field in next {
                    if let Some(previous) = fields
                        .iter_mut()
                        .find(|previous| previous.name == field.name)
                    {
                        // Inheritance commonly refines a property's metadata. Do
                        // not choose arbitrarily between incompatible wire types.
                        if previous.value.kind != field.value.kind {
                            return None;
                        }
                        previous.required |= field.required;
                        previous.value.nullable &= field.value.nullable;
                    } else {
                        fields.push(field);
                    }
                }
                match (&additional_properties, &additional) {
                    (_, AdditionalProperties::Unspecified) => {}
                    (AdditionalProperties::Unspecified, _) => additional_properties = additional,
                    (left, right) if left == right => {}
                    _ => return None,
                }
            }
            Some(SchemaKind::Object {
                fields,
                additional_properties,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object_value(name: &str, required: bool) -> SchemaValue {
        SchemaValue::new(SchemaKind::Object {
            fields: vec![poolster_core::Field {
                name: name.into(),
                value: SchemaValue::new(SchemaKind::String),
                required,
                annotations: BTreeMap::new(),
            }],
            additional_properties: AdditionalProperties::Unspecified,
        })
    }

    #[test]
    fn inheritance_keeps_fields_and_required_intersection() {
        let api = Api {
            schemas: vec![
                Schema::new("Entity", object_value("id", false)),
                Schema::new(
                    "User",
                    SchemaValue::new(SchemaKind::AllOf {
                        variants: vec![
                            SchemaValue::reference("#/components/schemas/Entity"),
                            object_value("id", true),
                            object_value("displayName", false),
                        ],
                    }),
                ),
            ],
            ..Api::default()
        };
        let result = models(&api).unwrap();
        let SchemaKind::Object { fields, .. } = &result[1].value.kind else {
            panic!("expected a typed model")
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "id");
        assert!(fields[0].required);
        assert_eq!(fields[1].name, "displayName");
    }

    #[test]
    fn cyclic_or_non_object_intersections_remain_lossless() {
        let api = Api {
            schemas: vec![
                Schema::new(
                    "Cycle",
                    SchemaValue::new(SchemaKind::AllOf {
                        variants: vec![SchemaValue::reference("#/components/schemas/Cycle")],
                    }),
                ),
                Schema::new(
                    "Scalar",
                    SchemaValue::new(SchemaKind::AllOf {
                        variants: vec![SchemaValue::new(SchemaKind::String)],
                    }),
                ),
            ],
            ..Api::default()
        };
        assert_eq!(models(&api).unwrap(), api.schemas);
    }
}
