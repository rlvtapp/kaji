use apollo_compiler::schema::ExtendedType;
use poolster_core::input::InputPlugin;
use poolster_input_graphql::{GraphqlDocument, GraphqlInput, parse};

#[test]
fn explicit_schema_does_not_infer_unused_mutation_root() {
    let schema =
        parse("schema { query: Read } type Read { read: String } type Mutation { write: String }")
            .unwrap();
    assert_eq!(schema.summary().operations.len(), 1);
    assert!(schema.schema.schema_definition.mutation.is_none());
}

#[test]
fn implicit_schema_infers_all_three_conventional_roots() {
    let schema = parse("type Query { read: String } type Mutation { write: String } type Subscription { changes: String }").unwrap();
    let summary = schema.summary();
    assert_eq!(
        summary
            .operations
            .iter()
            .map(|op| op.kind.as_str())
            .collect::<Vec<_>>(),
        vec!["query", "mutation", "subscription"]
    );
}

#[test]
fn input_defaults_descriptions_and_deprecations_survive() {
    let schema = parse(
        r#"
        enum Order { ASC DESC }
        input Filter { limit: Int = 10 order: Order = ASC tags: [String!] = ["a", "b"] }
        type Query {
            "Find inventory items"
            items(filter: Filter = { limit: 5 }): [String!]!
            legacy: String @deprecated(reason: "Use items")
        }
    "#,
    )
    .unwrap();
    let ExtendedType::InputObject(filter) = &schema.schema.types["Filter"] else {
        panic!()
    };
    assert_eq!(
        filter.fields["limit"]
            .default_value
            .as_ref()
            .unwrap()
            .to_string(),
        "10"
    );
    assert_eq!(
        filter.fields["order"]
            .default_value
            .as_ref()
            .unwrap()
            .to_string(),
        "ASC"
    );
    let ExtendedType::Object(query) = &schema.schema.types["Query"] else {
        panic!()
    };
    assert_eq!(
        query.fields["items"].description.as_ref().unwrap().as_ref(),
        "Find inventory items"
    );
    assert_eq!(query.fields["legacy"].directives.0.len(), 1);
    assert!(query.fields["items"].arguments[0].default_value.is_some());
}

#[test]
fn recursive_objects_and_nullable_recursive_inputs_are_valid() {
    let schema = parse("input Filter { child: Filter children: [Filter!] } type Node { next: Node } type Query { node(filter: Filter): Node }").unwrap();
    assert!(schema.summary().types.contains(&"Filter".into()));
    assert!(schema.summary().types.contains(&"Node".into()));
}

#[test]
fn interface_inheritance_and_covariant_return_types_are_retained() {
    let schema = parse(
        r#"
        interface Node { id: ID! }
        interface Named implements Node { id: ID! name: String }
        type Item implements Node & Named { id: ID! name: String! }
        type Query { node: Node item: Item }
    "#,
    )
    .unwrap();
    let ExtendedType::Object(item) = &schema.schema.types["Item"] else {
        panic!()
    };
    assert_eq!(item.implements_interfaces.len(), 2);
}

#[test]
fn extensions_retain_input_enum_union_interface_and_scalar_metadata() {
    let schema = parse(
        r#"
        directive @tag on SCALAR
        scalar Date
        extend scalar Date @tag
        input Filter { before: Date }
        extend input Filter { after: Date }
        enum State { ACTIVE }
        extend enum State { ARCHIVED }
        interface Node { id: ID! }
        extend interface Node { state: State }
        type Item implements Node { id: ID! state: State }
        type Other { id: ID }
        union Result = Item
        extend union Result = Other
        type Query { result(filter: Filter): Result }
    "#,
    )
    .unwrap();
    let ExtendedType::InputObject(filter) = &schema.schema.types["Filter"] else {
        panic!()
    };
    assert_eq!(filter.fields.len(), 2);
    let ExtendedType::Enum(state) = &schema.schema.types["State"] else {
        panic!()
    };
    assert_eq!(state.values.len(), 2);
    let ExtendedType::Union(result) = &schema.schema.types["Result"] else {
        panic!()
    };
    assert_eq!(result.members.len(), 2);
    let ExtendedType::Scalar(date) = &schema.schema.types["Date"] else {
        panic!()
    };
    assert_eq!(date.directives.0.len(), 1);
}

#[test]
fn repeatable_directives_retain_each_application() {
    let schema = parse(
        r#"
        directive @tag(value: String!) repeatable on OBJECT
        type Query @tag(value: "one") @tag(value: "two") { hello: String }
    "#,
    )
    .unwrap();
    let ExtendedType::Object(query) = &schema.schema.types["Query"] else {
        panic!()
    };
    assert_eq!(query.directives.0.len(), 2);
}

#[test]
fn schema_source_and_unicode_description_survive() {
    let schema =
        parse("\"商品 API\" schema { query: Query } type Query { hello: String }").unwrap();
    assert_eq!(schema.summary().title, "商品 API");
    assert!(!schema.schema.sources.is_empty());
}

#[test]
fn summary_is_repeatable_and_excludes_nested_fields_from_operations() {
    let schema = parse("type Item { id: ID! name: String } type Query { item: Item }").unwrap();
    assert_eq!(schema.summary(), schema.clone().summary());
    assert_eq!(schema.summary().operations.len(), 1);
    assert_eq!(schema.summary().operations[0].name, "item");
    assert_eq!(schema.summary().format, "graphql");
    assert_eq!(schema.summary().version, None);
}

macro_rules! invalid_schema {
    ($name:ident, $source:expr) => {
        #[test]
        fn $name() {
            let error = parse($source).unwrap_err().to_string();
            assert!(error.contains("invalid GraphQL schema"), "{error}");
            assert!(
                error.contains("schema.graphql"),
                "diagnostic loses source location: {error}"
            );
        }
    };
}
invalid_schema!(
    rejects_unknown_directive,
    "type Query @unknown { hello: String }"
);
invalid_schema!(
    rejects_wrong_directive_location,
    "directive @only on FIELD_DEFINITION type Query @only { hello: String }"
);
invalid_schema!(
    rejects_missing_directive_argument,
    "directive @tag(value: String!) on OBJECT type Query @tag { hello: String }"
);
invalid_schema!(
    rejects_nonrepeatable_directive_duplicates,
    "directive @tag on OBJECT type Query @tag @tag { hello: String }"
);
invalid_schema!(
    rejects_unknown_argument_type,
    "type Query { hello(arg: Unknown): String }"
);
invalid_schema!(
    rejects_invalid_default,
    "type Query { hello(count: Int = \"bad\"): String }"
);
invalid_schema!(
    rejects_interface_field_omission,
    "interface Node { id: ID! } type Item implements Node { name: String } type Query { item: Item }"
);
invalid_schema!(
    rejects_interface_return_type_mismatch,
    "interface Node { id: ID! } type Item implements Node { id: Int! } type Query { item: Item }"
);
invalid_schema!(
    rejects_input_nonnullable_cycle,
    "input Filter { next: Filter! } type Query { hello(filter: Filter): String }"
);
invalid_schema!(
    rejects_union_scalar_member,
    "union Result = String type Query { result: Result }"
);
invalid_schema!(
    rejects_union_duplicate_member,
    "type Item { id: ID } union Result = Item | Item type Query { result: Result }"
);
invalid_schema!(
    rejects_duplicate_enum_value,
    "enum State { ACTIVE ACTIVE } type Query { state: State }"
);
invalid_schema!(
    rejects_missing_extension_base,
    "extend type Unknown { hello: String } type Query { hello: String }"
);
invalid_schema!(
    rejects_extension_kind_mismatch,
    "scalar Date extend type Date { hello: String } type Query { date: Date }"
);
invalid_schema!(
    rejects_reserved_type_names,
    "type __Private { id: ID } type Query { private: __Private }"
);
invalid_schema!(
    rejects_reserved_field_names,
    "type Query { __private: String }"
);
invalid_schema!(
    rejects_duplicate_schema_roots,
    "schema { query: Query query: Other } type Query { hello: String } type Other { hi: String }"
);
invalid_schema!(
    rejects_root_reuse,
    "schema { query: Query mutation: Query } type Query { hello: String }"
);
invalid_schema!(rejects_empty_document, "");

#[test]
fn provider_load_publishes_native_validated_contract() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("inventory.graphql");
    std::fs::write(&path, "type Query { inventory: String }").unwrap();
    let input = GraphqlInput.load(&path).unwrap();
    assert_eq!(input.summary.operations[0].name, "inventory");
    assert_eq!(
        input.get::<GraphqlDocument>().unwrap().summary(),
        input.summary
    );
    assert!(input.diagnostics.is_empty());
    assert_eq!(GraphqlInput.id(), "graphql.apollo");
    assert_eq!(GraphqlInput.format(), "graphql");
}

#[test]
fn provider_load_reports_missing_file_with_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.graphql");
    let error = GraphqlInput.load(&path).err().unwrap().to_string();
    assert!(error.contains("cannot read contract"));
    assert!(error.contains("missing.graphql"));
}

#[test]
fn provider_load_rejects_directories_and_non_utf8() {
    let dir = tempfile::tempdir().unwrap();
    assert!(GraphqlInput.load(dir.path()).is_err());
    let path = dir.path().join("invalid.graphql");
    std::fs::write(&path, [0xff, 0xfe]).unwrap();
    assert!(GraphqlInput.load(&path).is_err());
}

#[test]
fn provider_load_rejects_invalid_schema_instead_of_publishing_summary() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("invalid.graphql");
    std::fs::write(&path, "type Query { missing: Unknown }").unwrap();
    let error = GraphqlInput.load(&path).err().unwrap().to_string();
    assert!(error.contains("invalid GraphQL schema"));
    assert!(error.contains("Unknown"));
}

invalid_schema!(
    rejects_integer_overflow_default,
    "type Query { hello(count: Int = 2147483648): String }"
);
invalid_schema!(
    rejects_nonnullable_null_default,
    "type Query { hello(count: Int! = null): String }"
);
invalid_schema!(
    rejects_invalid_list_element_default,
    "type Query { hello(counts: [Int!] = [1, null]): String }"
);
invalid_schema!(
    rejects_invalid_enum_default,
    "enum State { ACTIVE } type Query { hello(state: State = UNKNOWN): String }"
);
invalid_schema!(
    rejects_quoted_enum_default,
    "enum State { ACTIVE } type Query { hello(state: State = \"ACTIVE\"): String }"
);
invalid_schema!(
    rejects_unknown_input_default_field,
    "input Filter { count: Int } type Query { hello(filter: Filter = { unknown: 1 }): String }"
);
invalid_schema!(
    rejects_missing_required_default_field,
    "input Filter { count: Int! } type Query { hello(filter: Filter = {}): String }"
);
invalid_schema!(
    rejects_duplicate_default_field,
    "input Filter { count: Int } type Query { hello(filter: Filter = { count: 1 count: 2 }): String }"
);
invalid_schema!(
    rejects_invalid_input_field_default,
    "input Filter { count: Int = \"bad\" } type Query { hello(filter: Filter): String }"
);
invalid_schema!(
    rejects_invalid_directive_definition_default,
    "directive @tag(count: Int = \"bad\") on OBJECT type Query { hello: String }"
);
invalid_schema!(
    rejects_invalid_interface_argument_default,
    "interface Node { name(count: Int = \"bad\"): String } type Item implements Node { name(count: Int): String } type Query { item: Item }"
);

#[test]
fn valid_default_coercions_include_singleton_lists_ids_and_custom_scalars() {
    parse(
        r#"
        scalar JSON
        input Filter { required: Int! = 10 }
        type Query {
            hello(counts: [Int!] = 1, id: ID = 12345678901234567890,
                ratio: Float = 1, optional: String = null, filter: Filter = {},
                custom: JSON = { nested: [true, 1, "text"] }): String
        }
    "#,
    )
    .unwrap();
}

invalid_schema!(
    rejects_integer_underflow_default,
    "type Query { hello(count: Int = -2147483649): String }"
);
invalid_schema!(
    rejects_nonfinite_float_default,
    "type Query { hello(ratio: Float = 1e999): String }"
);

#[test]
fn nested_singleton_lists_and_int_bounds_coerce() {
    parse("type Query { hello(nested: [[Int!]!]! = 1, min: Int = -2147483648, max: Int = 2147483647): String }").unwrap();
}
