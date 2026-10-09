use super::*;
fn contract() -> GraphqlOperations {
    GraphqlOperations {
        schema_source: String::new(),
        operation_source: String::new(),
        operations: vec![],
        input_objects: BTreeMap::new(),
    }
}
fn field(kind: ModelKind) -> ModelField {
    ModelField {
        name: "value".into(),
        ty: ModelType {
            nullable: false,
            kind,
        },
        optional: false,
        default_value: None,
    }
}
#[test]
fn runtime_names_are_reserved_for_substituted_contracts() {
    let mut contract = contract();
    for name in ["GraphqlError", "GraphqlErrorLocation", "Result", "Presence"] {
        contract.input_objects.insert(name.into(), vec![]);
    }
    let mut models = Models::new(&contract);
    models.input_objects().unwrap();
    for name in ["GraphqlError", "GraphqlErrorLocation", "Result", "Presence"] {
        assert!(models.source.contains(&format!("pub struct {name}2")));
    }
}
#[test]
fn abstract_variants_cannot_change_types_under_identical_response_keys() {
    let contract = contract();
    let mut models = Models::new(&contract);
    let selection = ModelType {
        nullable: false,
        kind: ModelKind::Union(vec![
            ModelType {
                nullable: false,
                kind: ModelKind::Object(vec![field(ModelKind::Scalar("String".into()))]),
            },
            ModelType {
                nullable: false,
                kind: ModelKind::Object(vec![field(ModelKind::Scalar("Int".into()))]),
            },
        ]),
    };
    assert!(
        models
            .ty(&selection, "Node", false, None)
            .unwrap_err()
            .to_string()
            .contains("select __typename")
    );
}
