//! Export a neutral multioperation wire fixture for the native conformance runner.
mod support;
use poolster::{generate_with_security_catalog, prelude::*};
use poolster_core::{
    SecurityRequirement, SecurityScheme, SecuritySchemeCatalog, SecuritySchemeKind,
};
use std::collections::BTreeMap;
#[test]
fn export_runtime_contract_fixture() {
    let mut api = support::sdk_contract_api();
    api.name = "Poolster Contract".into();
    api.operations[0].security = vec![SecurityRequirement {
        schemes: BTreeMap::from([("Bearer".into(), vec![])]),
    }];
    api.schemas.push(poolster_core::Schema::new(
        "ContactPage",
        poolster_core::SchemaValue::new(poolster_core::SchemaKind::Object {
            fields: vec![poolster_core::Field {
                name: "items".into(),
                value: poolster_core::SchemaValue::new(poolster_core::SchemaKind::Array {
                    items: Box::new(poolster_core::SchemaValue::reference(
                        "#/components/schemas/Contact",
                    )),
                }),
                required: true,
                annotations: Default::default(),
            }],
            additional_properties: poolster_core::AdditionalProperties::Forbidden,
        }),
    ));
    let mut listing = api.operations[0].clone();
    listing.id = "listContacts".into();
    listing.path = "/contacts".into();
    listing.parameters = ["page", "limit"]
        .into_iter()
        .map(|name| poolster_core::OperationParameter {
            name: name.into(),
            location: "query".into(),
            required: false,
            schema: Some(poolster_core::SchemaValue::new(
                poolster_core::SchemaKind::Integer,
            )),
            description: None,
            annotations: Default::default(),
        })
        .collect();
    listing.responses = vec![poolster_core::OperationResponse::json(
        "200",
        poolster_core::SchemaValue::reference("#/components/schemas/ContactPage"),
    )];
    listing.annotations.insert("x-poolster-pagination".into(), serde_json::json!({"type":"page","inputs":[{"name":"page","in":"parameters","type":"page"},{"name":"limit","in":"parameters","type":"limit"}],"outputs":{"results":"/items"}}));
    api.operations.push(listing);
    let mut creating = api.operations[0].clone();
    creating.id = "createContact".into();
    creating.method = poolster_core::HttpMethod::Post;
    creating.path = "/contacts".into();
    creating.parameters.clear();
    creating.annotations.insert(
        "x-poolster-idempotency".into(),
        serde_json::json!({"header":"X-Once", "auto_generate":true}),
    );
    let mut patching = creating.clone();
    patching.id = "patchContact".into();
    patching.method = poolster_core::HttpMethod::Patch;
    let mut unsafe_creating = creating.clone();
    unsafe_creating.id = "unsafeCreateContact".into();
    unsafe_creating.path = "/unsafe".into();
    unsafe_creating.annotations.clear();
    let mut unsafe_patching = unsafe_creating.clone();
    unsafe_patching.id = "unsafePatchContact".into();
    unsafe_patching.method = poolster_core::HttpMethod::Patch;
    api.operations
        .extend([creating, patching, unsafe_creating, unsafe_patching]);
    let mut nullable_note = poolster_core::SchemaValue::new(poolster_core::SchemaKind::String);
    nullable_note.nullable = true;
    api.schemas.push(poolster_core::Schema::new(
        "WireInput",
        poolster_core::SchemaValue::new(poolster_core::SchemaKind::Object {
            fields: vec![
                poolster_core::Field {
                    name: "enabled".into(),
                    value: poolster_core::SchemaValue::new(poolster_core::SchemaKind::Boolean),
                    required: true,
                    annotations: Default::default(),
                },
                poolster_core::Field {
                    name: "count".into(),
                    value: poolster_core::SchemaValue::new(poolster_core::SchemaKind::Integer),
                    required: true,
                    annotations: Default::default(),
                },
                poolster_core::Field {
                    name: "note".into(),
                    value: nullable_note,
                    required: true,
                    annotations: Default::default(),
                },
                poolster_core::Field {
                    name: "missing".into(),
                    value: poolster_core::SchemaValue::new(poolster_core::SchemaKind::String),
                    required: false,
                    annotations: Default::default(),
                },
            ],
            additional_properties: poolster_core::AdditionalProperties::Forbidden,
        }),
    ));
    let mut echo = api.operations[0].clone();
    echo.id = "echoWire".into();
    echo.method = poolster_core::HttpMethod::Post;
    echo.path = "/wire/{key}".into();
    echo.parameters = vec![];
    for (name, location, kind, required) in [
        ("key", "path", poolster_core::SchemaKind::String, true),
        ("text", "query", poolster_core::SchemaKind::String, false),
        ("flag", "query", poolster_core::SchemaKind::Boolean, false),
        ("count", "query", poolster_core::SchemaKind::Integer, false),
        (
            "tags",
            "query",
            poolster_core::SchemaKind::Array {
                items: Box::new(poolster_core::SchemaValue::new(
                    poolster_core::SchemaKind::String,
                )),
            },
            false,
        ),
        (
            "X-Label",
            "header",
            poolster_core::SchemaKind::String,
            false,
        ),
    ] {
        echo.parameters.push(poolster_core::OperationParameter {
            name: name.into(),
            location: location.into(),
            required,
            schema: Some(poolster_core::SchemaValue::new(kind)),
            description: None,
            annotations: Default::default(),
        });
    }
    echo.request_body = Some(poolster_core::OperationRequestBody {
        required: true,
        media_types: vec![poolster_core::OperationMediaType {
            content_type: "application/json".into(),
            schema: Some(poolster_core::SchemaValue::reference(
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
                poolster::ts::package("typescript")
                    .name("contractsdk")
                    .with(poolster::ts::sdk().fetch()),
            )
            .package(
                poolster::go::package("go")
                    .name("contractsdk")
                    .with(poolster::go::sdk()),
            )
            .package(
                poolster::python::package("python")
                    .name("contractsdk")
                    .with(poolster::python::sdk()),
            )
            .package(
                poolster::rust::package("rust")
                    .name("contractsdk")
                    .with(poolster::rust::sdk()),
            )
            .package(
                poolster::ruby::package("ruby")
                    .name("contractsdk")
                    .with(poolster::ruby::sdk()),
            )
            .package(
                poolster::swift::package("swift")
                    .name("contractsdk")
                    .with(poolster::swift::sdk()),
            )
            .package(
                poolster::java::package("java")
                    .name("contract.sdk")
                    .with(poolster::java::sdk()),
            )
            .package(
                poolster::dotnet::package("csharp")
                    .name("ContractSdk")
                    .with(poolster::dotnet::sdk()),
            )
            .package(
                poolster::php::package("php")
                    .name("contract/sdk")
                    .with(poolster::php::sdk()),
            )
            .package(
                poolster::elixir::package("elixir")
                    .name("contractsdk")
                    .with(poolster::elixir::sdk()),
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
    if let Some(directory) = std::env::var_os("POOLSTER_RUNTIME_EXPORT") {
        tree.write_to(std::path::Path::new(&directory)).unwrap();
    }
}
