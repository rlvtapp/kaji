use super::*;

#[test]
fn emits_cursor_paginators_only_for_declared_operation_inputs() {
    let mut source = api();
    source.operations[0].id = "listContacts".into();
    source.operations[0].path = "/contacts".into();
    source.operations[0].parameters = vec![OperationParameter {
        name: "cursor".into(),
        location: "query".into(),
        required: false,
        schema: Some(SchemaValue::new(SchemaKind::String)),
        description: None,
        annotations: Default::default(),
    }];
    source.operations[0].annotations.insert(
        "x-poolster-pagination".into(),
        serde_json::json!({
            "type": "cursor",
            "inputs": [{ "name": "cursor", "in": "parameters", "type": "cursor" }],
            "outputs": { "nextCursor": "$.page.next[-1]" }
        }),
    );

    let tree = render_sdk(
        &source,
        "sdk/python",
        Some("example-api-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    let client = rendered_python(&tree);
    assert!(client.contains(
        "def list_contacts_pages(self, *, cursor: str | None = None) -> Iterator[Contact]:"
    ));
    assert!(client.contains("response = self.list_contacts(cursor=poolster_cursor)"));
    assert!(client.contains("_poolster_json_path(response, \"$.page.next[-1]\")"));
    assert!(client.contains("Supported paths start at ``$``"));
    assert!(
        client.contains("def list_pages(self, *, cursor: str | None = None) -> Iterator[Contact]:")
    );

    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let status = Command::new("python3")
        .args(["-m", "compileall", "-q"])
        .env("PYTHONPYCACHEPREFIX", root.path().join("python-cache"))
        .arg(root.path().join("sdk/python/src"))
        .status()
        .unwrap();
    assert!(status.success());

    source.operations[0].annotations.insert(
        "x-poolster-pagination".into(),
        serde_json::json!({
            "type": "cursor",
            "inputs": [{ "name": "missing", "in": "parameters", "type": "cursor" }],
            "outputs": { "nextCursor": "$.next" }
        }),
    );
    let without_input = render_test_sdk(&source, "sdk/python", Some("example-api-sdk")).unwrap();
    assert!(!rendered_python(&without_input).contains("def list_contacts_pages"));
}

#[test]
fn page_pagination_executes_sync_async_defaults_and_selectors() {
    let mut source = api();
    source.operations.truncate(1);
    let op = &mut source.operations[0];
    op.id = "listItems".into();
    op.path = "/items".into();
    op.parameters = ["page", "limit"]
        .into_iter()
        .map(|name| OperationParameter {
            name: name.into(),
            location: "query".into(),
            required: false,
            schema: Some(SchemaValue::new(SchemaKind::Integer)),
            description: None,
            annotations: Default::default(),
        })
        .collect();
    op.responses = vec![OperationResponse::json(
        "200",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![Field {
                name: "items".into(),
                required: true,
                value: SchemaValue::new(SchemaKind::Array {
                    items: Box::new(SchemaValue::new(SchemaKind::Integer)),
                }),
                annotations: Default::default(),
            }],
            additional_properties: AdditionalProperties::Any,
        }),
    )];
    op.annotations.insert("x-poolster-pagination".into(), serde_json::json!({
            "type":"page", "inputs":[{"name":"page","in":"parameters","type":"page"},{"name":"limit","in":"parameters","type":"limit"}],
            "outputs":{"results":"/items"}
        }));
    let root = tempfile::tempdir().unwrap();
    render_sdk_with_async(&source, "sdk", Some("page-sdk"), SdkClientStyle::Flat, true)
        .unwrap()
        .write_to(root.path())
        .unwrap();
    let script = r#"import asyncio
from page_sdk import Client, AsyncClient
from page_sdk.runtime import _poolster_json_path
assert _poolster_json_path([{'items':[1,2]}], '$[0].items[-1]') == 2
assert _poolster_json_path({'a/b':{'~items':[[1]]}}, '/a~1b/~0items/0') == [1]
assert _poolster_json_path({}, '$.bad[') is None
assert _poolster_json_path([1,2], '/-1') is None
assert _poolster_json_path([1,2], '/01') is None
assert _poolster_json_path([1,2], '/+1') is None
assert _poolster_json_path([1,2], '/0') == 1
assert _poolster_json_path([1,2], '$[-1]') == 2
class Pages(Client):
    def __init__(self): self.seen=[]
    def list_items(self, *, page=None, limit=None):
        self.seen.append(page)
        return {'items':[page] if page < 3 else []}
p=Pages(); assert len(list(p.list_items_pages()))==3 and p.seen==[1,2,3]
p=Pages(); assert len(list(p.list_items_pages(page=0,limit=2)))==1 and p.seen==[0]
for limit in [True,0,-1,1.5]:
    p=Pages()
    try: list(p.list_items_pages(limit=limit)); raise AssertionError('bad limit accepted')
    except ValueError: pass
    assert p.seen==[]
for bad in [True,-1,1.5]:
    try: list(Pages().list_items_pages(page=bad)); raise AssertionError('bad page accepted')
    except ValueError: pass
class AsyncPages(AsyncClient):
    def __init__(self): self.seen=[]
    async def list_items(self, *, page=None, limit=None):
        self.seen.append(page)
        return {'items':[page] if page<3 else []}
async def run():
    p=AsyncPages(); assert len([x async for x in p.list_items_pages()])==3 and p.seen==[1,2,3]
asyncio.run(run())
"#;
    assert!(
        Command::new("python3")
            .args(["-c", script])
            .env("PYTHONPATH", root.path().join("sdk/src"))
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn body_page_pagination_preserves_limit_filters_and_original_inputs() {
    let mut source = api();
    source.operations.truncate(1);
    source.schemas.push(Schema::new(
        "PageInput",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![
                Field {
                    name: "page-number".into(),
                    value: SchemaValue::new(SchemaKind::Integer),
                    required: false,
                    annotations: Default::default(),
                },
                Field {
                    name: "page-size".into(),
                    value: SchemaValue::new(SchemaKind::Integer),
                    required: true,
                    annotations: Default::default(),
                },
                Field {
                    name: "filter".into(),
                    value: SchemaValue::new(SchemaKind::String),
                    required: true,
                    annotations: Default::default(),
                },
            ],
            additional_properties: AdditionalProperties::Forbidden,
        }),
    ));
    let op = &mut source.operations[0];
    op.id = "searchItems".into();
    op.method = HttpMethod::Post;
    op.path = "/search".into();
    op.parameters = vec![OperationParameter {
        name: "page-number".into(),
        location: "query".into(),
        required: false,
        schema: Some(SchemaValue::new(SchemaKind::Integer)),
        description: None,
        annotations: Default::default(),
    }];
    op.request_body = Some(OperationRequestBody::json(
        SchemaValue::reference("#/components/schemas/PageInput"),
        true,
    ));
    op.responses = vec![OperationResponse::json(
        "200",
        SchemaValue::new(SchemaKind::Object {
            fields: vec![Field {
                name: "items".into(),
                required: true,
                value: SchemaValue::new(SchemaKind::Array {
                    items: Box::new(SchemaValue::new(SchemaKind::Integer)),
                }),
                annotations: Default::default(),
            }],
            additional_properties: AdditionalProperties::Any,
        }),
    )];
    op.annotations.insert("x-poolster-pagination".into(),serde_json::json!({"type":"page","inputs":[{"name":"page-number","in":"requestBody","type":"page"},{"name":"page-size","in":"requestBody","type":"limit"}],"outputs":{"results":"/items"}}));
    let root = tempfile::tempdir().unwrap();
    render_sdk_with_async(
        &source,
        "sdk",
        Some("body-page-sdk"),
        SdkClientStyle::Namespaced,
        true,
    )
    .unwrap()
    .write_to(root.path())
    .unwrap();
    let script = r#"import asyncio
from body_page_sdk import Client, AsyncClient
from body_page_sdk.models import PageInput, _to_wire
class Pages(Client):
    def __init__(self): self.seen=[]
    def search_items(self, *, body, page_number=None):
        wire=_to_wire(body)
        self.seen.append((wire,page_number))
        return {'items':[1,2] if wire['page-number']<3 else [3]}
original={'page-number':2,'page-size':2,'filter':'keep'}
p=Pages();assert len(list(p.search_items_pages(body=original,page_number=77)))==2
assert p.seen==[({'page-number':2,'page-size':2,'filter':'keep'},77),({'page-number':3,'page-size':2,'filter':'keep'},77)]
assert original=={'page-number':2,'page-size':2,'filter':'keep'}
model=PageInput.from_dict({'page-number':2,'page-size':2,'filter':'keep'})
p=Pages();assert len(list(p.search_items_pages(body=model,page_number=88)))==2
assert _to_wire(model)=={'page-number':2,'page-size':2,'filter':'keep'} and p.seen[-1][1]==88
missing=PageInput.from_dict({'page-size':2,'filter':'keep'})
p=Pages();assert len(list(p.search_items_pages(body=missing)))==3
assert _to_wire(missing)=={'page-size':2,'filter':'keep'} and p.seen[0][0]['page-number']==1
for bad in [True,-1,1.5]:
    try: list(Pages().search_items_pages(body={'page-number':bad,'page-size':2,'filter':'keep'}));raise AssertionError('bad body page accepted')
    except ValueError: pass
class AsyncPages(AsyncClient):
    def __init__(self): self.seen=[]
    async def search_items(self, *, body, page_number=None):
        wire=_to_wire(body);self.seen.append((wire,page_number))
        return {'items':[1,2] if wire['page-number']<3 else [3]}
async def run():
    p=AsyncPages();assert len([value async for value in p.search_items_pages(body=model,page_number=99)])==2
    assert p.seen==[({'page-number':2,'page-size':2,'filter':'keep'},99),({'page-number':3,'page-size':2,'filter':'keep'},99)]
    assert _to_wire(model)=={'page-number':2,'page-size':2,'filter':'keep'}
asyncio.run(run())
"#;
    let output = Command::new("python3")
        .args(["-c", script])
        .env("PYTHONPATH", root.path().join("sdk/src"))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
