use client::*;
fn success<T>(response:GraphqlResponse<T>)->T {match response{GraphqlResponse::Success{data,..}=>data,_=>panic!("expected success")}}
#[tokio::main]
async fn main(){
 let endpoint=std::env::var("GRAPHQL_ENDPOINT").unwrap();
 let http=reqwest::Client::builder().timeout(std::time::Duration::from_secs(3)).build().unwrap();
 let transport=GraphqlHttpTransport::new(&endpoint,http.clone());
 let scalar_variables=ScalarsVariables{value:"wire-time".into(),input:ScalarInput{required:"nested-time".into(),values:vec![Some("list-time".into()),None],optional:Presence::Null},optional:Presence::Absent,include:false};
 let mapped=success(scalars(&transport,&scalar_variables).await.unwrap()).scalars;
 let timestamp:i64=mapped.timestamp;assert_eq!(timestamp,1700000000);
 let values:Vec<Option<i64>>=mapped.values;assert_eq!(values,vec![Some(1700000001),None]);
 assert_eq!(mapped.nullable,None);assert!(matches!(mapped.optional,Presence::Absent));assert_eq!(mapped.raw["retained"][1],42);
 let mapped=success(scalars(&transport,&ScalarsVariables{include:true,..scalar_variables.clone()}).await.unwrap()).scalars;
 assert!(matches!(mapped.optional,Presence::Value(1700000002)));
 let malformed=GraphqlHttpTransport::new(format!("{endpoint}/bad-scalar"),http.clone());
 assert!(matches!(scalars(&malformed,&scalar_variables).await.unwrap_err(),GraphqlTransportError::Decode(_)));
 let abstract_data=success(abstract_node(&transport,&AbstractNodeVariables{}).await.unwrap());
 assert_eq!(serde_json::to_value(abstract_data).unwrap()["node"]["code"],"retained-second-variant");
 let data=success(read(&transport,&ReadVariables{id:"7".into()}).await.unwrap());
 assert_eq!(data.person.id,"7");assert_eq!(data.person.name,"Ada");assert_eq!(data.person.nickname,None);
 assert_eq!(success(rename(&transport,&RenameVariables{name:"Grace".into()}).await.unwrap()).rename.name,"Grace");
 match partial(&transport,&PartialVariables{id:"7".into()}).await.unwrap(){GraphqlResponse::Partial{data,errors,..}=>{assert_eq!(data.user.fragile,None);assert_eq!(errors[0].path.as_ref().unwrap()[1],"fragile");},_=>panic!("expected partial")}
 assert!(matches!(fatal(&transport,&FatalVariables{}).await.unwrap(),GraphqlResponse::Error{..}));
 for (value,expected) in [(Presence::Absent,Some("argument-default")),(Presence::Null,None),(Presence::Value("wire".into()),Some("wire"))]{assert_eq!(success(presence_query(&transport,&PresenceQueryVariables{value}).await.unwrap()).value.as_deref(),expected);}
 for (options,expected) in [(Presence::Absent,Some("options-omitted")),(Presence::Null,Some("options-null")),(Presence::Value(Options{note:Presence::Absent}),Some("input-default")),(Presence::Value(Options{note:Presence::Null}),None),(Presence::Value(Options{note:Presence::Value("present".into())}),Some("present"))]{assert_eq!(success(input_presence(&transport,&InputPresenceVariables{options}).await.unwrap()).value.as_deref(),expected);}
 let omitted=success(conditional(&transport,&ConditionalVariables{include:false}).await.unwrap());assert!(matches!(omitted.user.name,Optional::Absent));assert!(matches!(omitted.user.nickname,Presence::Absent));
 let included=success(conditional(&transport,&ConditionalVariables{include:true}).await.unwrap());assert!(matches!(included.user.name,Optional::Value(_)));assert!(matches!(included.user.nickname,Presence::Null));
 let defaulted=success(defaulted(&transport,&DefaultedVariables{include:Optional::Absent}).await.unwrap());assert!(matches!(defaulted.user.name,Optional::Absent));
 for (route,kind) in [("/http-error",0),("/bad-json",1),("/bad-envelope",1),("/missing-nullable",2),("/disconnect",3)]{
  let transport=GraphqlHttpTransport::new(format!("{endpoint}{route}"),http.clone());let error=read(&transport,&ReadVariables{id:"7".into()}).await.unwrap_err();
  assert!(match kind{0=>matches!(error,GraphqlTransportError::Http{status:503,..}),1=>matches!(error,GraphqlTransportError::Protocol(_)),2=>matches!(error,GraphqlTransportError::Decode(_)),_=>matches!(error,GraphqlTransportError::Network(_))});
 }
 let transport=GraphqlHttpTransport::new(format!("{endpoint}/null-nonnull"),http);
 assert!(matches!(conditional(&transport,&ConditionalVariables{include:true}).await.unwrap_err(),GraphqlTransportError::Decode(_)));
 println!("packaged Rust GraphQL runtime passed");
}
