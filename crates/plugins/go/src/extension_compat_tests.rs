use super::*;
use poolster_core::HttpMethod;
use serde_json::json;

#[test]
fn idempotency_keys_generate_the_same_go_support() {
    for key in [
        "x-poolster-idempotency-resolved",
        "x-kaji-idempotency-resolved",
    ] {
        let mut operation = Operation {
            id: "write".into(),
            method: HttpMethod::Post,
            path: "/write".into(),
            ..Default::default()
        };
        operation.annotations.insert(
            key.into(),
            json!({"header": "X-Request-Key", "parameter_name": "X-Request-Key", "auto_generate": true}),
        );
        let api = Api {
            name: "Keys".into(),
            operations: vec![operation],
            ..Default::default()
        };
        let mut output = String::new();
        render_operation(&mut output, &api, &api.operations[0]);
        assert!(
            output.contains("poolsterNewIdempotencyKey()"),
            "missing generated key support for {key}"
        );
        assert!(
            output.contains("X-Request-Key"),
            "missing configured header for {key}"
        );
    }
}
