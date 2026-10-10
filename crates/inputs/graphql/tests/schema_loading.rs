use poolster_core::{
    input::{InputOptions, InputPlugin},
    native::GraphqlOperations,
};
use poolster_input_graphql::{GraphqlDocument, GraphqlInput};
use std::path::Path;

#[test]
fn pinned_introspection_loads_same_operation_contract_as_sdl() {
    let dir = tempfile::tempdir().unwrap();
    let query = dir.path().join("query.graphql");
    std::fs::write(
        &query,
        "query Users($filter: Filter) { users(filter:$filter) { id name created } }",
    )
    .unwrap();
    let options = InputOptions {
        operation_files: vec![query],
        ..Default::default()
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/introspection");
    let sdl = GraphqlInput
        .load_with_options(&root.join("schema.graphql"), &options)
        .unwrap();
    let json = GraphqlInput
        .load_with_options(&root.join("schema.json"), &options)
        .unwrap();
    let a = sdl.get::<GraphqlOperations>().unwrap();
    let b = json.get::<GraphqlOperations>().unwrap();
    assert_eq!(a.operations, b.operations);
    assert_eq!(a.input_objects, b.input_objects);
    assert!(b.schema_source.contains("repeatable"));
    assert!(b.schema_source.contains("@specifiedBy"));
    assert!(b.schema_source.contains("@deprecated"));
    let original = json.get::<GraphqlDocument>().unwrap();
    assert!(
        original
            .native_documents
            .values()
            .any(|v| v.contains("\"__schema\""))
    );
}
#[test]
fn schema_and_fragment_imports_resolve_roots_and_deduplicate() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir(root.join("imports")).unwrap();
    std::fs::write(
        root.join("schema.graphql"),
        "#import * from 'types.graphql'\ntype Query { user: User! }",
    )
    .unwrap();
    std::fs::write(
        root.join("imports/types.graphql"),
        "type User { id: ID! name: String! }",
    )
    .unwrap();
    std::fs::write(
        root.join("fragment.graphql"),
        "fragment Fields on User { id name }",
    )
    .unwrap();
    for (file, name) in [("one.graphql", "One"), ("two.graphql", "Two")] {
        std::fs::write(
            root.join(file),
            format!("#import \"fragment.graphql\"\nquery {name} {{ user {{ ...Fields }} }}"),
        )
        .unwrap();
    }
    let options = InputOptions {
        operation_files: vec![root.join("one.graphql"), root.join("two.graphql")],
        import_roots: vec![root.join("imports")],
        ..Default::default()
    };
    let input = GraphqlInput
        .load_with_options(&root.join("schema.graphql"), &options)
        .unwrap();
    assert_eq!(
        input.get::<GraphqlOperations>().unwrap().operations.len(),
        2
    );
    assert_eq!(
        input
            .get::<GraphqlDocument>()
            .unwrap()
            .native_documents
            .len(),
        5
    );
    std::fs::write(
        root.join("imports/types.graphql"),
        "#import '../schema.graphql'\ntype User { id: ID! }",
    )
    .unwrap();
    let error = GraphqlInput
        .load_with_options(&root.join("schema.graphql"), &options)
        .err()
        .unwrap();
    assert!(error.to_string().contains("cyclic"));
}
#[test]
fn malformed_introspection_and_imports_are_diagnostic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("schema.json");
    for value in [
        r#"{"data":{"__schema":{}}}"#,
        r#"{"errors":[{"message":"denied"}],"data":null}"#,
        r#"{"notSchema":{}}"#,
        r#"{"data":{"__schema":{"queryType":{"name":"Query"},"types":"wrong"}}}"#,
    ] {
        std::fs::write(&path, value).unwrap();
        assert!(GraphqlInput.load(&path).is_err());
    }
    std::fs::write(
        &path,
        "#import \"missing.graphql\"\ntype Query { hello: String }",
    )
    .unwrap();
    assert!(
        GraphqlInput
            .load(&path)
            .err()
            .unwrap()
            .to_string()
            .contains("not found")
    );
    std::fs::write(
        &path,
        "#import User from \"types.graphql\"\ntype Query { hello: String }",
    )
    .unwrap();
    assert!(
        GraphqlInput
            .load(&path)
            .err()
            .unwrap()
            .to_string()
            .contains("selective imports")
    );
}

#[test]
fn retained_documents_revision_tracks_operation_imports() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(
        root.join("schema.graphql"),
        "type Query{user:User!} type User{id:ID! name:String!}",
    )
    .unwrap();
    std::fs::write(
        root.join("operation.graphql"),
        "#import \"fragment.graphql\"\nquery Read{user{...Fields}}",
    )
    .unwrap();
    let options = InputOptions {
        operation_files: vec![root.join("operation.graphql")],
        ..Default::default()
    };
    std::fs::write(root.join("fragment.graphql"), "fragment Fields on User{id}").unwrap();
    let before = GraphqlInput
        .load_with_options(&root.join("schema.graphql"), &options)
        .unwrap();
    std::fs::write(
        root.join("fragment.graphql"),
        "fragment Fields on User{id name}",
    )
    .unwrap();
    let after = GraphqlInput
        .load_with_options(&root.join("schema.graphql"), &options)
        .unwrap();
    let old = before.get_reference::<GraphqlDocument>().unwrap().unwrap();
    let new = after.get_reference::<GraphqlDocument>().unwrap().unwrap();
    assert_eq!(old.instance, new.instance);
    assert_ne!(old.revision, new.revision);
    assert_ne!(
        before.get_reference::<GraphqlOperations>().unwrap(),
        after.get_reference::<GraphqlOperations>().unwrap()
    );
}
