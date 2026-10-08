use super::super::check_command::{load_check_baseline, write_check_baseline};
use super::*;
use crate::check_rules::{CheckDiagnostic, CheckSidecarOperation, check_api};
use crate::openapi_sources::remote_request;

#[test]
fn check_baselines_use_stable_fingerprints() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("baseline.json");
    let diagnostics = vec![CheckDiagnostic {
        code: "missing-operation-id",
        severity: CheckSeverity::Error,
        method: "GET".into(),
        path: "/users".into(),
        message: "irrelevant to the baseline".into(),
        hint: "irrelevant to the baseline".into(),
    }];
    write_check_baseline(&path, &diagnostics).unwrap();
    assert_eq!(
        load_check_baseline(&path).unwrap(),
        BTreeSet::from(["missing-operation-id:GET:/users".into()])
    );
    let document: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(document["version"], 1);
    assert_eq!(
        document["$schema"],
        "https://raw.githubusercontent.com/rlvtapp/kaji/main/schemas/v1/check-baseline.schema.json"
    );
}

#[test]
fn contract_check_reports_generator_facing_issues() {
    let api = Api {
        operations: vec![
            Operation {
                id: "inferred".into(),
                method: poolster_core::HttpMethod::Get,
                path: "/admin//users/{userId}".into(),
                parameters: vec![OperationParameter {
                    name: "userId".into(),
                    location: "path".into(),
                    required: false,
                    schema: None,
                    description: None,
                    annotations: BTreeMap::new(),
                }],
                responses: vec![],
                ..Operation::default()
            },
            Operation {
                id: "get-user".into(),
                method: poolster_core::HttpMethod::Get,
                path: "/users/{id}".into(),
                responses: vec![OperationResponse::json(
                    "200",
                    poolster_core::SchemaValue::unknown(),
                )],
                ..Operation::default()
            },
            Operation {
                id: "get_user".into(),
                method: poolster_core::HttpMethod::Get,
                path: "/people".into(),
                responses: vec![OperationResponse::json(
                    "200",
                    poolster_core::SchemaValue::unknown(),
                )],
                ..Operation::default()
            },
        ],
        ..Api::default()
    };
    let source = vec![
        CheckSidecarOperation {
            operation_id: String::new(),
        },
        CheckSidecarOperation {
            operation_id: "get-user".into(),
        },
        CheckSidecarOperation {
            operation_id: "get_user".into(),
        },
    ];
    let codes = check_api(&api, &source)
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();
    assert!(codes.contains(&"missing-operation-id"));
    assert!(codes.contains(&"ambiguous-path"));
    assert!(codes.contains(&"optional-path-parameter"));
    assert!(codes.contains(&"missing-success-response"));
    assert!(codes.contains(&"missing-path-parameter"));
    assert!(codes.contains(&"ambiguous-operation-id"));
}

#[test]
fn native_mock_paths_require_values_and_prefer_literal_routes() {
    assert!(mock_path_matches("/users/{id}", "/users/me"));
    assert!(!mock_path_matches("/users/{id}", "/users/"));
    assert!(mock_path_specificity("/users/me") > mock_path_specificity("/users/{id}"));
}

#[test]
fn native_mock_decodes_query_and_path_values_for_scenarios() {
    let (path, query) = mock_target_parts("/notes/note%201?expand=full%20body&tag=a%2Bb");
    assert_eq!(path, "/notes/note%201");
    assert_eq!(query["expand"], "full body");
    assert_eq!(query["tag"], "a+b");
    let parameters = mock_path_parameters("/notes/{noteId}", path);
    assert_eq!(parameters["noteId"], "note 1");
    assert_eq!(
        mock_response_body("text/plain", Some(serde_json::json!("rate limited"))).unwrap(),
        b"rate limited"
    );
}

#[test]
fn compiler_origin_preserves_resolution_without_forwarding_url_credentials() {
    let remote = RemoteInput {
        url: "https://user:secret@example.com/spec/root.yaml?version=1".into(),
        headers: BTreeMap::new(),
        auth: None,
    };
    assert_eq!(
        compiler_source_origin(&remote).unwrap(),
        "https://example.com/spec/root.yaml?version=1"
    );
    assert!(remote.url.contains("user:secret"));
    let plain = RemoteInput {
        url: "http://example.com/spec.yaml".into(),
        ..remote
    };
    assert_eq!(compiler_source_origin(&plain).unwrap(), plain.url);
}

#[test]
fn remote_input_applies_headers_and_basic_auth_before_downloading() {
    let source = RemoteInput {
        url: "https://example.com/openapi.yaml".into(),
        headers: BTreeMap::from([("X-OpenAPI-Key".into(), SecretValue::Literal("key".into()))]),
        auth: Some(RemoteAuth::Basic {
            username: SecretValue::Literal("alice".into()),
            password: SecretValue::Literal("secret".into()),
        }),
    };
    let client = reqwest::blocking::Client::builder().build().unwrap();
    let request = remote_request(&client, &source).unwrap().build().unwrap();
    assert_eq!(request.url().as_str(), source.url);
    assert_eq!(request.headers()["x-openapi-key"], "key");
    assert_eq!(request.headers()["authorization"], "Basic YWxpY2U6c2VjcmV0");
}
