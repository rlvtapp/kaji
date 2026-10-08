#[cfg(test)]mod multipart_native_probe {
 use super::*;use crate::transport::{Transport,TransportFuture};
 struct Driver(std::sync::Mutex<Vec<(String,Vec<u8>)>>);
 impl Transport for Driver {fn execute(&self,request:reqwest::Request)->TransportFuture<'_>{let bytes=request.body().unwrap().as_bytes().unwrap().to_vec();let content=request.headers()["content-type"].to_str().unwrap().to_owned();let mut seen=self.0.lock().unwrap();seen.push((content,bytes));let status=if seen.len()==1{503}else{204};Box::pin(async move{Ok(http::Response::builder().status(status).body(reqwest::Body::from("")).unwrap().into())})}}
 #[tokio::test]async fn wire_parts_and_retry_replay(){
 let mut body=MultipartBody::new();body.add_text("flag","false").unwrap().add_text("count","0").unwrap().add_file("file","café.bin","application/octet-stream",[0,255,13,10]).unwrap().add_json("strategy",&serde_json::json!({"type":"auto","name":"雪"})).unwrap().add_text("tags[]","a").unwrap().add_text("tags[]","b").unwrap();
 assert!(body.add_text("bad\r\nname","no").is_err());
 let driver=std::sync::Arc::new(Driver(std::sync::Mutex::new(Vec::new())));let client=Client::new("https://api.test").with_transport(driver.clone()).with_retry(RetryConfig{max_attempts:2,initial_delay:std::time::Duration::ZERO,max_delay:std::time::Duration::ZERO});client.put_upload(&body).await.unwrap();
 let seen=driver.0.lock().unwrap();assert_eq!(seen.len(),2);assert_eq!(seen[0],seen[1]);let text=String::from_utf8_lossy(&seen[0].1);assert!(text.contains("filename=\"café.bin\""));assert!(text.contains("Content-Type: application/json"));assert!(text.contains("雪"));assert_eq!(text.matches("name=\"tags[]\"").count(),2);assert!(seen[0].1.windows(4).any(|w|w==[0,255,13,10]));let boundary=seen[0].0.split("boundary=").nth(1).unwrap();assert!(text.ends_with(&format!("--{boundary}--\r\n")));drop(seen);
 let mixed_driver=std::sync::Arc::new(Driver(std::sync::Mutex::new(Vec::new())));let mixed_client=Client::new("https://api.test").with_transport(mixed_driver.clone()).with_retry(RetryConfig{max_attempts:2,initial_delay:std::time::Duration::ZERO,max_delay:std::time::Duration::ZERO});mixed_client.mixed_upload_multipart_body(&body).await.unwrap();mixed_client.upload().mixed_multipart(&body).await.unwrap();mixed_client.mixed_upload(&serde_json::json!({"hello":"snow"})).await.unwrap();let mixed_seen=mixed_driver.0.lock().unwrap();assert!(mixed_seen[0].0.starts_with("multipart/form-data"));assert!(mixed_seen[2].0.starts_with("multipart/form-data"));assert_eq!(mixed_seen[3].0,"application/json");drop(mixed_seen);
 let unsafe_driver=std::sync::Arc::new(Driver(std::sync::Mutex::new(Vec::new())));assert!(client.with_transport(unsafe_driver.clone()).post_upload(&body).await.is_err());assert_eq!(unsafe_driver.0.lock().unwrap().len(),1);
 }
}

#[cfg(test)] mod oas_wire_probe {
 use super::*;use crate::transport::{Transport,TransportFuture};
 struct SequenceDriver;
 impl Transport for SequenceDriver {fn execute(&self,request:reqwest::Request)->TransportFuture<'_>{
  assert_eq!(request.url().query(),Some("zero=0&false=false&name=%E9%9B%AA"));assert_eq!(request.headers()["content-type"],"application/x-ndjson");
  let bytes=request.body().unwrap().as_bytes().unwrap().to_vec();let value:serde_json::Value=poolster_decode_json(&bytes,"application/x-ndjson").unwrap();assert_eq!(value,serde_json::json!([false,0,null,{"future":"雪"}]));
  Box::pin(async move{Ok(http::Response::builder().status(200).header("content-type","application/x-ndjson").body(reqwest::Body::from(bytes)).unwrap().into())})
 }}
 #[tokio::test]async fn sequence_operation_wire(){let records=serde_json::json!([false,0,null,{"future":"雪"}]).as_array().unwrap().clone();let client=Client::new("https://api.test").with_transport(std::sync::Arc::new(SequenceDriver));let result=client.sequence(crate::client::operations::SequenceRequest{whole_query:"zero=0&false=false&name=%E9%9B%AA".into()},&records).await.unwrap();assert_eq!(result,records);}
 #[test]fn nested_positional_plan(){
  let mut inner=MultipartBody::new();inner.add_text("child","false").unwrap();let mut outer=MultipartBody::new();outer.add_json("metadata",&serde_json::json!({"future":"雪"})).unwrap().add_nested("nested",inner).unwrap().add_file("file","blob.bin","application/octet-stream",[0,255]).unwrap();
  let plan=serde_json::json!({"content_type":"multipart/mixed","prefix_encoding":[{"contentType":"application/json","headers":{"X-Part":{"required":true,"example_json":"\"v1\""}}},{"contentType":"multipart/mixed","prefixEncoding":[{"contentType":"text/plain","headers":{"X-Child":{"required":true,"example_json":"\"child\""}}}]}],"item_encoding":{"contentType":"application/octet-stream"}});
  let(media,bytes)=outer.encoded_with_plan(&plan).unwrap();assert!(media.starts_with("multipart/mixed"));let text=String::from_utf8_lossy(&bytes);assert!(text.contains("X-Part: v1"));assert!(text.contains("X-Child: child"));assert!(text.contains("Content-Disposition: attachment"));assert!(bytes.windows(2).any(|part|part==[0,255]));assert_eq!(outer.encoded_with_plan(&plan).unwrap(),(media,bytes));
 }
}

#[cfg(test)]mod content_parameter_probe {
 use super::*;use crate::transport::{Transport,TransportFuture};
 struct Driver;
 impl Transport for Driver {fn execute(&self,request:reqwest::Request)->TransportFuture<'_>{assert_eq!(request.url().path(),"/params/%22hello%20world%22");assert_eq!(request.url().query(),Some("filter=%22query%22"));assert_eq!(request.headers()["x-json"],"\"header\"");assert_eq!(request.headers()["cookie"],"cookie=%22cookie%22");Box::pin(async{Ok(http::Response::builder().status(204).body(reqwest::Body::from("")).unwrap().into())})}}
 #[tokio::test]async fn typed_json_parameters(){let client=Client::new("https://api.test").with_transport(std::sync::Arc::new(Driver));client.json_parameters(crate::client::operations::JsonParametersRequest{path:"hello world".into(),filter:"query".into(),x_json:"header".into(),cookie:"cookie".into()}).await.unwrap();}
}
