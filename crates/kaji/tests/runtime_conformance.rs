//! Export a single neutral wire fixture for the standalone native conformance runner.
mod support;
use kaji::{generate_with_security_catalog, prelude::*};
use kaji_core::{SecurityRequirement, SecurityScheme, SecuritySchemeCatalog, SecuritySchemeKind};
use std::collections::BTreeMap;
#[test]
fn export_runtime_contract_fixture() {
    let mut api = support::sdk_contract_api();
    api.name = "Kaji Contract".into();
    api.operations[0].security = vec![SecurityRequirement {
        schemes: BTreeMap::from([("Bearer".into(), vec![])]),
    }];
    let catalog = SecuritySchemeCatalog {
        schemes: vec![SecurityScheme {
            name: "Bearer".into(),
            description: None,
            kind: SecuritySchemeKind::Http {
                scheme: Some("bearer".into()),
                bearer_format: None,
            },
        }],
    };
    let tree = generate_with_security_catalog(
        &api,
        ProfileSet::new("sdk")
            .package(
                kaji::ts::package("typescript")
                    .name("contractsdk")
                    .with(kaji::ts::sdk().fetch()),
            )
            .package(
                kaji::go::package("go")
                    .name("contractsdk")
                    .with(kaji::go::sdk()),
            )
            .package(
                kaji::python::package("python")
                    .name("contractsdk")
                    .with(kaji::python::sdk()),
            )
            .package(
                kaji::rust::package("rust")
                    .name("contractsdk")
                    .with(kaji::rust::sdk()),
            )
            .package(
                kaji::ruby::package("ruby")
                    .name("contractsdk")
                    .with(kaji::ruby::sdk()),
            )
            .package(
                kaji::swift::package("swift")
                    .name("contractsdk")
                    .with(kaji::swift::sdk()),
            )
            .package(
                kaji::java::package("java")
                    .name("contract.sdk")
                    .with(kaji::java::sdk()),
            )
            .package(
                kaji::dotnet::package("csharp")
                    .name("ContractSdk")
                    .with(kaji::dotnet::sdk()),
            )
            .package(
                kaji::php::package("php")
                    .name("contract/sdk")
                    .with(kaji::php::sdk()),
            )
            .package(
                kaji::elixir::package("elixir")
                    .name("contractsdk")
                    .with(kaji::elixir::sdk()),
            ),
        Some(&catalog),
    )
    .unwrap();
    for language in [
        "typescript",
        "go",
        "python",
        "rust",
        "ruby",
        "swift",
        "java",
        "csharp",
        "php",
        "elixir",
    ] {
        assert!(
            tree.iter()
                .any(|(path, _)| path.starts_with(format!("sdk/{language}"))),
            "missing {language}"
        );
    }
    if let Some(directory) = std::env::var_os("KAJI_RUNTIME_EXPORT") {
        tree.write_to(std::path::Path::new(&directory)).unwrap();
    }
}
