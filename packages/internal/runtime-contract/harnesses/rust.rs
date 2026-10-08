use contractsdk::{Client, client::{CreateContactRequest, PatchContactRequest, ListContactsRequest, EchoWireRequest}, transport::{Middleware, MiddlewareTransport, DefaultTransport, Transport, TransportFuture}};
use futures_util::StreamExt;
use contractsdk::WireInput;
struct Policy;
impl Middleware for Policy {
    fn handle<'a>(&'a self, mut request: reqwest::Request, next: &'a dyn Transport) -> TransportFuture<'a> {
        request.headers_mut().insert("x-contract-middleware", reqwest::header::HeaderValue::from_static("yes"));
        next.execute(request)
    }
}
#[tokio::main]
async fn main() {
    let scenario: serde_json::Value = serde_json::from_str(&std::env::var("POOLSTER_CONTRACT_SCENARIO").unwrap_or_else(|_| "{}".into())).unwrap();
    let transport = MiddlewareTransport::new(Policy, DefaultTransport::default());
    let client = Client::new(std::env::var("POOLSTER_CONTRACT_URL").unwrap())
        .with_bearer_token(std::env::var("POOLSTER_CONTRACT_CASE").unwrap())
        .with_transport(std::sync::Arc::new(transport));
    let action = scenario["action"].as_str().unwrap_or("getContact");
    let key = scenario["caller_key"].as_str().map(str::to_owned);
    let mut outcome: Result<String, String> = Ok(String::new());
    for _ in 0..scenario["repeats"].as_u64().unwrap_or(1) {
        outcome = match action {
            "createContact" => client.create_contact(CreateContactRequest { x_once: key.clone() }).await.map(|model| model.id).map_err(|_| "error".into()),
            "patchContact" => client.patch_contact(PatchContactRequest { x_once: key.clone() }).await.map(|model| model.id).map_err(|_| "error".into()),
            "unsafeCreateContact" => client.unsafe_create_contact().await.map(|model| model.id).map_err(|_| "error".into()),
            "unsafePatchContact" => client.unsafe_patch_contact().await.map(|model| model.id).map_err(|_| "error".into()),
            "listContactsPages" => {
                let pages = client.list_contacts_pages(ListContactsRequest { page: Some(scenario["page"].as_i64().unwrap_or(1)), limit: Some(scenario["limit"].as_i64().unwrap_or(2)) });
                let mut pages = Box::pin(pages);
                let mut ids = Vec::new();
                let mut failure = false;
                while let Some(page) = pages.next().await {
                    match page { Ok(page) => ids.extend(page.items.into_iter().map(|item| item.id)), Err(_) => { failure = true; break; } }
                }
                if failure { Err("error".into()) } else { Ok(ids.join(",")) }
            }
            "echoWire" => client.echo_wire(EchoWireRequest {
                key: "café/雪".into(), text: Some("héllo 雪".into()), flag: Some(false), count: Some(0), tags: Some(vec!["a".into(), "b".into()]), x_label: Some("caller".into())
            }, &WireInput { enabled: false, count: 0, note: None, missing: None }).await.map(|model| model.id).map_err(|_| "error".into()),
            "getContact" => client.get_contact().await.map(|model| model.id).map_err(|_| "error".into()),
            _ => panic!("unimplemented runtime action {action}"),
        };
        if outcome.is_err() { break; }
    }
    let result = match outcome { Ok(id) => serde_json::json!({"outcome":"success","id":id}), Err(_) => serde_json::json!({"outcome":"error"}) };
    println!("{result}");
}
