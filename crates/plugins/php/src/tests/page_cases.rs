use super::*;

#[test]
fn page_pagination_preserves_native_facades_and_reports_invalid_controls() {
    let mut source = page_fixture();
    let tree = render_sdk(&source, "php", None, SdkClientStyle::Namespaced).unwrap();
    let operations = tree.get("php/src/ClientOperations000.php").unwrap();
    assert!(operations.contains(
        "public function listPetsPages(?int $page = null, ?int $limit = null): \\Generator"
    ));
    assert!(operations.contains("$_poolsterPage = $page ?? 1"));
    assert!(operations.contains("$_poolsterPage++"));
    assert!(
        tree.get("php/src/Resources/PetsResourceOperations000.php")
            .unwrap()
            .contains("listPetsPages")
    );
    let helpers = tree.get("php/src/Client.php").unwrap();
    assert!(helpers.contains("0|[1-9][0-9]*"));
    assert!(helpers.contains("array_is_list"));
    source.operations[0].parameters[0].schema = Some(SchemaValue::new(SchemaKind::String));
    let invalid = render_sdk(&source, "php", None, SdkClientStyle::Flat).unwrap();
    assert!(
        invalid
            .get("php/.poolster/pagination-diagnostics.json")
            .unwrap()
            .contains("unsupported")
    );
    assert!(
        !invalid
            .get("php/src/ClientOperations000.php")
            .unwrap()
            .contains("listPetsPages")
    );
}
#[test]
#[ignore = "requires PHP 8.2+; executes generated pages against an in-memory operation"]
fn page_pagination_executes_native_query_body_and_selectors() {
    let mut source = page_fixture();
    let query =
        page_pagination::render(&source, &source.operations[0], &Default::default()).unwrap();
    source.operations[0].parameters = vec![OperationParameter {
        name: "page".into(),
        location: "query".into(),
        required: false,
        schema: Some(SchemaValue::new(SchemaKind::Integer)),
        description: None,
        annotations: BTreeMap::new(),
    }];
    source.operations[0].request_body = Some(OperationRequestBody::json(
        SchemaValue::new(SchemaKind::Object {
            fields: [("page", false), ("limit", true)]
                .iter()
                .map(|(name, required)| Field {
                    name: (*name).into(),
                    required: *required,
                    annotations: BTreeMap::new(),
                    value: SchemaValue::new(SchemaKind::Integer),
                })
                .collect(),
            additional_properties: AdditionalProperties::Any,
        }),
        true,
    ));
    source.operations[0].annotations.insert("x-poolster-pagination".into(),serde_json::json!({"type":"page","inputs":[{"name":"page","in":"requestBody","type":"page"},{"name":"limit","in":"requestBody","type":"limit"}],"outputs":{"results":"/items"}}));
    let body =
        page_pagination::render(&source, &source.operations[0], &Default::default()).unwrap();
    assert!(body.contains("$_poolsterBody"));
    let script = format!(
        "<?php\nclass Query {{ public array $seen=[]; public function listPets(?int $page=null,?int $limit=null): array {{ $this->seen[]=$page;return ['items'=>$page<3?[$page]:[]]; }}\n{query}\n{} public function select(mixed $value,string $path):mixed {{return self::poolsterJsonPath($value,$path);}} }}\nclass Body {{ public array $seen=[]; public function listPets(array $body,?int $page=null):array {{ $this->seen[]=[$body,$page];return ['items'=>$body['page']<3?[1,2]:[3]]; }}\n{body}\n{} }}\n",
        render_pagination_helper(),
        render_pagination_helper()
    );
    let assertions = r#"
function check(bool $value):void { if(!$value){throw new Exception('pagination assertion failed');} }
$q=new Query();check(count(iterator_to_array($q->listPetsPages()))===3 && $q->seen===[1,2,3]);
$q->seen=[];check(count(iterator_to_array($q->listPetsPages(0,2)))===1 && $q->seen===[0]);
check($q->select([1,2],'/-1')===null && $q->select([1,2],'/01')===null && $q->select([1,2],'$[-1]')===2);
check($q->select(['a/b'=>['~items'=>[1]]],'/a~1b/~0items/0')===1);
$b=new Body();$original=['page'=>2,'limit'=>2,'filter'=>'keep'];check(count(iterator_to_array($b->listPetsPages($original,77)))===2);
check($original===['page'=>2,'limit'=>2,'filter'=>'keep'] && array_column(array_column($b->seen,0),'page')===[2,3] && $b->seen[0][1]===77);
"#;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("probe.php");
    std::fs::write(&file, script + assertions).unwrap();
    let output = std::process::Command::new("php")
        .arg(file)
        .output()
        .expect("PHP 8.2+ is required");
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
