use super::*;
use crate::PackageExt;
use poolster_core::{engine::Packages, native::GraphqlOperation};
fn ty(name: &str, nullable: bool) -> ModelType {
    ModelType {
        nullable,
        kind: ModelKind::Scalar(name.into()),
    }
}
fn field(name: &str, ty: ModelType, optional: bool) -> ModelField {
    ModelField {
        name: name.into(),
        ty,
        optional,
        default_value: None,
    }
}
fn input() -> GraphqlOperations {
    GraphqlOperations{schema_source:"type Query { user(id:ID!, show:Boolean, label:String):User! } type User {name:String! nickname:String} type Mutation {rename(name:String!):String!}".into(),operation_source:String::new(),input_objects:BTreeMap::new(),operations:vec![GraphqlOperation{name:"ReadUser".into(),kind:GraphqlOperationKind::Query,document:"query ReadUser($id:ID!, $show:Boolean = true, $label:String) {user(id:$id,show:$show,label:$label){name nickname}}".into(),variables:vec![field("id",ty("ID",false),false),field("show",ty("Boolean",true),true),field("label",ty("String",true),true)],result:ModelType{nullable:false,kind:ModelKind::Object(vec![])}},GraphqlOperation{name:"Rename".into(),kind:GraphqlOperationKind::Mutation,document:"mutation Rename($name:String!) {rename(name:$name)}".into(),variables:vec![field("name",ty("String",false),false)],result:ModelType{nullable:false,kind:ModelKind::Object(vec![])}}]}
}
struct Source {
    meta: Meta,
    value: GraphqlOperations,
}
impl Plugin<Postman> for Source {
    fn kind(&self) -> &'static str {
        "graphql-test-source"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<GraphqlOperations>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Postman>) -> Result<()> {
        cx.publish(self.value.clone())
    }
}
fn generate(value: GraphqlOperations, strict: bool) -> Result<poolster_core::GeneratedTree> {
    let source = Source {
        meta: Meta::new(),
        value,
    };
    let output = graphql()
        .input(source.meta.handle())
        .strict(strict)
        .variables("ReadUser", json!({"id":"1","label":null}));
    let env = graphql_environment().using_collection(output.handle());
    Packages::new()
        .package(
            super::super::package("postman")
                .base_url("http://localhost:9999/graphql")
                .with(source)
                .with(output)
                .with(env),
        )
        .generate_native()
}
#[test]
fn native_collection_environment_and_variables() {
    let tree = generate(input(), true).unwrap();
    let doc: Value = serde_json::from_str(tree.get("postman/collection.json").unwrap()).unwrap();
    let env: Value = serde_json::from_str(tree.get("postman/environment.json").unwrap()).unwrap();
    assert_eq!(doc["item"][0]["name"], "Mutation");
    let req = &doc["item"][1]["item"][0]["request"];
    assert_eq!(req["body"]["mode"], "graphql");
    assert_eq!(req["body"]["graphql"]["operationName"], "ReadUser");
    let vars = doc["variable"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| {
            v["key"].as_str().unwrap().starts_with("graphql_variables_")
                && v["value"].as_str().unwrap().contains("label")
        })
        .unwrap();
    let vars: Value = serde_json::from_str(vars["value"].as_str().unwrap()).unwrap();
    assert_eq!(vars, json!({"id":"1","label":null}));
    assert!(!vars.as_object().unwrap().contains_key("show"));
    assert_eq!(
        env["values"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["key"] == "base_url")
            .unwrap()["value"],
        "http://localhost:9999/graphql"
    );
}
#[test]
fn subscriptions_strict_and_reported_skip() {
    let mut value = input();
    let mut subscription = value.operations[0].clone();
    subscription.kind = GraphqlOperationKind::Subscription;
    subscription.name = "WatchUser".into();
    value.operations.push(subscription);
    assert!(format!("{:#}", generate(value.clone(), true).unwrap_err()).contains("Subscriptions"));
    let tree = generate(value, false).unwrap();
    let diag: Value = serde_json::from_str(tree.get("postman/diagnostics.json").unwrap()).unwrap();
    assert_eq!(diag[0]["code"], "unsupported_subscription");
}
#[test]
fn configured_variables_validate_and_custom_scalars_need_configuration() {
    let mut value = input();
    value.operations[0].variables[0].ty = ty("Opaque", false);
    let settings = super::super::Settings::default();
    let generated = collection(&value, &settings, true, &BTreeMap::new()).unwrap();
    assert_eq!(
        generated.diagnostics[0].code,
        "variables_require_configuration"
    );
    let overrides = BTreeMap::from([("ReadUser".into(), json!({"id":{"opaque":true}}))]);
    assert!(
        collection(&value, &settings, true, &overrides)
            .unwrap()
            .diagnostics
            .is_empty()
    );
    let value = input();
    assert!(
        collection(
            &value,
            &settings,
            false,
            &BTreeMap::from([("ReadUser".into(), json!({"id":null}))])
        )
        .is_err()
    );
    assert!(
        collection(
            &value,
            &settings,
            false,
            &BTreeMap::from([("ReadUser".into(), json!({"id":"1","unknown":true}))])
        )
        .is_err()
    );
}
#[test]
#[ignore = "requires Python jsonschema validator for pinned official Postman2.1 draft04 schema"]
fn validates_pinned_schema() {
    let tree = generate(input(), true).unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let python = std::env::var("POOLSTER_TEST_PYTHON").unwrap_or("python3".into());
    let output=std::process::Command::new(python).args(["-c","import json,sys,jsonschema; schema=json.load(open(sys.argv[1])); jsonschema.Draft4Validator.check_schema(schema); jsonschema.Draft4Validator(schema).validate(json.load(open(sys.argv[2])))"]).arg(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/schema/collection-v2.1.0.json")).arg(root.path().join("postman/collection.json")).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
#[ignore = "requires pinned Newman6.2.1+GraphQL.js16.14.2 and local HTTP server"]
fn newman_executes_native_graphql_body() {
    let tree = generate(input(), true).unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let output =
        std::process::Command::new(std::env::var("POOLSTER_TEST_NODE").unwrap_or("node".into()))
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/graphql/runtime.cjs"
            ))
            .arg(root.path().join("postman/collection.json"))
            .arg(root.path().join("postman/environment.json"))
            .output()
            .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Newman GraphQL passed"));
}

#[test]
fn deterministic_splits_privacy_and_empty_subscription_collection() {
    let mut source = input();
    source.operations[0]
        .variables
        .push(field("token", ty("String", true), true));
    source.operations[0].variables[0].default_value = Some("schema-secret".into());
    let settings = super::super::Settings::default();
    let configured = BTreeMap::from([("ReadUser".into(), json!({"id":"1","token":"user-secret"}))]);
    let first = collection(&source, &settings, true, &configured).unwrap();
    let second = collection(&source, &settings, true, &configured).unwrap();
    assert_eq!(first.document, second.document);
    let serialized = serde_json::to_string(&first.document).unwrap();
    assert!(!serialized.contains("user-secret"));
    assert!(!serialized.contains("schema-secret"));
    assert!(serialized.contains("<redacted>"));
    let splits = render::split_collections(&first.document).unwrap();
    assert_eq!(splits.len(), 2);
    for (_, split) in splits {
        assert_eq!(split["event"], first.document["event"]);
        assert_eq!(split["variable"], first.document["variable"]);
    }
    source.operations.truncate(1);
    source.operations[0].kind = GraphqlOperationKind::Subscription;
    let empty = collection(&source, &settings, true, &BTreeMap::new()).unwrap();
    assert_eq!(empty.document["item"], json!([]));
    assert_eq!(empty.diagnostics[0].code, "unsupported_subscription");
    source.operations.clear();
    assert!(collection(&source, &settings, true, &BTreeMap::new()).is_err());
}

#[test]
fn native_regeneration_preserves_customer_environment() {
    let first = generate(input(), true).unwrap();
    let second = generate(input(), true).unwrap();
    assert_eq!(
        first.get("postman/collection.json"),
        second.get("postman/collection.json")
    );
    let root = tempfile::tempdir().unwrap();
    first.write_to(root.path()).unwrap();
    let environment = root.path().join("postman/environment.json");
    std::fs::write(&environment, "customer environment").unwrap();
    second.write_to(root.path()).unwrap();
    assert_eq!(
        std::fs::read_to_string(environment).unwrap(),
        "customer environment"
    );
}
