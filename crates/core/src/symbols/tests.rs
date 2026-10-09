use super::*;
use crate::blocks::BlockId;
fn request(id: &str, name: &str) -> SymbolRequest {
    SymbolRequest {
        entity: BlockId {
            source: "orders".into(),
            local: id.into(),
        },
        target: "typescript".into(),
        module: "models".into(),
        preferred: name.into(),
    }
}
#[test]
fn registration_order_cannot_change_symbol_assignments() {
    let requests = [
        request("#/c", "Order"),
        request("#/b", "Order"),
        request("#/a", "Order_2"),
    ];
    let mut forward = SymbolRequests::new();
    let mut reverse = SymbolRequests::new();
    for r in &requests {
        forward.reserve(r.clone()).unwrap();
    }
    for r in requests.iter().rev() {
        reverse.reserve(r.clone()).unwrap();
    }
    assert_eq!(forward.resolve().unwrap(), reverse.resolve().unwrap());
    assert_eq!(
        forward
            .resolve()
            .unwrap()
            .for_request(&requests[1])
            .unwrap()
            .name,
        "Order"
    );
    assert_eq!(
        forward
            .resolve()
            .unwrap()
            .for_request(&requests[0])
            .unwrap()
            .name,
        "Order_3"
    );
    assert_eq!(
        forward
            .resolve()
            .unwrap()
            .for_request(&requests[2])
            .unwrap()
            .name,
        "Order_2"
    );
}
#[test]
fn reserve_resolve_emit_lifecycle_freezes_all_mutations() {
    let mut names = SymbolRequests::new();
    let req = request("#/a", "Order");
    let key = names.reserve(req.clone()).unwrap();
    assert!(names.symbol(&key).is_err());
    names.reserve(req.clone()).unwrap();
    names.resolve().unwrap();
    assert_eq!(names.symbol(&key).unwrap().name, "Order");
    assert!(names.reserve(req).is_err());
    assert!(
        names
            .reserve_name("typescript", "models", "Future")
            .is_err()
    );
    assert!(
        names
            .target_rules("typescript", SymbolRules::default())
            .is_err()
    );
    assert!(
        names
            .symbol(&request("#/unreserved", "Missing").key().unwrap())
            .is_err()
    );
}
#[test]
fn runtime_reservations_and_case_rules_do_not_steal_natural_suffixes() {
    let mut names = SymbolRequests::new();
    names
        .target_rules(
            "typescript",
            SymbolRules {
                case_sensitive: false,
            },
        )
        .unwrap();
    names.reserve_name("typescript", "models", "Order").unwrap();
    let a = request("#/a", "order");
    let b = request("#/b", "order_2");
    names.reserve(a.clone()).unwrap();
    names.reserve(b.clone()).unwrap();
    let resolved = names.resolve().unwrap();
    assert_eq!(resolved.for_request(&a).unwrap().name, "order_3");
    assert_eq!(resolved.for_request(&b).unwrap().name, "order_2");
}
#[test]
fn identity_conflicts_and_unsafe_modules_are_rejected() {
    let mut names = SymbolRequests::new();
    names.reserve(request("#/a", "Order")).unwrap();
    assert!(names.reserve(request("#/a", "Other")).is_err());
    for bad in ["../models", "/models", "C:\\models", ""] {
        let mut r = request("#/unsafe", "Other");
        r.module = bad.into();
        assert!(names.reserve(r).is_err());
    }
    let mut alias = request("#/b", "Other");
    alias.module = "Models".into();
    names.reserve(alias).unwrap();
    assert!(names.resolve().is_err());
    assert!(!names.is_resolved());
}
#[test]
fn canonical_modules_and_distinct_sources_preserve_identity() {
    let mut names = SymbolRequests::new();
    let mut a = request("#/a", "Order");
    a.module = "./models\\shared".into();
    let mut b = a.clone();
    b.module = "models/shared".into();
    assert_eq!(
        names.reserve(a.clone()).unwrap(),
        names.reserve(b.clone()).unwrap()
    );
    b.entity.source = "other-orders".into();
    names.reserve(b.clone()).unwrap();
    let resolved = names.resolve().unwrap();
    assert_eq!(resolved.for_request(&a).unwrap().name, "Order");
    assert_eq!(resolved.for_request(&b).unwrap().name, "Order_2");
}
