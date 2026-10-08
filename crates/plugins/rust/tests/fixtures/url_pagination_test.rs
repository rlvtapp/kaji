use url_sdk::{Client,client::ListLinksError,transport::{Transport,TransportFuture}};
use futures_util::StreamExt;
use std::sync::{Arc,Mutex};
struct Driver {seen:Arc<Mutex<Vec<String>>>,target:Option<String>,status:u16}
impl Transport for Driver {fn execute(&self,request:reqwest::Request)->TransportFuture<'_>{
 assert_eq!(request.headers().get("Authorization").unwrap(),"Bearer test");let url=request.url().to_string();self.seen.lock().unwrap().push(url.clone());
 let body=if self.status!=200 {"{\"reason\":\"bad\"}".into()}else if let Some(target)=&self.target {serde_json::json!({"next":target}).to_string()}else if url.contains("page=2") {"{}".into()}else {"{\"next\":\"https://api.example.test/links?page=2\"}".into()};let status=self.status;
 Box::pin(async move{Ok(http::Response::builder().status(status).body(reqwest::Body::from(body)).unwrap().into())})
}}
fn ready<T>(future:impl std::future::Future<Output=T>)->T {let w=futures_util::task::noop_waker();let mut cx=std::task::Context::from_waker(&w);let mut future=Box::pin(future);match future.as_mut().poll(&mut cx){std::task::Poll::Ready(value)=>value,_=>panic!("unexpected pending")}}
fn client(target:Option<String>,status:u16)->(Client,Arc<Mutex<Vec<String>>>) {let seen=Arc::new(Mutex::new(vec![]));(Client::new("https://api.example.test").with_bearer_token("test").with_transport(Arc::new(Driver{seen:seen.clone(),target,status})),seen)}
#[test] fn lazy_safe_and_bounded(){
 let (client,seen)=client(None,200);let mut pages=Box::pin(client.list_links_pages());assert!(seen.lock().unwrap().is_empty());assert!(ready(pages.next()).unwrap().is_ok());assert!(ready(pages.next()).unwrap().is_ok());assert!(ready(pages.next()).is_none());assert_eq!(*seen.lock().unwrap(),["https://api.example.test/links","https://api.example.test/links?page=2"]);
 for target in ["https://evil.example/links","http://api.example.test/links","https://api.example.test:444/links","https://user@api.example.test/links","https://api.example.test/links#fragment","/relative"] {let (client,seen)=self::client(Some(target.into()),200);let mut pages=Box::pin(client.list_links_pages());assert!(ready(pages.next()).unwrap().is_ok());assert!(matches!(ready(pages.next()).unwrap(),Err(ListLinksError::Pagination(_))));assert_eq!(seen.lock().unwrap().len(),1);}
 let (client,seen)=self::client(Some("https://api.example.test/links".into()),200);let mut pages=Box::pin(client.list_links_pages());assert!(ready(pages.next()).unwrap().is_ok());assert!(ready(pages.next()).unwrap().is_ok());assert!(ready(pages.next()).unwrap().is_err());assert_eq!(seen.lock().unwrap().len(),2);
}
#[test] fn typed_errors_keep_original_wire_bytes(){for status in [400,418]{let(client,_)=client(None,status);let error=ready(client.list_links()).unwrap_err();match error {ListLinksError::BadRequest{body,response}|ListLinksError::Default{body,response}=>{assert_eq!(body["reason"],"bad");assert_eq!(response.body,b"{\"reason\":\"bad\"}");},other=>panic!("{other:?}")}}}
