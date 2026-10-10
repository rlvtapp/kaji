use super::*;
use poolster_core::native::GraphqlOperation;
fn operations(names: &[(&str, GraphqlOperationKind)]) -> GraphqlOperations {
    GraphqlOperations {
        schema_source: String::new(),
        operation_source: String::new(),
        input_objects: BTreeMap::new(),
        operations: names
            .iter()
            .map(|(name, kind)| GraphqlOperation {
                name: (*name).into(),
                kind: *kind,
                document: String::new(),
                variables: vec![],
                result: ModelType {
                    nullable: false,
                    kind: ModelKind::Object(vec![]),
                },
            })
            .collect(),
    }
}
#[test]
fn bound_styles_keep_kind_and_transport_capabilities_separate() {
    let contract = operations(&[
        ("Read", GraphqlOperationKind::Query),
        ("Rename", GraphqlOperationKind::Mutation),
        ("Events", GraphqlOperationKind::Subscription),
    ]);
    let mut grouped = String::new();
    let methods = render(
        &mut grouped,
        GraphqlStyle::Idiomatic,
        &contract,
        &BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(methods["Read"], "query.read");
    assert_eq!(methods["Rename"], "mutation.rename");
    assert_eq!(methods["Events"], "subscription.events");
    assert!(grouped.contains("subscriptionTransport: SubscriptionTransport"));
    assert!(grouped.contains("Events(subscriptionTransport!, variables, options)"));
    let mut flat = String::new();
    assert_eq!(
        render(&mut flat, GraphqlStyle::Flat, &contract, &BTreeMap::new()).unwrap()["Read"],
        "read"
    );
    let mut raw = String::new();
    assert_eq!(
        render(&mut raw, GraphqlStyle::Raw, &contract, &BTreeMap::new()).unwrap()["Read"],
        "read"
    );
    assert!(raw.is_empty());
}
#[test]
fn normalized_collisions_and_promise_like_clients_are_rejected() {
    for names in [
        vec![
            ("Read", GraphqlOperationKind::Query),
            ("read", GraphqlOperationKind::Query),
        ],
        vec![("Then", GraphqlOperationKind::Query)],
    ] {
        assert!(
            render(
                &mut String::new(),
                GraphqlStyle::Flat,
                &operations(&names),
                &BTreeMap::new()
            )
            .is_err()
        );
    }
}
#[test]
fn explicit_groups_move_only_assigned_operations_and_validate_configuration() {
    let contract = operations(&[
        ("Read", GraphqlOperationKind::Query),
        ("Rename", GraphqlOperationKind::Mutation),
    ]);
    let custom = BTreeMap::from([(
        "user".into(),
        BTreeMap::from([("read".into(), "Read".into())]),
    )]);
    let methods = render(
        &mut String::new(),
        GraphqlStyle::Idiomatic,
        &contract,
        &custom,
    )
    .unwrap();
    assert_eq!(methods["Read"], "user.read");
    assert_eq!(methods["Rename"], "mutation.rename");
    for style in [GraphqlStyle::Flat, GraphqlStyle::Raw] {
        assert!(render(&mut String::new(), style, &contract, &custom).is_err());
    }
    for custom in [
        BTreeMap::from([(
            "user".into(),
            BTreeMap::from([("read".into(), "Missing".into())]),
        )]),
        BTreeMap::from([(
            "then".into(),
            BTreeMap::from([("read".into(), "Read".into())]),
        )]),
        BTreeMap::from([(
            "user".into(),
            BTreeMap::from([("one".into(), "Read".into()), ("two".into(), "Read".into())]),
        )]),
        BTreeMap::from([(
            "user".into(),
            BTreeMap::from([("invalid-name".into(), "Read".into())]),
        )]),
    ] {
        assert!(
            render(
                &mut String::new(),
                GraphqlStyle::Idiomatic,
                &contract,
                &custom
            )
            .is_err()
        );
    }
}
