use super::*;

#[test]
fn config_profiles_emit_a_typescript_cli_with_oauth() {
    let packages: Vec<PackageConfig> = serde_json::from_value(serde_json::json!([
        {
            "language": "typescript-cli",
            "path": "cli",
            "name": "@acme/cli",
            "plugins": [{
                "name": "cli",
                "command_name": "acme",
                "base_url": "https://api.acme.test/v1",
                "oauth": {
                    "security_scheme": "OAuth",
                    "client_id": "acme-cli",
                    "preferred_flow": "device",
                    "device_authorization_url": "https://auth.acme.test/device",
                    "token_url": "https://auth.acme.test/token",
                    "scopes": ["projects:read"]
                }
            }]
        }
    ]))
    .unwrap();
    let profiles = config_profiles(SdkClientStyle::Namespaced, &packages).unwrap();
    let api = Api {
        name: "Acme".into(),
        version: "1.0.0".into(),
        operations: vec![poolster_core::Operation {
            id: "listProjects".into(),
            method: poolster_core::HttpMethod::Get,
            path: "/projects".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let tree = poolster::generate(&api, profiles).unwrap();
    assert!(
        tree.get("./cli/package.json")
            .unwrap()
            .contains("@acme/cli")
    );
    let index = tree.get("./cli/src/index.ts").unwrap();
    assert!(index.contains("\"projects\",\n      \"list\""));
    assert!(index.contains("deviceAuthorizationUrl"));
    assert!(index.contains("ACME_TOKEN"));
    assert!(
        tree.get("./cli/src/runtime.ts")
            .unwrap()
            .contains("browserLogin")
    );
}
