use poolster_core::native::*;
use poolster_input_graphql::{lower_operations, parse};
fn model(s: &str, o: &str) -> GraphqlOperations {
    let d = parse(s).unwrap();
    lower_operations(&d.schema, s, o).unwrap()
}
fn fields(t: &ModelType) -> &Vec<ModelField> {
    let ModelKind::Object(f) = &t.kind else {
        panic!("expected object: {t:?}")
    };
    f
}
const SCHEMA: &str = r#"
interface Node { id: ID! }
type User implements Node { id: ID!, name: String!, email: String }
type Team implements Node { id: ID!, title: String! }
input Filter { term: String!, parent: Filter, ids: [ID!]! = [] }
enum State { ACTIVE CLOSED }
type Query { node: Node, users(filter: Filter, limit: Int): [[User!]!]!, state: State! }
type Mutation { rename(name: String!): User! }
type Subscription { user: User! }
"#;
#[test]
fn aliases_fragments_abstract_results_nullability_presence_and_defaults() {
    let m = model(
        SCHEMA,
        r#"query Read($show:Boolean!, $filter:Filter, $limit:Int! = 2) {
   people:users(filter:$filter,limit:$limit) { ...Details email @include(if:$show) }
   node { __typename id ... on User { name } ... on Team { title } }
   state
 } fragment Details on User { name }
 "#,
    );
    let op = &m.operations[0];
    assert_eq!(op.name, "Read");
    assert_eq!(op.kind, GraphqlOperationKind::Query);
    assert!(!op.variables[0].optional);
    assert!(op.variables[1].optional);
    assert!(op.variables[2].optional);
    assert_eq!(op.variables[2].default_value.as_deref(), Some("2"));
    let f = fields(&op.result);
    let people = f.iter().find(|f| f.name == "people").unwrap();
    assert!(!people.ty.nullable);
    let ModelKind::List(l) = &people.ty.kind else {
        panic!()
    };
    assert!(!l.nullable);
    let ModelKind::List(item) = &l.kind else {
        panic!()
    };
    assert!(!item.nullable);
    let selected = fields(item);
    assert_eq!(selected.len(), 2);
    let email = selected.iter().find(|f| f.name == "email").unwrap();
    assert!(email.optional);
    assert!(email.ty.nullable);
    let node = &f.iter().find(|f| f.name == "node").unwrap().ty;
    assert!(node.nullable);
    let ModelKind::Union(variants) = &node.kind else {
        panic!()
    };
    assert_eq!(variants.len(), 2);
    assert!(
        fields(&variants[0])
            .iter()
            .any(|f| f.ty.kind == ModelKind::Literal("Team".into()))
    );
    assert!(!op.document.contains("email id"));
    assert!(op.document.contains("fragment Details"));
    assert_eq!(m.schema_source, SCHEMA);
    assert!(m.input_objects["Filter"][2].optional);
    assert!(matches!(
        m.input_objects["Filter"][1].ty.kind,
        ModelKind::Named(_)
    ));
}
#[test]
fn selections_merge_without_losing_fields_and_static_directives_remove_fields() {
    let m = model(
        SCHEMA,
        "query Read { users { name } users { email } state @skip(if:true) node { id @include(if:false) __typename } }",
    );
    let f = fields(&m.operations[0].result);
    assert_eq!(f.len(), 2);
    let users = &f.iter().find(|f| f.name == "users").unwrap().ty;
    let ModelKind::List(a) = &users.kind else {
        panic!()
    };
    let ModelKind::List(b) = &a.kind else {
        panic!()
    };
    assert_eq!(fields(b).len(), 2);
}
#[test]
fn malformed_unknown_fields_invalid_variables_cycles_and_unsupported_features_fail() {
    let d = parse(SCHEMA).unwrap();
    for op in [
        "query {",
        "query X { missing }",
        "query X($x:Int!){ users(filter:$x){name} }",
        "query X { users { ...A } } fragment A on User { ...A }",
        "query X { node }",
        "query X { users { name @defer } }",
    ] {
        assert!(
            lower_operations(&d.schema, SCHEMA, op).is_err(),
            "accepted: {op}"
        );
    }
    let s = "directive @custom on FIELD type Query { hello:String }";
    let d = parse(s).unwrap();
    assert!(lower_operations(&d.schema, s, "{ hello @custom }").is_err());
}
#[test]
fn independent_documents_and_subscriptions_retain_protocol_capability() {
    let m = model(
        SCHEMA,
        "query A { state } mutation B($name:String!) { rename(name:$name){name} } subscription C { user { id } }",
    );
    assert_eq!(m.operations.len(), 3);
    assert!(!m.operations[0].document.contains("mutation B"));
    assert_eq!(m.operations[2].kind, GraphqlOperationKind::Subscription);
}
#[test]
fn provider_options_publish_owned_contract_and_support_replacement() {
    use poolster_core::input::*;
    use poolster_input_graphql::GraphqlInput;
    let dir = tempfile::tempdir().unwrap();
    let schema = dir.path().join("schema.graphql");
    let ops = dir.path().join("ops.graphql");
    std::fs::write(&schema, SCHEMA).unwrap();
    std::fs::write(&ops, "query Read { state }").unwrap();
    let options = InputOptions {
        operation_files: vec![ops],
        ..Default::default()
    };
    let mut registry = InputRegistry::new();
    registry.register(GraphqlInput).unwrap();
    let loaded = registry
        .load_with_options("graphql", Some("graphql.apollo"), &schema, &options)
        .unwrap();
    assert_eq!(
        loaded
            .contract
            .get::<GraphqlOperations>()
            .unwrap()
            .operations
            .len(),
        1
    );
    struct Replacement;
    impl InputPlugin for Replacement {
        fn id(&self) -> &str {
            "graphql.replacement"
        }
        fn format(&self) -> &str {
            "graphql"
        }
        fn load(&self, p: &std::path::Path) -> anyhow::Result<InputContract> {
            GraphqlInput.load(p)
        }
        fn load_with_options(
            &self,
            p: &std::path::Path,
            o: &InputOptions,
        ) -> anyhow::Result<InputContract> {
            GraphqlInput.load_with_options(p, o)
        }
    }
    registry.register(Replacement).unwrap();
    assert!(
        registry
            .load_with_options("graphql", None, &schema, &options)
            .is_err()
    );
    assert!(
        registry
            .load_with_options("graphql", Some("graphql.replacement"), &schema, &options)
            .unwrap()
            .contract
            .get::<GraphqlOperations>()
            .is_ok()
    );
    let unsupported = InputOptions {
        import_roots: vec![dir.path().into()],
        ..options
    };
    assert!(
        registry
            .load_with_options("graphql", Some("graphql.apollo"), &schema, &unsupported)
            .is_err()
    );
}

#[test]
fn pinned_github_schema_lowers_real_operation_deterministically() {
    let schema = include_str!("fixtures/github/schema.graphql");
    let operations =
        "query Viewer { viewer { login repositories(first: 2) { nodes { nameWithOwner } } } }";
    let first = model(schema, operations);
    let second = model(schema, operations);
    assert_eq!(first, second);
    assert_eq!(first.operations[0].name, "Viewer");
    assert_eq!(fields(&first.operations[0].result)[0].name, "viewer");
}

#[test]
fn anonymous_operations_receive_matching_wire_name_without_mutating_source() {
    let m = model(SCHEMA, "{ state }");
    assert_eq!(m.operations[0].name, "Anonymous");
    assert!(m.operations[0].document.starts_with("query Anonymous"));
    assert_eq!(m.operation_source, "{ state }");
}

#[test]
fn conditional_duplicate_object_selection_does_not_require_conditionally_added_properties() {
    for operation in [
        "query Read($show:Boolean!) { users { name } users @include(if:$show) { email } }",
        "query Read($show:Boolean!) { users @include(if:$show) { email } users { name } }",
    ] {
        let m = model(SCHEMA, operation);
        let selected = &fields(&m.operations[0].result)[0];
        assert!(!selected.optional);
        let ModelKind::List(first) = &selected.ty.kind else {
            panic!()
        };
        let ModelKind::List(item) = &first.kind else {
            panic!()
        };
        assert!(
            fields(item)
                .iter()
                .find(|f| f.name == "email")
                .unwrap()
                .optional
        );
        assert!(
            !fields(item)
                .iter()
                .find(|f| f.name == "name")
                .unwrap()
                .optional
        );
    }
}
#[test]
fn only_transitively_reachable_input_objects_are_published() {
    let m = model(SCHEMA, "query Read { state }");
    assert!(m.input_objects.is_empty());
    let m = model(
        SCHEMA,
        "query Read($filter:Filter) { users(filter:$filter){name} }",
    );
    assert_eq!(m.input_objects.len(), 1);
    assert!(m.input_objects.contains_key("Filter"));
}

#[test]
fn nested_conditional_duplicates_propagate_presence_through_merged_objects() {
    let schema = "type Query { user:User! } type User { address:Address! } type Address { email:String! zip:String! }";
    let m = model(
        schema,
        "query Read($show:Boolean!) { user @include(if:$show) { address { email } } user { address { zip } } }",
    );
    let user = &fields(&m.operations[0].result)[0];
    let address = &fields(&user.ty)[0];
    assert!(!address.optional);
    assert!(
        fields(&address.ty)
            .iter()
            .find(|f| f.name == "email")
            .unwrap()
            .optional
    );
    assert!(
        !fields(&address.ty)
            .iter()
            .find(|f| f.name == "zip")
            .unwrap()
            .optional
    );
}
