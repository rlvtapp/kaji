use poolster_core::input::{InputOptions, InputPlugin};
use poolster_input_graphql::{
    GraphqlInput,
    blocks::{InputModelBlocks, OperationBlocks},
    contracts::{GraphqlDocument, GraphqlOperations},
};
#[test]
fn public_whole_model_and_operation_contracts_are_published() {
    let dir = tempfile::tempdir().unwrap();
    let schema = dir.path().join("schema.graphql");
    let operations = dir.path().join("operations.graphql");
    std::fs::write(
        &schema,
        "input Filter { name: String = \"guest\" } type Query { hello(filter: Filter): String! }",
    )
    .unwrap();
    std::fs::write(
        &operations,
        "query Hello($filter: Filter) { hello(filter: $filter) }",
    )
    .unwrap();
    let inspected = GraphqlInput.load(&schema).unwrap();
    assert!(inspected.get::<GraphqlDocument>().is_ok());
    let models = inspected.get::<InputModelBlocks>().unwrap();
    assert_eq!(models.items.len(), 1);
    assert!(inspected.get::<GraphqlOperations>().is_err());
    assert!(matches!(
        inspected.get::<OperationBlocks>().unwrap().state,
        poolster_core::blocks::CollectionState::Unavailable { .. }
    ));
    let generated = GraphqlInput
        .load_with_options(
            &schema,
            &InputOptions {
                operation_files: vec![operations],
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        generated.get::<InputModelBlocks>().unwrap().items[0].value,
        models.items[0].value
    );
    let whole_ref = generated
        .get_reference::<GraphqlOperations>()
        .unwrap()
        .unwrap();
    generated
        .get::<OperationBlocks>()
        .unwrap()
        .require_parent(whole_ref)
        .unwrap();
    generated
        .get::<InputModelBlocks>()
        .unwrap()
        .require_parent(whole_ref)
        .unwrap();
    assert_eq!(
        generated.get::<OperationBlocks>().unwrap().items[0]
            .value
            .name,
        "Hello"
    );
    assert_eq!(
        generated
            .get::<GraphqlOperations>()
            .unwrap()
            .operations
            .len(),
        1
    );
}

#[test]
fn empty_model_collection_is_complete_while_missing_operations_are_unavailable() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), "type Query { hello: String }").unwrap();
    let loaded = GraphqlInput.load(file.path()).unwrap();
    let models = loaded.get::<InputModelBlocks>().unwrap();
    assert!(models.items.is_empty());
    models.require_complete().unwrap();
    models
        .require_parent(loaded.get_reference::<GraphqlDocument>().unwrap().unwrap())
        .unwrap();
    let operations = loaded.get::<OperationBlocks>().unwrap();
    assert!(operations.items.is_empty());
    assert!(operations.require_complete().is_err());
}
