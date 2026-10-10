#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs};

    use super::*;
    use poolster_core::ast::{AdditionalProperties, Field, HttpMethod, SchemaKind, SchemaValue};
    use poolster_core::{
        Api, Operation, OperationMediaType, OperationResponse, Schema, SecurityRequirement,
    };

    fn rendered_source(files: &[GeneratedFile]) -> String {
        files
            .iter()
            .map(|file| file.contents.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn native_idempotency_keys_are_stable_across_retries() {
        use poolster_core::engine::Packages;
        let operation = Operation {
            id: "createItem".into(),
            method: HttpMethod::Post,
            path: "/items".into(),
            responses: vec![OperationResponse::json(
                "200",
                SchemaValue::new(SchemaKind::Integer),
            )],
            parameters: vec![OperationParameter {
                name: "x_once".into(),
                location: "query".into(),
                required: false,
                schema: Some(SchemaValue::new(SchemaKind::String)),
                description: None,
                annotations: BTreeMap::new(),
            }],
            annotations: BTreeMap::from([(
                "x-poolster-idempotency".into(),
                serde_json::json!({"header":"Query", "auto_generate":true}),
            )]),
            ..Default::default()
        };
        let api = Api {
            name: "idempotency".into(),
            version: "1.0.0".into(),
            operations: vec![operation],
            ..Default::default()
        };
        let tree = Packages::new()
            .package(crate::package("sdk").with(crate::sdk()))
            .generate(&api, None)
            .unwrap();
        let root = tempfile::tempdir().unwrap();
        tree.write_to(root.path()).unwrap();
        let sdk = root.path().join("sdk");
        let manifest = sdk.join("Cargo.toml");
        let mut cargo = fs::read_to_string(&manifest).unwrap();
        cargo.push_str("\n[dev-dependencies]\nhttp = \"1\"\ntokio = { version = \"1\", features = [\"macros\", \"rt-multi-thread\"] }\n");
        fs::write(manifest, cargo).unwrap();
        let runtime = sdk.join("src/client/mod.rs");
        let mut source = fs::read_to_string(&runtime).unwrap();
        source.push_str(r#"
#[cfg(test)] mod retry_header_tests {
 #[test] fn server_delays() {
  use std::time::Duration;
  let mut headers=reqwest::header::HeaderMap::new();
  headers.insert("retry-after","2".parse().unwrap());assert_eq!(super::poolster_retry_after(&headers),Some(Duration::from_secs(2)));
  headers.insert("retry-after-ms","125.5".parse().unwrap());assert_eq!(super::poolster_retry_after(&headers),Some(Duration::from_micros(125500)));
  for bad in ["-1","NaN","inf","broken"] {headers.insert("retry-after-ms",bad.parse().unwrap());assert_eq!(super::poolster_retry_after(&headers),Some(Duration::from_secs(2)));}
  headers.insert("retry-after-ms","0".parse().unwrap());assert_eq!(super::poolster_retry_after(&headers),Some(Duration::ZERO));headers.remove("retry-after-ms");
  headers.insert("retry-after","Sun, 06 Nov 1994 08:49:37 GMT".parse().unwrap());assert_eq!(super::poolster_retry_after(&headers),Some(Duration::ZERO));
  headers.insert("retry-after","garbage".parse().unwrap());assert_eq!(super::poolster_retry_after(&headers),None);
  let client=super::Client::new("https://unused.test");assert_eq!(client.poolster_retry_delay(1,Some(Duration::from_secs(1000))),client.retry.max_delay);
 }
}
"#);
        fs::write(runtime, source).unwrap();
        fs::create_dir(sdk.join("tests")).unwrap();
        fs::write(sdk.join("tests/idempotency.rs"),r#"
use idempotency_sdk::{Client,client::{CreateItemRequest,RetryConfig},transport::{Transport,TransportFuture}};
use std::sync::{Arc,Mutex};
struct Mock(Arc<Mutex<Vec<String>>>);
impl Transport for Mock {fn execute(&self,request:reqwest::Request)->TransportFuture<'_>{
 assert_eq!(request.url().query(),Some("x_once=query-value"));
 let key=request.headers().get("Query").unwrap().to_str().unwrap().to_owned();let mut seen=self.0.lock().unwrap();seen.push(key);let status=if seen.len()%2==1 {503}else{200};
 Box::pin(async move{Ok(http::Response::builder().status(status).body(reqwest::Body::from("1")).unwrap().into())})
}}
#[tokio::test] async fn stable_and_overridable(){
 let seen=Arc::new(Mutex::new(Vec::new()));let client=Client::new("https://unused.test").with_transport(Arc::new(Mock(seen.clone()))).with_retry(RetryConfig {max_attempts:2,initial_delay:std::time::Duration::ZERO,max_delay:std::time::Duration::ZERO});
 for _ in 0..2 {assert_eq!(client.create_item(CreateItemRequest{x_once:Some("query-value".into()),query2:None}).await.unwrap(),1);}
 assert_eq!(client.create_item(CreateItemRequest{x_once:Some("query-value".into()),query2:Some("durable-key".into())}).await.unwrap(),1);
 let keys=seen.lock().unwrap();assert_eq!(keys.len(),6);assert_eq!(keys[0],keys[1]);assert_eq!(keys[2],keys[3]);assert_ne!(keys[0],keys[2]);assert_eq!(uuid::Uuid::parse_str(&keys[0]).unwrap().get_version_num(),4);assert_eq!(keys[4],"durable-key");assert_eq!(keys[4],keys[5]);drop(keys);
 for blank in ["", "   "] {seen.lock().unwrap().clear();assert!(client.create_item(CreateItemRequest{x_once:Some("query-value".into()),query2:Some(blank.into())}).await.is_err());let keys=seen.lock().unwrap();assert_eq!(keys.len(),1);assert_eq!(keys[0],blank);}
}
"#).unwrap();
        let output = crate::native_cargo()
            .args(["test", "--quiet"])
            .env("RUSTFLAGS", "-Dwarnings")
            .current_dir(sdk)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn native_url_pages_preserve_origin_auth_and_raw_error_responses() {
        use poolster_core::engine::Packages;
        let mut operation = Operation {
            id: "listLinks".into(),
            method: HttpMethod::Get,
            path: "/links".into(),
            responses: vec![
                OperationResponse::json("200", SchemaValue::new(SchemaKind::Any)),
                OperationResponse::json("400", SchemaValue::new(SchemaKind::Any)),
                OperationResponse::json("default", SchemaValue::new(SchemaKind::Any)),
            ],
            ..Default::default()
        };
        operation.security.push(poolster_core::SecurityRequirement {
            schemes: Default::default(),
        });
        operation.annotations.insert(
            "x-poolster-pagination".into(),
            serde_json::json!({"type":"url","outputs":{"nextUrl":"/next"}}),
        );
        let api = Api {
            name: "url".into(),
            version: "1.0.0".into(),
            operations: vec![operation],
            ..Default::default()
        };
        let root = tempfile::tempdir().unwrap();
        Packages::new()
            .package(crate::package("sdk").with(crate::sdk()))
            .generate(&api, None)
            .unwrap()
            .write_to(root.path())
            .unwrap();
        let sdk = root.path().join("sdk");
        let manifest = sdk.join("Cargo.toml");
        let mut cargo = fs::read_to_string(&manifest).unwrap();
        cargo.push_str("\n[dev-dependencies]\nhttp=\"1\"\n");
        fs::write(manifest, cargo).unwrap();
        fs::create_dir(sdk.join("tests")).unwrap();
        fs::write(
            sdk.join("tests/url.rs"),
            include_str!("../tests/fixtures/url_pagination_test.rs"),
        )
        .unwrap();
        let output = crate::native_cargo()
            .args(["test", "--quiet"])
            .env("RUSTFLAGS", "-Dwarnings")
            .current_dir(sdk)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    #[test]
    fn native_page_stream_defaults_and_preserves_required_controls() {
        use poolster_core::engine::Packages;
        let mut operation = Operation {
            id: "listItems".into(),
            method: HttpMethod::Get,
            path: "/items".into(),
            parameters: ["page", "limit"]
                .into_iter()
                .map(|name| OperationParameter {
                    name: name.into(),
                    location: "query".into(),
                    required: false,
                    schema: Some(SchemaValue::new(SchemaKind::Integer)),
                    description: None,
                    annotations: Default::default(),
                })
                .collect(),
            responses: vec![OperationResponse::json(
                "200",
                SchemaValue::new(SchemaKind::Object {
                    fields: vec![Field {
                        name: "items".into(),
                        value: SchemaValue::new(SchemaKind::Array {
                            items: Box::new(SchemaValue::new(SchemaKind::Integer)),
                        }),
                        required: true,
                        annotations: Default::default(),
                    }],
                    additional_properties: AdditionalProperties::Any,
                }),
            )],
            ..Default::default()
        };
        operation.annotations.insert("x-poolster-pagination".into(),serde_json::json!({"type":"page","inputs":[{"name":"page","in":"parameters","type":"page"},{"name":"limit","in":"parameters","type":"limit"}],"outputs":{"results":"/items"}}));
        let mut required = operation.clone();
        required.id = "requiredItems".into();
        required.parameters[0].location = "header".into();
        required.parameters[0].required = true;
        let mut legacy = operation.clone();
        legacy.id = "legacyItems".into();
        legacy.annotations.insert("x-poolster-pagination".into(),serde_json::json!({"type":"offsetLimit","inputs":[{"name":"page","in":"parameters","type":"page"}],"outputs":{"numPages":"$.numPages"}}));
        let mut offset = operation.clone();
        offset.id = "offsetItems".into();
        offset.parameters[0].name = "offset".into();
        offset.annotations.insert("x-poolster-pagination".into(),serde_json::json!({"type":"offsetLimit","inputs":[{"name":"offset","in":"parameters","type":"offset"},{"name":"limit","in":"parameters","type":"limit"}],"outputs":{"results":"/items"}}));
        let api = Api {
            name: "page".into(),
            version: "1.0.0".into(),
            operations: vec![operation, required, legacy, offset],
            ..Default::default()
        };
        let tree = Packages::new()
            .package(crate::package("sdk").with(crate::sdk()))
            .generate(&api, None)
            .unwrap();
        let root = tempfile::tempdir().unwrap();
        tree.write_to(root.path()).unwrap();
        let sdk = root.path().join("sdk");
        let manifest = sdk.join("Cargo.toml");
        let mut cargo = fs::read_to_string(&manifest).unwrap();
        cargo.push_str("\n[dev-dependencies]\nhttp = \"1\"\n");
        fs::write(manifest, cargo).unwrap();
        let runtime = sdk.join("src/client/mod.rs");
        let mut source = fs::read_to_string(&runtime).unwrap();
        source.push_str(r#"
#[cfg(test)] mod selector_checks {
    #[test] fn portable_selectors() {
        let value=serde_json::json!({"a/b":{"~items":[[1,2]]}});
        assert_eq!(super::poolster_json_path(&value,"/a~1b/~0items/0/1"),Some(&serde_json::json!(2)));
        assert_eq!(super::poolster_json_path(&serde_json::json!([{"items":[1,2]}]),"$[0].items[-1]"),Some(&serde_json::json!(2)));
        for pointer in ["/items/01","/items/-1","/items/+1","/items/", "/bad~2"] { assert!(super::poolster_json_path(&serde_json::json!({"items":[1,2]}),pointer).is_none()); }
    }
}
"#);
        fs::write(runtime, source).unwrap();
        fs::create_dir(sdk.join("tests")).unwrap();
        fs::write(sdk.join("tests/page.rs"),r#"
use page_sdk::{Client,client::{ListItemsRequest,RequiredItemsRequest,LegacyItemsRequest,OffsetItemsRequest},transport::{Transport,TransportFuture}};
use std::{future::Future,sync::{Arc,Mutex},task::{Context,Poll,Waker}};
use futures_util::StreamExt;
fn ready<F:Future>(future:F)->F::Output {let mut future=Box::pin(future);match future.as_mut().poll(&mut Context::from_waker(Waker::noop())) {Poll::Ready(value)=>value,Poll::Pending=>panic!("mock pending")}}
struct Mock(Arc<Mutex<Vec<i64>>>);
impl Transport for Mock {fn execute(&self, request:reqwest::Request)->TransportFuture<'_>{
 let page=request.url().query_pairs().find(|(key,_)|key=="page" || key=="offset").map(|(_,value)|value.parse::<i64>().unwrap()).or_else(||request.headers().get("page").map(|v|v.to_str().unwrap().parse().unwrap())).unwrap();self.0.lock().unwrap().push(page);
 let items=if page<3 || page==i64::MAX {serde_json::json!([page])} else {serde_json::json!([])};
 let body=serde_json::json!({"items":items,"numPages":if page==i64::MAX {i64::MAX} else {3}}).to_string();Box::pin(async move{Ok(http::Response::builder().status(200).header("content-type","application/json").body(reqwest::Body::from(body)).unwrap().into())})
}}
#[test] fn page_stream(){
 let seen=Arc::new(Mutex::new(Vec::new()));let client=Client::new("https://unused.test").with_transport(Arc::new(Mock(seen.clone())));
 let input=ListItemsRequest{page:None,limit:None};let mut pages=Box::pin(client.list_items_pages(input.clone()));let mut count=0;while let Some(page)=ready(pages.next()){page.unwrap();count+=1;}assert_eq!(count,3);assert_eq!(*seen.lock().unwrap(),[1,2,3]);assert_eq!(input.page,None);
 seen.lock().unwrap().clear();let mut pages=Box::pin(client.list_items_pages(ListItemsRequest{page:Some(0),limit:Some(2)}));assert!(ready(pages.next()).unwrap().is_ok());assert!(ready(pages.next()).is_none());assert_eq!(*seen.lock().unwrap(),[0]);
 seen.lock().unwrap().clear();let mut pages=Box::pin(client.required_items_pages(RequiredItemsRequest{page:2,limit:Some(2)}));assert!(ready(pages.next()).unwrap().is_ok());assert!(ready(pages.next()).is_none());assert_eq!(*seen.lock().unwrap(),[2]);
 seen.lock().unwrap().clear();let mut pages=Box::pin(client.list_items_pages(ListItemsRequest{page:Some(i64::MAX),limit:None}));assert!(ready(pages.next()).unwrap().is_ok());assert!(ready(pages.next()).is_none());assert_eq!(*seen.lock().unwrap(),[i64::MAX]);
 seen.lock().unwrap().clear();let mut pages=Box::pin(client.legacy_items_pages(LegacyItemsRequest{page:None,limit:None}));let mut count=0;while let Some(page)=ready(pages.next()){page.unwrap();count+=1;}assert_eq!(count,3);assert_eq!(*seen.lock().unwrap(),[1,2,3]);
 seen.lock().unwrap().clear();let mut pages=Box::pin(client.offset_items_pages(OffsetItemsRequest{offset:None,limit:Some(2)}));assert!(ready(pages.next()).unwrap().is_ok());assert!(ready(pages.next()).is_none());assert_eq!(*seen.lock().unwrap(),[0]);
 seen.lock().unwrap().clear();let mut pages=Box::pin(client.legacy_items_pages(LegacyItemsRequest{page:Some(i64::MAX),limit:None}));assert!(ready(pages.next()).unwrap().is_ok());assert!(ready(pages.next()).is_none());assert_eq!(*seen.lock().unwrap(),[i64::MAX]);
 for limit in [0,-1] { seen.lock().unwrap().clear();let mut pages=Box::pin(client.list_items_pages(ListItemsRequest{page:None,limit:Some(limit)}));assert!(ready(pages.next()).unwrap().is_err());assert!(seen.lock().unwrap().is_empty()); }
 seen.lock().unwrap().clear();let mut pages=Box::pin(client.list_items_pages(ListItemsRequest{page:Some(-1),limit:None}));assert!(ready(pages.next()).unwrap().is_err());assert!(seen.lock().unwrap().is_empty());
}
"#).unwrap();
        let output = crate::native_cargo()
            .args(["test", "--quiet"])
            .env("RUSTFLAGS", "-Dwarnings")
            .current_dir(sdk)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn emits_a_reqwest_package_with_serde_models() {
        let api = Api {
            name: "Pets API".into(),
            version: "1.2.3".into(),
            schemas: vec![Schema::new(
                "Pet",
                SchemaValue::new(SchemaKind::Object {
                    fields: vec![Field {
                        name: "display-name".into(),
                        value: SchemaValue::new(SchemaKind::String),
                        required: true,
                        annotations: BTreeMap::new(),
                    }],
                    additional_properties: AdditionalProperties::Forbidden,
                }),
            )],
            operations: vec![Operation {
                id: "getPet".into(),
                method: HttpMethod::Get,
                path: "/pets/{id}".into(),
                annotations: BTreeMap::new(),

                responses: vec![OperationResponse::json(
                    "200",
                    SchemaValue::reference("#/components/schemas/Pet"),
                )],
                ..Default::default()
            }],
            annotations: BTreeMap::new(),
        };
        let config = RenderOptions::default();
        let models = RustModels.generate(&api, &config).unwrap();
        let client = RustReqwest.generate(&api, &config).unwrap();
        let package = RustPackage.generate(&api, &config).unwrap();
        assert!(rendered_source(&models).contains("#[serde(rename = \"display-name\")]"));
        assert!(rendered_source(&client).contains("pub async fn get_pet"));
        assert!(package[1].contents.contains("pets-api-sdk"));
    }

    #[test]
    fn converts_date_style_openapi_versions_to_cargo_semver() {
        assert_eq!(cargo_package_version("2026-09-19"), "2026.9.19");
        assert_eq!(cargo_package_version("current"), "0.1.0");
    }

    #[test]
    fn splits_models_operations_and_resource_methods_into_bounded_modules() {
        let api = Api {
            schemas: (0..101)
                .map(|index| {
                    Schema::new(
                        format!("Model{index}"),
                        SchemaValue::new(SchemaKind::Object {
                            fields: Vec::new(),
                            additional_properties: AdditionalProperties::Forbidden,
                        }),
                    )
                })
                .collect(),
            operations: (0..101)
                .map(|index| Operation {
                    id: format!("listContacts{index}"),
                    method: HttpMethod::Get,
                    path: format!("/contacts/{index}"),
                    ..Operation::default()
                })
                .collect(),
            ..Api::default()
        };
        let files = generate_sdk(&api, &RenderOptions::default()).unwrap();
        let paths = files
            .iter()
            .map(|(path, _)| path.to_str().unwrap())
            .collect::<Vec<_>>();

        assert!(paths.contains(&"src/models/chunk_0001/mod.rs"));
        assert!(paths.contains(&"src/models/chunk_0002/mod.rs"));
        assert!(paths.contains(&"src/client/operations/chunk_0001.rs"));
        assert!(paths.contains(&"src/client/operations/chunk_0005.rs"));
        assert!(paths.contains(&"src/client/resources/contacts_1/chunk_0001.rs"));
        assert!(paths.contains(&"src/client/resources/contacts_1/chunk_0005.rs"));
        assert!(!paths.contains(&"src/models.rs"));
        assert!(!paths.contains(&"src/client.rs"));
    }

    #[test]
    fn namespaced_surface_exposes_resource_accessors_and_direct_methods() {
        let api = Api {
            name: "Example API".into(),
            operations: vec![Operation {
                id: "listContacts".into(),
                method: HttpMethod::Get,
                path: "/v1/contacts".into(),

                responses: vec![OperationResponse::json(
                    "200",
                    SchemaValue::reference("#/components/schemas/ContactList"),
                )],
                ..Operation::default()
            }],
            ..Api::default()
        };
        let config = RenderOptions::default();

        let client = RustReqwest.generate(&api, &config).unwrap();
        let source = rendered_source(&client);
        assert!(source.contains("pub async fn list_contacts"));
        assert!(source.contains("pub fn contacts(&self) -> ContactsClient"));
        assert!(source.contains("pub async fn list(&self)"));
        assert!(source.contains("self.client.list_contacts().await"));

        let package = RustPackage.generate(&api, &config).unwrap();
        assert!(
            package[2]
                .contents
                .contains("client.contacts().list().await?")
        );
    }

    #[test]
    fn preserves_declared_errors_and_non_json_response_media() {
        let api = Api {
            name: "Exports API".into(),
            operations: vec![Operation {
                id: "downloadExport".into(),
                method: HttpMethod::Get,
                path: "/exports/{id}".into(),
                responses: vec![
                    OperationResponse {
                        status: "200".into(),
                        description: None,
                        media_types: vec![OperationMediaType {
                            content_type: "application/octet-stream".into(),
                            schema: None,
                        }],
                    },
                    OperationResponse {
                        status: "404".into(),
                        description: None,
                        media_types: vec![OperationMediaType {
                            content_type: "application/json".into(),
                            schema: Some(SchemaValue::reference("#/components/schemas/ApiError")),
                        }],
                    },
                ],
                security: vec![SecurityRequirement {
                    schemes: BTreeMap::from([("bearerAuth".into(), Vec::new())]),
                }],
                ..Operation::default()
            }],
            ..Api::default()
        };

        let client = RustReqwest
            .generate(&api, &RenderOptions::default())
            .unwrap();
        let source = rendered_source(&client);
        assert!(source.contains("pub struct ApiResponse"));
        assert!(source.contains("pub enum DownloadExportError"));
        assert!(
            source.contains("NotFound { body: crate::models::ApiError, response: ApiResponse }")
        );
        assert!(source.contains("Result<Vec<u8>, DownloadExportError>"));
        assert!(source.contains("response.bytes().await.map(|body| body.to_vec())"));
        assert!(source.contains("pub fn with_bearer_token"));
        assert!(source.contains("if let Some(token) = &self.bearer_token"));
    }

    #[test]
    fn returns_a_live_response_for_server_sent_events() {
        let api = Api {
            operations: vec![Operation {
                id: "watchEvents".into(),
                method: HttpMethod::Get,
                path: "/events".into(),
                responses: vec![OperationResponse {
                    status: "200".into(),
                    description: None,
                    media_types: vec![OperationMediaType {
                        content_type: "text/event-stream".into(),
                        schema: None,
                    }],
                }],
                ..Operation::default()
            }],
            ..Api::default()
        };

        let client = RustReqwest
            .generate(&api, &RenderOptions::default())
            .unwrap();
        assert!(rendered_source(&client).contains("Result<reqwest::Response, WatchEventsError>"));
        assert!(rendered_source(&client).contains("Ok(response)"));
    }

    #[test]
    fn renders_conservative_retry_and_lifecycle_runtime() {
        let api = Api {
            name: "Retry API".into(),
            schemas: vec![Schema::new(
                "Pet",
                SchemaValue::new(SchemaKind::Object {
                    fields: Vec::new(),
                    additional_properties: AdditionalProperties::Forbidden,
                }),
            )],
            operations: vec![
                Operation {
                    id: "listPets".into(),
                    method: HttpMethod::Get,
                    path: "/pets".into(),
                    responses: vec![OperationResponse {
                        status: "200".into(),
                        description: None,
                        media_types: vec![OperationMediaType {
                            content_type: "application/json".into(),
                            schema: Some(SchemaValue::reference("#/components/schemas/Pet")),
                        }],
                    }],
                    ..Operation::default()
                },
                Operation {
                    id: "createPet".into(),
                    method: HttpMethod::Post,
                    path: "/pets".into(),
                    request_body: Some(poolster_core::OperationRequestBody::json(
                        SchemaValue::reference("#/components/schemas/Pet"),
                        true,
                    )),
                    parameters: vec![OperationParameter {
                        name: "Idempotency-Key".into(),
                        location: "header".into(),
                        required: false,
                        schema: Some(SchemaValue::new(SchemaKind::String)),
                        description: None,
                        annotations: BTreeMap::new(),
                    }],
                    responses: vec![OperationResponse {
                        status: "200".into(),
                        description: None,
                        media_types: vec![OperationMediaType {
                            content_type: "application/json".into(),
                            schema: Some(SchemaValue::reference("#/components/schemas/Pet")),
                        }],
                    }],
                    ..Operation::default()
                },
            ],
            ..Api::default()
        };

        let client = RustReqwest
            .generate(&api, &RenderOptions::default())
            .unwrap();
        let source = rendered_source(&client);
        assert!(source.contains("pub struct RetryConfig"));
        assert!(source.contains("pub trait ClientHooks"));
        assert!(source.contains("pub fn with_retry"));
        assert!(source.contains("pub fn with_hooks"));
        assert!(source.contains("matches!(status.as_u16(), 408 | 429 | 500 | 502 | 503 | 504)"));
        assert!(source.contains("let retry_allowed = true;"));
        assert!(source.contains("let retry_allowed = input.idempotency_key.as_ref().is_some_and(|key| !poolster_query_value(key).trim().is_empty());"));
        assert!(source.contains("self.poolster_after_response(&request_info, &response);"));
    }

    #[test]
    fn emits_stream_paginators_for_safe_parameter_locations() {
        let json_response = || OperationResponse {
            status: "200".into(),
            description: None,
            media_types: vec![OperationMediaType {
                content_type: "application/json".into(),
                schema: None,
            }],
        };
        let optional_integer = |name: &str| OperationParameter {
            name: name.into(),
            location: "query".into(),
            required: false,
            schema: Some(SchemaValue::new(SchemaKind::Integer)),
            description: None,
            annotations: BTreeMap::new(),
        };
        let api = Api {
            operations: vec![
                Operation {
                    id: "listByCursor".into(),
                    method: HttpMethod::Get,
                    path: "/cursor".into(),
                    parameters: vec![OperationParameter {
                        name: "cursor".into(),
                        location: "query".into(),
                        required: false,
                        schema: Some(SchemaValue::new(SchemaKind::String)),
                        description: None,
                        annotations: BTreeMap::new(),
                    }],
                    responses: vec![json_response()],
                    annotations: BTreeMap::from([(
                        "x-poolster-pagination".into(),
                        serde_json::json!({
                            "type": "cursor",
                            "inputs": [{ "name": "cursor", "in": "parameters", "type": "cursor" }],
                            "outputs": { "nextCursor": "$.data[-1].nextCursor" },
                        }),
                    )]),
                    ..Operation::default()
                },
                Operation {
                    id: "listByOffset".into(),
                    method: HttpMethod::Get,
                    path: "/offset".into(),
                    parameters: vec![optional_integer("offset"), optional_integer("limit")],
                    responses: vec![json_response()],
                    annotations: BTreeMap::from([(
                        "x-speakeasy-pagination".into(),
                        serde_json::json!({
                            "type": "offsetLimit",
                            "inputs": [
                                { "name": "offset", "in": "parameters", "type": "offset" },
                                { "name": "limit", "in": "parameters", "type": "limit" },
                            ],
                            "outputs": { "results": "$.data" },
                        }),
                    )]),
                    ..Operation::default()
                },
                Operation {
                    id: "listByHeaderCursor".into(),
                    method: HttpMethod::Get,
                    path: "/header-cursor".into(),
                    parameters: vec![OperationParameter {
                        name: "X-Page-Cursor".into(),
                        location: "header".into(),
                        required: false,
                        schema: Some(SchemaValue::new(SchemaKind::String)),
                        description: None,
                        annotations: BTreeMap::new(),
                    }],
                    responses: vec![json_response()],
                    annotations: BTreeMap::from([(
                        "x-poolster-pagination".into(),
                        serde_json::json!({
                            "type": "cursor",
                            "inputs": [{ "name": "X-Page-Cursor", "in": "parameters", "type": "cursor" }],
                            "outputs": { "nextCursor": "$.next" },
                        }),
                    )]),
                    ..Operation::default()
                },
                Operation {
                    id: "listByPathCursor".into(),
                    method: HttpMethod::Get,
                    path: "/cursor/{cursor}".into(),
                    parameters: vec![OperationParameter {
                        name: "cursor".into(),
                        location: "path".into(),
                        required: true,
                        schema: Some(SchemaValue::new(SchemaKind::String)),
                        description: None,
                        annotations: BTreeMap::new(),
                    }],
                    responses: vec![json_response()],
                    annotations: BTreeMap::from([(
                        "x-poolster-pagination".into(),
                        serde_json::json!({
                            "type": "cursor",
                            "inputs": [{ "name": "cursor", "in": "parameters", "type": "cursor" }],
                            "outputs": { "nextCursor": "$.next" },
                        }),
                    )]),
                    ..Operation::default()
                },
                // OpenAPI 3 requires path parameters to be required. Keep an
                // invalid AST shape out of generated paginator APIs too.
                Operation {
                    id: "invalidOptionalPathCursor".into(),
                    method: HttpMethod::Get,
                    path: "/invalid/{cursor}".into(),
                    parameters: vec![OperationParameter {
                        name: "cursor".into(),
                        location: "path".into(),
                        required: false,
                        schema: Some(SchemaValue::new(SchemaKind::String)),
                        description: None,
                        annotations: BTreeMap::new(),
                    }],
                    responses: vec![json_response()],
                    annotations: BTreeMap::from([(
                        "x-poolster-pagination".into(),
                        serde_json::json!({
                            "type": "cursor",
                            "inputs": [{ "name": "cursor", "in": "parameters", "type": "cursor" }],
                            "outputs": { "nextCursor": "$.next" },
                        }),
                    )]),
                    ..Operation::default()
                },
                Operation {
                    id: "unsafeBodyCursor".into(),
                    method: HttpMethod::Get,
                    path: "/unsafe".into(),
                    responses: vec![json_response()],
                    annotations: BTreeMap::from([(
                        "x-poolster-pagination".into(),
                        serde_json::json!({
                            "type": "cursor",
                            "inputs": [{ "name": "cursor", "in": "requestBody", "type": "cursor" }],
                            "outputs": { "nextCursor": "$.next" },
                        }),
                    )]),
                    ..Operation::default()
                },
            ],
            ..Api::default()
        };

        let client = RustReqwest
            .generate(&api, &RenderOptions::default())
            .unwrap();
        let source = rendered_source(&client);
        assert!(source.contains("pub fn list_by_cursor_pages"));
        assert!(source.contains("pub fn list_by_offset_pages"));
        assert!(source.contains("pub fn list_by_header_cursor_pages"));
        assert!(source.contains("input.x_page_cursor = Some(cursor.to_owned());"));
        assert!(source.contains("pub fn list_by_path_cursor_pages"));
        assert!(source.contains("input.cursor = cursor.to_owned();"));
        assert!(!source.contains("invalid_optional_path_cursor_pages"));
        assert!(source.contains("$.data[-1].nextCursor"));
        assert!(source.contains("futures_util::stream::try_unfold"));
        assert!(!source.contains("unsafe_body_cursor_pages"));
    }

    #[test]
    #[ignore = "builds a generated SDK in a nested Cargo workspace; run in release verification"]
    fn generated_retry_runtime_compiles_warning_free() {
        let api = Api {
            name: "Compile API".into(),
            version: "1.0.0".into(),
            schemas: vec![Schema::new(
                "Pet",
                SchemaValue::new(SchemaKind::Object {
                    fields: Vec::new(),
                    additional_properties: AdditionalProperties::Forbidden,
                }),
            )],
            operations: vec![
                Operation {
                    id: "getPet".into(),
                    method: HttpMethod::Get,
                    path: "/pets".into(),
                    responses: vec![OperationResponse {
                        status: "200".into(),
                        description: None,
                        media_types: vec![OperationMediaType {
                            content_type: "application/json".into(),
                            schema: Some(SchemaValue::reference("#/components/schemas/Pet")),
                        }],
                    }],
                    ..Operation::default()
                },
                Operation {
                    id: "listPets".into(),
                    method: HttpMethod::Get,
                    path: "/pets".into(),
                    parameters: vec![OperationParameter {
                        name: "cursor".into(),
                        location: "query".into(),
                        required: false,
                        schema: Some(SchemaValue::new(SchemaKind::String)),
                        description: None,
                        annotations: BTreeMap::new(),
                    }],
                    responses: vec![OperationResponse {
                        status: "200".into(),
                        description: None,
                        media_types: vec![OperationMediaType {
                            content_type: "application/json".into(),
                            schema: Some(SchemaValue::reference("#/components/schemas/Pet")),
                        }],
                    }],
                    annotations: BTreeMap::from([(
                        "x-poolster-pagination".into(),
                        serde_json::json!({
                            "type": "cursor",
                            "inputs": [{ "name": "cursor", "in": "parameters", "type": "cursor" }],
                            "outputs": { "nextCursor": "$.nextCursor" },
                        }),
                    )]),
                    ..Operation::default()
                },
                Operation {
                    id: "createPet".into(),
                    method: HttpMethod::Post,
                    path: "/pets".into(),
                    request_body: Some(poolster_core::OperationRequestBody::json(
                        SchemaValue::reference("#/components/schemas/Pet"),
                        true,
                    )),
                    parameters: vec![OperationParameter {
                        name: "Idempotency-Key".into(),
                        location: "header".into(),
                        required: false,
                        schema: Some(SchemaValue::new(SchemaKind::String)),
                        description: None,
                        annotations: BTreeMap::new(),
                    }],
                    responses: vec![OperationResponse {
                        status: "200".into(),
                        description: None,
                        media_types: vec![OperationMediaType {
                            content_type: "application/json".into(),
                            schema: Some(SchemaValue::reference("#/components/schemas/Pet")),
                        }],
                    }],
                    ..Operation::default()
                },
            ],
            ..Api::default()
        };
        let temp = tempfile::tempdir().unwrap();
        poolster_core::engine::Packages::new()
            .package(crate::package("sdk").with(crate::sdk()))
            .generate(&api, None)
            .unwrap()
            .write_to(temp.path())
            .unwrap();
        let sdk = temp.path().join("sdk");

        let manifest = sdk.join("Cargo.toml");
        let cargo_toml = fs::read_to_string(&manifest).unwrap();
        fs::write(
            &manifest,
            format!(
                "{cargo_toml}\n[dev-dependencies]\ntokio = {{ version = \"1\", features = [\"macros\", \"rt-multi-thread\"] }}\n"
            ),
        )
        .unwrap();
        let test_directory = sdk.join("tests");
        fs::create_dir_all(&test_directory).unwrap();
        fs::write(
            test_directory.join("retry_runtime.rs"),
            r#"
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{atomic::{AtomicUsize, Ordering}, Arc},
    thread,
};

use compile_api_sdk::{Client, ClientHooks, ListPetsRequest, RequestInfo, ResponseInfo, RetryConfig};
use futures_util::TryStreamExt;

struct Hooks(Arc<AtomicUsize>, Arc<AtomicUsize>);
impl ClientHooks for Hooks {
    fn before_request(&self, _: &RequestInfo) { self.0.fetch_add(1, Ordering::SeqCst); }
    fn after_response(&self, _: &ResponseInfo) { self.1.fetch_add(1, Ordering::SeqCst); }
}

#[test]
fn generated_paginator_is_a_usable_lazy_stream() {
    let client = Client::new("https://api.example.com");
    let mut pages = Box::pin(client.list_pets_pages(ListPetsRequest { cursor: None }));
    let _next_page = pages.as_mut().try_next();
}

#[tokio::test]
#[ignore = "opens a loopback listener; run explicitly where socket binding is permitted"]
async fn retries_safe_requests_and_only_hooks_the_final_outcome() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let worker = thread::spawn(move || {
        for attempt in 0..3 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).unwrap();
            let response = if attempt < 2 {
                "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            } else {
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}"
            };
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    let starts = Arc::new(AtomicUsize::new(0));
    let finishes = Arc::new(AtomicUsize::new(0));
    let client = Client::new(format!("http://{address}"))
        .with_retry(RetryConfig { max_attempts: 3, initial_delay: std::time::Duration::ZERO, max_delay: std::time::Duration::ZERO })
        .with_hooks(Arc::new(Hooks(starts.clone(), finishes.clone())));

    client.get_pet().await.unwrap();
    worker.join().unwrap();
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    assert_eq!(finishes.load(Ordering::SeqCst), 1);
}
"#,
        )
        .unwrap();

        let status = crate::native_cargo()
            .args(["test", "--quiet"])
            .current_dir(&sdk)
            .env("RUSTFLAGS", "-Dwarnings")
            .status()
            .expect("cargo should be available for generated SDK tests");
        assert!(
            status.success(),
            "generated Rust SDK must compile and retry safely"
        );
    }
}
