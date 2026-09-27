use std::collections::BTreeMap;

use kaji_core::{
    HttpMethod, Operation, OperationFilter, OperationSelection, OverrideFilter, OverrideRule,
    OverrideRules, Schema, SchemaKind, SchemaValue, wildcard_matches,
};

#[test]
fn selection_respects_exclusions_and_explicit_tags() {
    let operation = Operation {
        id: "listContacts".into(),
        path: "/contacts".into(),
        method: HttpMethod::Get,
        ..Operation::default()
    };
    let selection = OperationSelection {
        include: vec![OperationFilter::Tag("public*".into())],
        exclude: vec![OperationFilter::Method(HttpMethod::Delete)],
    };
    assert!(selection.includes_with_tags(&operation, &["public-api".into()]));
    assert!(!selection.includes(&operation));
    let excluded = Operation {
        method: HttpMethod::Delete,
        ..operation
    };
    assert!(!selection.includes_with_tags(&excluded, &["public-api".into()]));
}

#[test]
fn plugin_owned_override_maps_resolve_in_declaration_order() {
    let operation = Operation {
        id: "listContacts".into(),
        path: "/contacts".into(),
        method: HttpMethod::Get,
        ..Operation::default()
    };
    let rules = OverrideRules {
        rules: vec![
            OverrideRule {
                filter: OverrideFilter::Path("/contacts*".into()),
                values: BTreeMap::from([
                    ("name".into(), "all".into()),
                    ("tag".into(), "retained".into()),
                ]),
            },
            OverrideRule {
                filter: OverrideFilter::OperationId("list*".into()),
                values: BTreeMap::from([("name".into(), "specific".into())]),
            },
            OverrideRule {
                filter: OverrideFilter::SchemaName("Contact*".into()),
                values: BTreeMap::from([("name".into(), "model".into())]),
            },
        ],
    };
    let base = BTreeMap::from([
        ("name".into(), "default".into()),
        ("keep".into(), "yes".into()),
    ]);
    let resolved = rules.resolve_operation(&operation, &[], &base);
    assert_eq!(resolved["name"], "specific");
    assert_eq!(resolved["tag"], "retained");
    assert_eq!(resolved["keep"], "yes");
    let schema = Schema::new("Contact", SchemaValue::new(SchemaKind::String));
    assert_eq!(rules.resolve_schema(&schema, &base)["name"], "model");
    assert_eq!(base["name"], "default");
}

#[test]
fn wildcards_match_paths_and_unicode_scalars() {
    assert!(wildcard_matches("/contacts/*", "/contacts/123/events"));
    assert!(wildcard_matches("?", "é"));
    assert!(!wildcard_matches("?", "ab"));
    assert!(!wildcard_matches("list*", "getContact"));
}
