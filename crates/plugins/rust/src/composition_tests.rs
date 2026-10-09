use super::*;
use poolster_core::{
    Api, HttpMethod, Operation, OperationResponse, Schema, SchemaKind, SchemaValue,
    engine::Packages,
};
struct CustomTransport {
    meta: Meta,
}
impl Plugin<Rust> for CustomTransport {
    fn kind(&self) -> &'static str {
        "custom-rust-http"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<Transport>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Rust>) -> Result<()> {
        cx.files.emit(GeneratedFile::new("src/custom_http.rs",r#"use std::{future::Future,pin::Pin};
pub type TransportFuture<'a>=Pin<Box<dyn Future<Output=Result<reqwest::Response,reqwest::Error>>+Send+'a>>;
pub trait Transport:Send+Sync {fn execute(&self,request:reqwest::Request)->TransportFuture<'_>;}
#[derive(Default)] pub struct CustomHTTP;
impl Transport for CustomHTTP {fn execute(&self,request:reqwest::Request)->TransportFuture<'_>{Box::pin(async move {assert_eq!(request.url().path(),"/label");panic!("custom transport invoked")})}}
"#)?)?;
        cx.publish(Transport {
            module: "crate::custom_http".into(),
            constructor: "crate::custom_http::CustomHTTP".into(),
        })
    }
}
#[test]
fn provider_contracts_generate_replaceable_transport_and_tests() {
    let mut integer = SchemaValue::new(SchemaKind::Integer);
    integer.format = Some("int64".into());
    let api = Api {
        name: "demo".into(),
        version: "1.0.0".into(),
        schemas: vec![
            Schema::new(
                "Details",
                SchemaValue::new(SchemaKind::Object {
                    fields: vec![poolster_core::Field {
                        name: "note".into(),
                        value: {
                            let mut value = SchemaValue::new(SchemaKind::String);
                            value.nullable = true;
                            value
                        },
                        required: false,
                        annotations: Default::default(),
                    }],
                    additional_properties: poolster_core::AdditionalProperties::Any,
                }),
            ),
            Schema::new("Counter", integer),
            Schema::new("Label", SchemaValue::new(SchemaKind::String)),
        ],
        operations: vec![Operation {
            id: "getLabel".into(),
            method: HttpMethod::Get,
            path: "/label".into(),
            responses: vec![OperationResponse::json(
                "200",
                SchemaValue::new(SchemaKind::String),
            )],
            ..Default::default()
        }],
        ..Default::default()
    };
    let model = models();
    let mh = model.models_handle();
    let transport = CustomTransport { meta: Meta::new() };
    let th = transport.meta.handle::<Transport>();
    let operation = operations().using_models(mh).using_transport(th);
    let oh = operation.operations_handle();
    let tree = Packages::new()
        .package(
            crate::package("sdk")
                .with(client().using_operations(oh))
                .with(operation)
                .with(transport)
                .with(model)
                .with(roundtrip_tests().using_models(mh)),
        )
        .generate(&api, None)
        .unwrap();
    assert!(
        tree.get("sdk/src/client/mod.rs")
            .unwrap()
            .contains("Arc<dyn crate::custom_http::Transport>")
    );
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    std::fs::create_dir(root.path().join("sdk/tests")).unwrap();
    std::fs::write(root.path().join("sdk/tests/custom.rs"),r#"#[test]
#[should_panic(expected="custom transport invoked")]
fn custom_executor_runs(){use std::future::Future;let client=demo_sdk::Client::new("https://unused.example");let mut future=Box::pin(client.get_label());let _=future.as_mut().poll(&mut std::task::Context::from_waker(std::task::Waker::noop()));}
"#).unwrap();
    let output = crate::native_cargo()
        .args(["test", "--quiet"])
        .env("RUSTFLAGS", "-Dwarnings")
        .current_dir(root.path().join("sdk"))
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
fn generated_middleware_composes_and_short_circuits() {
    let api = Api {
        name: "middleware".into(),
        version: "1.0.0".into(),
        operations: vec![Operation {
            id: "getLabel".into(),
            method: HttpMethod::Get,
            path: "/label".into(),
            responses: vec![OperationResponse::json(
                "200",
                SchemaValue::new(SchemaKind::String),
            )],
            ..Default::default()
        }],
        ..Default::default()
    };
    let tree = Packages::new()
            .package(crate::package("sdk").with(crate::sdk()).middleware(poolster_core::customization::BundledMiddleware {
                path: "src/author_policy.rs".into(), symbol: "author_policy".into(), async_symbol: None,
                contents: r#"use crate::transport::{Middleware, Transport, TransportFuture};
pub fn author_policy() -> impl Middleware { Policy }
struct Policy;
impl Middleware for Policy {
    fn handle<'a>(&'a self, _: reqwest::Request, _: &'a dyn Transport) -> TransportFuture<'a> {
        Box::pin(async { Ok(http::Response::builder().status(200).header("content-type", "application/json").body(reqwest::Body::from("\"cached\"")).unwrap().into()) })
    }
}"#.into(),
            }))
            .generate(&api, None)
            .unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let sdk = root.path().join("sdk");
    let manifest = sdk.join("Cargo.toml");
    let mut cargo = std::fs::read_to_string(&manifest).unwrap();
    cargo.push_str("\n[dependencies.http]\nversion = \"1\"\n");
    std::fs::write(manifest, cargo).unwrap();
    std::fs::create_dir(sdk.join("tests")).unwrap();
    std::fs::write(sdk.join("tests/middleware.rs"), r#"
use middleware_sdk::transport::{Middleware, MiddlewareTransport, Transport, TransportFuture};
use std::{future::Future, sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}}, task::{Context, Poll, Waker}};
fn ready<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    match future.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value, Poll::Pending => panic!("test transport must be immediately ready"),
    }
}
fn response() -> reqwest::Response {
    http::Response::builder().status(200).header("content-type", "application/json")
        .body(reqwest::Body::from("\"cached\"")).unwrap().into()
}
struct Terminal { calls: Arc<AtomicUsize>, fail: bool }
impl Transport for Terminal {
    fn execute(&self, request: reqwest::Request) -> TransportFuture<'_> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(request.headers()["x-outer"], "yes");
        assert_eq!(request.headers()["x-inner"], "yes");
        Box::pin(async move {
            if self.fail { Err(reqwest::Client::new().get("invalid url").build().unwrap_err()) }
            else { Ok(response()) }
        })
    }
}
struct Layer { name: &'static str, events: Arc<Mutex<Vec<String>>>, recover: bool }
impl Middleware for Layer {
    fn handle<'a>(&'a self, mut request: reqwest::Request, next: &'a dyn Transport) -> TransportFuture<'a> {
        Box::pin(async move {
            self.events.lock().unwrap().push(format!("{}:request", self.name));
            request.headers_mut().insert(format!("x-{}", self.name).parse::<reqwest::header::HeaderName>().unwrap(), "yes".parse().unwrap());
            let result = next.execute(request).await;
            self.events.lock().unwrap().push(format!("{}:{}", self.name, if result.is_ok() {"response"} else {"error"}));
            let mut response = match result { Ok(r) => r, Err(_) if self.recover => response(), Err(e) => return Err(e) };
            response.headers_mut().insert("x-middleware", self.name.parse().unwrap());
            Ok(response)
        })
    }
}
struct Cache;
impl Middleware for Cache {
    fn handle<'a>(&'a self, _: reqwest::Request, _: &'a dyn Transport) -> TransportFuture<'a> {
        Box::pin(async { Ok(response()) })
    }
}
fn chain(fail: bool, recover: bool) -> (impl Transport, Arc<AtomicUsize>, Arc<Mutex<Vec<String>>>) {
    let calls = Arc::new(AtomicUsize::new(0)); let events = Arc::new(Mutex::new(vec![]));
    let inner = MiddlewareTransport::new(Layer {name:"inner",events:events.clone(),recover}, Terminal {calls:calls.clone(),fail});
    let transport = MiddlewareTransport::new(Layer {name:"outer",events:events.clone(),recover:false}, inner);
    (transport, calls, events)
}
#[test]
fn request_response_order_and_mutation() {
    let (transport, calls, events) = chain(false, false);
    let response = ready(transport.execute(reqwest::Request::new(reqwest::Method::GET, "https://unused.example/label".parse().unwrap()))).unwrap();
    assert_eq!(response.headers()["x-middleware"], "outer"); assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(*events.lock().unwrap(), ["outer:request", "inner:request", "inner:response", "outer:response"]);
}
#[test]
fn errors_propagate_or_recover() {
    let (transport, _, events) = chain(true, false);
    assert!(ready(transport.execute(reqwest::Request::new(reqwest::Method::GET, "https://unused.example".parse().unwrap()))).is_err());
    assert_eq!(*events.lock().unwrap(), ["outer:request", "inner:request", "inner:error", "outer:error"]);
    let (transport, _, events) = chain(true, true);
    assert!(ready(transport.execute(reqwest::Request::new(reqwest::Method::GET, "https://unused.example".parse().unwrap()))).is_ok());
    assert_eq!(*events.lock().unwrap(), ["outer:request", "inner:request", "inner:error", "outer:response"]);
}
#[test]
fn author_policy_is_enabled_without_consumer_registration() {
    let client = middleware_sdk::Client::new("https://unused.example");
    assert_eq!(ready(client.get_label()).unwrap(), "cached");
}
#[test]
fn short_circuit_is_used_by_generated_operation() {
    let calls = Arc::new(AtomicUsize::new(0));
    let terminal = Terminal {calls:calls.clone(),fail:false};
    let transport: Arc<dyn Transport> = Arc::new(MiddlewareTransport::new(Cache, terminal));
    let client = middleware_sdk::Client::new("https://unused.example").with_transport(transport);
    assert_eq!(ready(client.get_label()).unwrap(), "cached"); assert_eq!(calls.load(Ordering::SeqCst), 0);
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
