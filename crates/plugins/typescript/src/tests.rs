use super::*;
use poolster_core::engine::Packages;
use poolster_core::{
    Api, Field, HttpMethod, Operation, OperationResponse, Schema, SchemaKind, SchemaValue,
    SecurityRequirement,
};

fn api() -> Api {
    Api {
        name: "Contacts".into(),
        version: "1.0.0".into(),
        schemas: vec![Schema::new(
            "Contact",
            SchemaValue::new(SchemaKind::Object {
                fields: vec![Field {
                    name: "ids".into(),
                    required: false,
                    value: SchemaValue::new(SchemaKind::Array {
                        items: Box::new(SchemaValue::new(SchemaKind::Integer)),
                    }),
                    annotations: Default::default(),
                }],
                additional_properties: Default::default(),
            }),
        )],
        operations: vec![Operation {
            id: "listContacts".into(),
            method: HttpMethod::Get,
            path: "/contacts".into(),
            responses: vec![OperationResponse {
                status: "200".into(),
                description: None,
                media_types: vec![poolster_core::OperationMediaType {
                    content_type: "application/json".into(),
                    schema: Some(SchemaValue::reference("#/components/schemas/Contact")),
                }],
            }],
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[test]
fn typed_model_settings_reach_sdk_rendering() {
    let tree = Packages::new()
        .package(package("ts").with(sdk().model_options(ModelOptions {
            array_type: ArrayType::Generic,
            optional_type: OptionalType::Undefined,
            syntax: Syntax::Interface,
            integer_as_string: true,
            ..Default::default()
        })))
        .generate(&api(), None)
        .unwrap();
    let source = tree.get("ts/models/Contact.ts").unwrap();
    assert!(source.contains("export interface Contact"), "{source}");
    assert!(
        source.contains("ids: Array<string> | undefined"),
        "{source}"
    );
}

#[test]
fn enum_values_are_exported_and_flat_layout_imports_the_shared_runtime() {
    let mut api = api();
    let mut status = SchemaValue::new(SchemaKind::String);
    status.enum_values = vec![serde_json::json!("active"), serde_json::json!("disabled")];
    api.schemas.push(Schema::new("Status", status));
    let tree = Packages::new()
        .package(
            package("ts").with(
                sdk()
                    .raw()
                    .group_by_tag(false)
                    .throw_on_error(false)
                    .model_options(ModelOptions {
                        enum_type: EnumType::AsConst,
                        ..Default::default()
                    }),
            ),
        )
        .generate(&api, None)
        .unwrap();
    assert!(
        tree.get("ts/models/schemas_0001.ts")
            .unwrap()
            .contains("export * from './Status.js'")
    );
    assert!(
        tree.get("ts/models/Status.ts")
            .unwrap()
            .contains("as const")
    );
    let operation = tree.get("ts/clients/listContacts.ts").unwrap();
    assert!(operation.contains("from '../.poolster/client.js'"));
    assert!(operation.contains("ThrowOnError extends boolean = false"));
    assert!(tree.get("ts/client.ts").is_none());
}

#[test]
fn dotted_component_names_keep_distinct_model_files_and_exports() {
    let mut api = api();
    api.schemas.push(Schema::new(
        "meeting.participant",
        SchemaValue::new(SchemaKind::String),
    ));
    api.schemas.push(Schema::new(
        "chat.participant",
        SchemaValue::new(SchemaKind::String),
    ));
    let tree = Packages::new()
        .package(package("ts").with(sdk().raw()))
        .generate(&api, None)
        .unwrap();
    assert!(tree.get("ts/models/MeetingParticipant.ts").is_some());
    assert!(tree.get("ts/models/ChatParticipant.ts").is_some());
    let barrel = tree.get("ts/models/schemas_0001.ts").unwrap();
    assert!(barrel.contains("./MeetingParticipant"));
    assert!(barrel.contains("./ChatParticipant"));
}

#[test]
fn namespaced_sdk_keeps_the_root_facade_small_and_splits_resources() {
    let tree = Packages::new()
        .package(package("ts").with(sdk().namespaced()))
        .generate(&api(), None)
        .unwrap();
    let root = tree.get("ts/client.ts").unwrap();
    let resource = tree.get("ts/resources/contacts.ts").unwrap();
    let resource_operations = tree
        .get("ts/resources/contacts/operations_0001.ts")
        .unwrap();
    assert!(root.contains("import { ContactsClient } from './resources/contacts.js'"));
    assert!(!root.contains("import { listContacts }"));
    assert!(resource.contains("export class ContactsClient"));
    assert!(
        resource_operations
            .contains("import { listContacts } from '../../clients/contacts/listContacts.js'")
    );
    assert!(resource_operations.contains(", client }"));
    assert!(!resource_operations.contains("this.client"));
}

#[test]
fn large_sdk_uses_bounded_barrels_and_composed_resource_chunks() {
    let mut source = api();
    source.operations = (0..101)
        .map(|index| Operation {
            id: format!("listContacts{index}"),
            method: HttpMethod::Get,
            path: format!("/contacts/{index}"),
            annotations: std::collections::BTreeMap::from([(
                "tags".into(),
                serde_json::json!(["Contacts"]),
            )]),
            ..Operation::default()
        })
        .collect();
    let tree = Packages::new()
        .package(package("ts").with(sdk().namespaced()))
        .generate(&source, None)
        .unwrap();
    let root = tree.get("ts/index.ts").unwrap();
    assert!(root.contains("export * from './models/index.js'"));
    assert!(root.contains("export * from './clients/index.js'"));
    assert!(!root.contains("listContacts100"));
    assert!(tree.get("ts/clients/contacts/operations_0002.ts").is_some());
    assert!(
        tree.get("ts/models/contacts/operation_types_0002.ts")
            .is_some()
    );
    let resource = tree.get("ts/resources/contacts.ts").unwrap();
    assert!(resource.contains("extends ContactsClientOperations0001"));
    assert!(resource.contains("ContactsClientOperations0002"));
    assert!(
        tree.get("ts/resources/contacts/operations_0002.ts")
            .is_some()
    );
}

#[test]
fn query_artifacts_use_actual_operation_modules_not_a_second_transport_api() {
    let artifacts = artifacts::TypeScriptReactQuery
        .generate(&api(), &artifacts::ArtifactOptions::default())
        .unwrap();
    assert!(
        artifacts[0]
            .contents
            .contains("from \"./clients/contacts/listContacts\"")
    );
    assert!(
        artifacts[0]
            .contents
            .contains("Parameters<typeof listContacts>[0]")
    );
    assert!(!artifacts[0].contents.contains("FetchClient"));
}

#[test]
fn missing_security_definitions_fail_instead_of_guessing_from_names() {
    let mut api = api();
    api.operations[0].security = vec![SecurityRequirement {
        schemes: [("myKey".into(), vec![])].into_iter().collect(),
    }];
    let error = Packages::new()
        .package(package("ts").with(sdk()))
        .generate(&api, None)
        .unwrap_err();
    assert!(format!("{error:#}").contains("definition is missing"));
}

fn response_contract_tree() -> poolster_core::GeneratedTree {
    let mut api = api();
    // Require a body field so structural TypeScript assignability cannot
    // mistake an open object schema with only optional fields for an envelope.
    if let SchemaKind::Object { fields, .. } = &mut api.schemas[0].value.kind {
        fields[0].required = true;
    }
    api.operations[0].responses.push(OperationResponse::json(
        "400",
        SchemaValue::new(SchemaKind::String),
    ));
    Packages::new()
        .package(package("raw").with(sdk().raw()))
        .package(package("full").with(sdk().client_name("Contacts")))
        .generate(&api, None)
        .unwrap()
}

#[test]
fn operation_references_and_success_body_types_match_transport_contract() {
    let tree = response_contract_tree();
    for package in ["raw", "full"] {
        let models = tree
            .get(format!("{package}/models/contacts/ListContacts.ts"))
            .unwrap();
        assert!(models.contains("import type { Contact } from '../Contact.js'"));
        let runtime = tree.get(format!("{package}/.poolster/client.ts")).unwrap();
        assert!(runtime.contains("SuccessOf<T> = T[Extract<keyof T, `2${string}`>]"));
        assert!(runtime.contains("ThrowOnError extends true ? SuccessResult<T> : StatusResult<T>"));
        assert!(runtime.contains("security?: SecurityDescriptor[][]"));
        assert!(!runtime.contains("Array.isArray(security[0])"));
        assert!(!runtime.contains("scheme.name ?? scheme.id"));
    }
}

#[test]
#[ignore = "requires Node and POOLSTER_TSC_JS pointing to a locally installed TypeScript compiler"]
fn generated_fetch_consumer_compiles_with_strict_typescript() {
    let compiler =
        std::env::var("POOLSTER_TSC_JS").expect("set POOLSTER_TSC_JS to typescript/lib/tsc.js");
    let directory = std::env::temp_dir().join(format!(
        "poolster-typescript-consumer-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    let mut tree = response_contract_tree();
    tree.insert(
        poolster_core::GeneratedFile::new(
            "consumer.ts",
            r#"
import { listContacts, type Contact } from './raw/index';
import { Contacts } from './full/index';
const rawData: Contact = await listContacts({});
const fullData: Contact = await new Contacts().contacts.list({});
const errorOrSuccess = await listContacts({ throwOnError: false });
if (errorOrSuccess.status === 200) {
  const success: Contact = errorOrSuccess.data;
  console.log(success, errorOrSuccess.contentType);
}
// @ts-expect-error: status-discriminated results are not bare response bodies.
const incorrect: Contact = await listContacts({ throwOnError: false });
// @ts-expect-error: regular operations resolve to the body, not an unwrappable envelope.
await listContacts({}).unwrap();
console.log(rawData, fullData, errorOrSuccess, incorrect);
"#,
        )
        .unwrap(),
    )
    .unwrap();
    tree.write_to(&directory).unwrap();
    let output = std::process::Command::new("node")
        .arg(compiler)
        .args([
            "--noEmit",
            "--strict",
            "--target",
            "ES2022",
            "--module",
            "ESNext",
            "--moduleResolution",
            "bundler",
            "--lib",
            "ES2022,DOM,DOM.Iterable",
            "consumer.ts",
        ])
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}\nGenerated fixture: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        directory.display()
    );
    std::fs::remove_dir_all(&directory).unwrap();
}
