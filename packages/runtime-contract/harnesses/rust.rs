use contractsdk::{Client, transport::{Middleware,MiddlewareTransport,DefaultTransport,Transport,TransportFuture}};
struct Policy;
impl Middleware for Policy {
    fn handle<'a>(&'a self, mut request:reqwest::Request,next:&'a dyn Transport)->TransportFuture<'a>{
        request.headers_mut().insert("x-contract-middleware",reqwest::header::HeaderValue::from_static("yes"));
        next.execute(request)
    }
}
#[tokio::main]
async fn main(){
    let transport=MiddlewareTransport::new(Policy,DefaultTransport::default());
    let client=Client::new(std::env::var("KAJI_CONTRACT_URL").unwrap()).with_bearer_token(std::env::var("KAJI_CONTRACT_CASE").unwrap()).with_transport(std::sync::Arc::new(transport));
    let result=match client.get_contact().await{Ok(model)=>serde_json::json!({"outcome":"success","id":model.id}),Err(_)=>serde_json::json!({"outcome":"error"})};
    println!("{result}");
}
