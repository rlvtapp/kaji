use super::*;
use poolster_core::HttpMethod;
use serde_json::json;

#[test]
fn pagination_keys_keep_legacy_support_and_prefer_poolster() {
    let mut operation = Operation::default();
    operation.annotations.insert(
        "x-kaji-pagination".into(),
        json!({"type": "url", "outputs": {"nextUrl": "$.old"}}),
    );
    assert_eq!(url_pagination(&operation).unwrap().next_url_path, "$.old");

    operation.annotations.insert(
        "x-poolster-pagination".into(),
        json!({"type": "url", "outputs": {"nextUrl": "$.new"}}),
    );
    assert_eq!(url_pagination(&operation).unwrap().next_url_path, "$.new");

    operation.annotations.remove("x-poolster-pagination");
    operation.annotations.remove("x-kaji-pagination");
    operation.annotations.insert(
        "x-speakeasy-pagination".into(),
        json!({"type": "url", "outputs": {"nextUrl": "$.external"}}),
    );
    assert_eq!(
        url_pagination(&operation).unwrap().next_url_path,
        "$.external"
    );
}

#[test]
fn open_enum_keys_keep_legacy_support_and_prefer_poolster() {
    let mut value = SchemaValue::new(SchemaKind::String);
    value.enum_values = vec![json!("known")];
    value
        .extensions
        .insert("x-kaji-open-enum".into(), json!(true));
    assert!(
        render_model(&Schema::new("State", value.clone())).contains("Literal[\"known\"] | str")
    );

    value
        .extensions
        .insert("x-poolster-open-enum".into(), json!(false));
    assert!(
        render_model(&Schema::new("State", value.clone())).contains("State = Literal[\"known\"]\n")
    );

    value
        .extensions
        .insert("x-poolster-open-enum".into(), json!(true));
    assert!(render_model(&Schema::new("State", value)).contains("Literal[\"known\"] | str"));
}

#[test]
fn idempotency_keys_keep_legacy_support_and_prefer_poolster() {
    let mut operation = Operation {
        id: "createThing".into(),
        method: HttpMethod::Post,
        path: "/things".into(),
        ..Default::default()
    };
    operation.annotations.insert(
        "x-kaji-idempotency-resolved".into(),
        json!({"header": "X-Old-Key", "auto_generate": false}),
    );
    let api = Api::default();
    assert!(render_operation(&api, &operation).contains("idempotency_header=\"X-Old-Key\""));

    operation.annotations.insert(
        "x-poolster-idempotency-resolved".into(),
        json!({"header": "X-New-Key", "auto_generate": false}),
    );
    let rendered = render_operation(&api, &operation);
    assert!(rendered.contains("idempotency_header=\"X-New-Key\""));
    assert!(!rendered.contains("idempotency_header=\"X-Old-Key\""));
}
