use super::*;
use poolster_core::engine::Packages;
use poolster_core::{
    HttpMethod, Operation, SecurityRequirement, SecurityScheme, SecuritySchemeCatalog,
    SecuritySchemeKind,
};
use std::collections::BTreeMap;

fn large_api() -> Api {
    Api {
        name: "Chunk probe".into(),
        version: "1.0.0".into(),
        operations: (0..101)
            .map(|index| Operation {
                id: format!("listItems{index}"),
                method: HttpMethod::Get,
                path: format!("/resource{index}"),
                ..Operation::default()
            })
            .collect(),
        ..Api::default()
    }
}

#[test]
#[ignore = "requires native Cargo CLI dependencies"]
fn split_commands_compile_and_show_last_chunk_in_help() {
    let root = tempfile::tempdir().unwrap();
    Packages::new()
        .package(package("cli").with(cli().command_name("probe")))
        .generate(&large_api(), None)
        .unwrap()
        .write_to(root.path())
        .unwrap();
    let target = std::env::var_os("POOLSTER_CLI_CARGO_TARGET_DIR")
        .unwrap_or_else(|| root.path().join("target").into_os_string());
    let mut cargo = std::process::Command::new("cargo");
    cargo.arg("run");
    if std::env::var("POOLSTER_RUNTIME_OFFLINE").as_deref() == Ok("1") {
        cargo.arg("--offline");
    }
    let output = cargo
        .args(["--quiet", "--", "--help"])
        .env("CARGO_TARGET_DIR", target)
        .current_dir(root.path().join("cli"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(
        help.contains("resource0") && help.contains("resource100"),
        "{help}"
    );
}

#[test]
fn emits_one_authentication_extension_with_openapi_default() {
    let api = Api {
        name: "Example".into(),
        operations: vec![Operation {
            id: "listUsers".into(),
            method: HttpMethod::Get,
            path: "/admin/users".into(),
            ..Operation::default()
        }],
        ..Api::default()
    };
    let tree = Packages::new()
        .package(package("cli").with(cli()))
        .generate(&api, None)
        .unwrap();
    let main = tree.get("cli/src/main.rs").unwrap();
    let admin = tree.get("cli/references/admin.md").unwrap();
    let extension = tree.get("cli/src/poolster_extension.rs").unwrap();
    assert!(main.contains("extension.authenticate(&mut headers, &mut query, &context)?"));
    assert!(main.contains("AuthenticationResult::UseOpenApi"));
    assert!(main.contains("fn is_interactive(matches: &ArgMatches) -> bool"));
    assert!(main.contains("prompt_secret(\"Credential\")?"));
    assert!(main.contains("dialoguer::{Input, Password}"));
    assert!(main.contains("\"code\": \"cli_error\""));
    assert!(admin.contains("## example admin users list"));
    assert!(extension.contains("pub trait Extension"));
    assert!(extension.contains("pub enum AuthenticationResult { Handled, UseOpenApi }"));
    assert!(extension.contains("fn authenticate(&self, _headers: &mut HeaderMap"));
    assert!(extension.contains("#[allow(dead_code)]"));
    assert!(tree.preserves_existing("cli/src/poolster_extension.rs"));
    assert!(tree.get("cli/src/poolster_auth.rs").is_none());
}

#[test]
fn groups_admin_paths() {
    let operation = Operation {
        id: "listUsers".into(),
        method: HttpMethod::Get,
        path: "/admin/users".into(),
        ..Operation::default()
    };
    assert_eq!(command_parts(&operation), ["admin", "users", "list"]);
}

#[test]
fn materializes_openapi_api_keys_and_profile_commands() {
    let api = Api {
        operations: vec![Operation {
            id: "listWidgets".into(),
            method: HttpMethod::Get,
            path: "/widgets".into(),
            security: vec![SecurityRequirement {
                schemes: BTreeMap::from([("WidgetKey".into(), vec![])]),
            }],
            ..Operation::default()
        }],
        ..Api::default()
    };
    let catalog = SecuritySchemeCatalog {
        schemes: vec![SecurityScheme {
            name: "WidgetKey".into(),
            description: None,
            kind: SecuritySchemeKind::ApiKey {
                name: Some("x-widget-key".into()),
                location: Some("header".into()),
            },
        }],
    };
    let main = render_main(&api, "widgets", None, Some(&catalog));
    assert!(main.contains("kind: \"api-key\""));
    assert!(main.contains("name: \"x-widget-key\""));
    assert!(main.contains("credential(\"set-key\")"));
    assert!(main.contains("Command::new(\"profiles\")"));
    assert!(main.contains("env_key(scheme.id)"));
}
