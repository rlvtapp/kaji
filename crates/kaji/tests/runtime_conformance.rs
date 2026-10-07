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
    api.schemas.push(kaji_core::Schema::new(
        "ContactPage",
        kaji_core::SchemaValue::new(kaji_core::SchemaKind::Object {
            fields: vec![kaji_core::Field {
                name: "items".into(),
                value: kaji_core::SchemaValue::new(kaji_core::SchemaKind::Array {
                    items: Box::new(kaji_core::SchemaValue::reference(
                        "#/components/schemas/Contact",
                    )),
                }),
                required: true,
                annotations: Default::default(),
            }],
            additional_properties: kaji_core::AdditionalProperties::Forbidden,
        }),
    ));
    let mut listing = api.operations[0].clone();
    listing.id = "listContacts".into();
    listing.path = "/contacts".into();
    listing.parameters = ["page", "limit"]
        .into_iter()
        .map(|name| kaji_core::OperationParameter {
            name: name.into(),
            location: "query".into(),
            required: false,
            schema: Some(kaji_core::SchemaValue::new(kaji_core::SchemaKind::Integer)),
            description: None,
            annotations: Default::default(),
        })
        .collect();
    listing.responses = vec![kaji_core::OperationResponse::json(
        "200",
        kaji_core::SchemaValue::reference("#/components/schemas/ContactPage"),
    )];
    listing.annotations.insert("x-kaji-pagination".into(), serde_json::json!({"type":"page","inputs":[{"name":"page","in":"parameters","type":"page"},{"name":"limit","in":"parameters","type":"limit"}],"outputs":{"results":"/items"}}));
    api.operations.push(listing);
    let mut creating = api.operations[0].clone();
    creating.id = "createContact".into();
    creating.method = kaji_core::HttpMethod::Post;
    creating.path = "/contacts".into();
    creating.parameters.clear();
    creating.annotations.insert(
        "x-kaji-idempotency".into(),
        serde_json::json!({"header":"X-Once", "auto_generate":true}),
    );
    api.operations.push(creating);
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
        let pager = match language {
            "rust" | "python" | "ruby" | "elixir" => "list_contacts_pages",
            "csharp" => "ListContactsPagesAsync",
            "go" => "ListContactsPages",
            "typescript" => "listPages",
            _ => "listContactsPages",
        };
        assert!(
            tree.iter()
                .any(|(path, file)| path.starts_with(format!("sdk/{language}"))
                    && file.contains(pager)),
            "missing page-number helper {language}: {pager}"
        );
    }
    if let Some(directory) = std::env::var_os("KAJI_RUNTIME_EXPORT") {
        tree.write_to(std::path::Path::new(&directory)).unwrap();
    }
}
