//! Export a neutral multioperation wire fixture for the native conformance runner.
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
    let mut patching = creating.clone();
    patching.id = "patchContact".into();
    patching.method = kaji_core::HttpMethod::Patch;
    let mut unsafe_creating = creating.clone();
    unsafe_creating.id = "unsafeCreateContact".into();
    unsafe_creating.path = "/unsafe".into();
    unsafe_creating.annotations.clear();
    let mut unsafe_patching = unsafe_creating.clone();
    unsafe_patching.id = "unsafePatchContact".into();
    unsafe_patching.method = kaji_core::HttpMethod::Patch;
    api.operations
        .extend([creating, patching, unsafe_creating, unsafe_patching]);
    let mut nullable_note = kaji_core::SchemaValue::new(kaji_core::SchemaKind::String);
    nullable_note.nullable = true;
    api.schemas.push(kaji_core::Schema::new(
        "WireInput",
        kaji_core::SchemaValue::new(kaji_core::SchemaKind::Object {
            fields: vec![
                kaji_core::Field {
                    name: "enabled".into(),
                    value: kaji_core::SchemaValue::new(kaji_core::SchemaKind::Boolean),
                    required: true,
                    annotations: Default::default(),
                },
                kaji_core::Field {
                    name: "count".into(),
                    value: kaji_core::SchemaValue::new(kaji_core::SchemaKind::Integer),
                    required: true,
                    annotations: Default::default(),
                },
                kaji_core::Field {
                    name: "note".into(),
                    value: nullable_note,
                    required: true,
                    annotations: Default::default(),
                },
                kaji_core::Field {
                    name: "missing".into(),
                    value: kaji_core::SchemaValue::new(kaji_core::SchemaKind::String),
                    required: false,
                    annotations: Default::default(),
                },
            ],
            additional_properties: kaji_core::AdditionalProperties::Forbidden,
        }),
    ));
    let mut echo = api.operations[0].clone();
    echo.id = "echoWire".into();
    echo.method = kaji_core::HttpMethod::Post;
    echo.path = "/wire/{key}".into();
    echo.parameters = vec![];
    for (name, location, kind, required) in [
        ("key", "path", kaji_core::SchemaKind::String, true),
        ("text", "query", kaji_core::SchemaKind::String, false),
        ("flag", "query", kaji_core::SchemaKind::Boolean, false),
        ("count", "query", kaji_core::SchemaKind::Integer, false),
        (
            "tags",
            "query",
            kaji_core::SchemaKind::Array {
                items: Box::new(kaji_core::SchemaValue::new(kaji_core::SchemaKind::String)),
            },
            false,
        ),
        ("X-Label", "header", kaji_core::SchemaKind::String, false),
    ] {
        echo.parameters.push(kaji_core::OperationParameter {
            name: name.into(),
            location: location.into(),
            required,
            schema: Some(kaji_core::SchemaValue::new(kind)),
            description: None,
            annotations: Default::default(),
        });
    }
    echo.request_body = Some(kaji_core::OperationRequestBody {
        required: true,
        media_types: vec![kaji_core::OperationMediaType {
            content_type: "application/json".into(),
            schema: Some(kaji_core::SchemaValue::reference(
                "#/components/schemas/WireInput",
            )),
        }],
        description: None,
    });
    api.operations.push(echo);
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
