use super::*;

#[test]
fn async_oauth_coordinates_refresh_and_bounds_repeated_401() {
    let root = tempfile::tempdir().unwrap();
    render_sdk_with_async(
        &api(),
        "sdk/python",
        Some("example-api-sdk"),
        SdkClientStyle::Flat,
        true,
    )
    .unwrap()
    .write_to(root.path())
    .unwrap();
    let script = r#"import asyncio
from example_api_sdk import AsyncClient, AsyncOAuthClientCredentials, ApiError
class TokenResponse:
    def __init__(self, token): self.token = token
    def raise_for_status(self): pass
    def json(self): return {'access_token': self.token, 'expires_in': 3600}
    async def aclose(self): pass
class Issuer:
    calls = 0
    async def post(self, url, **options):
        self.calls += 1; await asyncio.sleep(0.01); return TokenResponse('token'+str(self.calls))
class Denied:
    status_code = 401
    headers = {'content-type': 'application/json'}
    async def aread(self): return b'{}'
    async def aclose(self): pass
class Transport:
    def __init__(self): self.tokens = []
    def build_request(self, method, url, **options): return options
    async def send(self, request, stream=False):
        self.tokens.append(request['headers']['Authorization']); return Denied()
async def main():
    issuer = Issuer(); provider = AsyncOAuthClientCredentials('https://auth.example/token', 'id', 'secret', http_client=issuer)
    assert set(await asyncio.gather(*[provider(None) for _ in range(8)])) == {'token1'}
    assert issuer.calls == 1
    assert set(await asyncio.gather(*[provider('token1') for _ in range(8)])) == {'token2'}
    assert issuer.calls == 2
    transport = Transport(); client = AsyncClient('https://example.com', token_provider=provider, http_client=transport, max_retries=10)
    try: await client.health()
    except ApiError as error: assert error.status_code == 401
    else: raise AssertionError('repeated 401 accepted')
    assert transport.tokens == ['Bearer token2', 'Bearer token3']
    assert issuer.calls == 3
    # Cancellation while leading a refresh or waiting for its lock must release
    # ownership and leave the previous cached token available for a later retry.
    class BlockingIssuer:
        def __init__(self): self.calls=0; self.started=asyncio.Event(); self.release=asyncio.Event()
        async def post(self, url, **options):
            self.calls+=1
            if self.calls==2:
                self.started.set(); await self.release.wait()
            return TokenResponse('fresh'+str(self.calls))
    blocking=BlockingIssuer();managed=AsyncOAuthClientCredentials('https://auth.example/token','id','secret',http_client=blocking)
    assert await managed(None)=='fresh1'
    leader=asyncio.create_task(managed('fresh1'));await blocking.started.wait()
    waiter=asyncio.create_task(managed('fresh1'));await asyncio.sleep(0)
    waiter.cancel()
    try:await waiter
    except asyncio.CancelledError:pass
    else:raise AssertionError('cancelled refresh waiter returned a token')
    assert blocking.calls==2
    leader.cancel()
    try:await leader
    except asyncio.CancelledError:pass
    else:raise AssertionError('cancelled refresh leader returned a token')
    assert not managed._lock.locked() and managed._token=='fresh1'
    assert await managed('fresh1')=='fresh3' and blocking.calls==3
    # Transport cancellation must bypass retry/error hooks and permit reuse.
    class BlockingTransport:
        def __init__(self):self.calls=0;self.started=asyncio.Event()
        def build_request(self,method,url,**options):return options
        async def send(self,request,stream=False):
            self.calls+=1;self.started.set();await asyncio.Event().wait()
    terminal=BlockingTransport();errors=[]
    cancelled_client=AsyncClient('https://unused.example',http_client=terminal,max_retries=10,on_error=errors.append)
    pending=asyncio.create_task(cancelled_client.health());await terminal.started.wait();pending.cancel()
    try:await pending
    except asyncio.CancelledError:pass
    else:raise AssertionError('transport cancellation swallowed')
    assert terminal.calls==1 and errors==[]
asyncio.run(main())
"#;
    let status = Command::new("python3")
        .args(["-c", script])
        .env("PYTHONPATH", root.path().join("sdk/python/src"))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn async_pagination_awaits_each_page_and_forwards_resource_iterator() {
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
    source.operations[0].annotations.insert("x-kaji-pagination".into(), serde_json::json!({"type":"cursor", "inputs":[{"name":"cursor", "in":"parameters", "type":"cursor"}], "outputs":{"nextCursor":"$.next"}}));
    if let SchemaKind::Object {
        additional_properties,
        ..
    } = &mut source.schemas[0].value.kind
    {
        *additional_properties = AdditionalProperties::Any;
    }
    let root = tempfile::tempdir().unwrap();
    render_sdk_with_async(
        &source,
        "sdk/python",
        Some("example-api-sdk"),
        SdkClientStyle::Namespaced,
        true,
    )
    .unwrap()
    .write_to(root.path())
    .unwrap();
    let script = r#"import asyncio, json
from example_api_sdk import AsyncClient
class Response:
    status_code = 200
    headers = {'content-type': 'application/json'}
    def __init__(self, cursor): self.cursor = cursor
    async def aread(self): return json.dumps({'id': self.cursor or 'first', 'next': 'last' if self.cursor is None else None}).encode()
    async def aclose(self): pass
class Transport:
    def __init__(self): self.cursors = []
    def build_request(self, method, url, **options): return options
    async def send(self, request, stream=False):
        cursor = request['params'].get('cursor'); self.cursors.append(cursor); return Response(cursor)
async def main():
    transport = Transport(); client = AsyncClient('https://example.com', http_client=transport)
    pages = [page async for page in client.contacts.list_pages()]
    assert [page.id for page in pages] == ['first', 'last']
    assert transport.cursors == [None, 'last']
asyncio.run(main())
"#;
    let status = Command::new("python3")
        .args(["-c", script])
        .env("PYTHONPATH", root.path().join("sdk/python/src"))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn oauth_refresh_is_coordinated_and_401_replay_is_bounded() {
    let root = tempfile::tempdir().unwrap();
    render_test_sdk(&api(), "sdk/python", Some("example-api-sdk"))
        .unwrap()
        .write_to(root.path())
        .unwrap();
    let script = r#"import io, json, threading, time
from concurrent.futures import ThreadPoolExecutor
from urllib.error import HTTPError
from example_api_sdk import Client, ApiError, OAuthClientCredentials
import example_api_sdk.runtime as runtime
class TokenResponse:
    def __init__(self, token): self.token = token
    def __enter__(self): return self
    def __exit__(self, *args): pass
    def read(self): return json.dumps({'access_token': self.token, 'expires_in': 3600, 'token_type': 'Bearer'}).encode()
calls = []
def issue(request, timeout):
    calls.append(request); time.sleep(0.01); return TokenResponse('token' + str(len(calls)))
provider = OAuthClientCredentials('https://auth.example/token', 'id', 'secret', scopes=['read'], opener=issue)
with ThreadPoolExecutor(max_workers=8) as pool:
    assert set(pool.map(lambda _: provider(None), range(8))) == {'token1'}
assert len(calls) == 1
with ThreadPoolExecutor(max_workers=8) as pool:
    assert set(pool.map(lambda _: provider('token1'), range(8))) == {'token2'}
assert len(calls) == 2
assert b'grant_type=client_credentials' in calls[0].data and b'scope=read' in calls[0].data
attempts = []
def denied(request, timeout):
    attempts.append(request.get_header('Authorization'))
    raise HTTPError(request.full_url, 401, 'unauthorized', {}, io.BytesIO(b'{}'))
runtime.urlopen = denied
client = Client('https://api.example', token_provider=provider, max_retries=10)
try: client.health()
except ApiError as error: assert error.status_code == 401
else: raise AssertionError('repeated 401 accepted')
assert attempts == ['Bearer token2', 'Bearer token3']
assert len(calls) == 3
attempts.clear()
client = Client('https://api.example', token_provider=provider, headers={'authorization': 'custom'})
try: client.health()
except ApiError: pass
assert len(attempts) == 1 and len(calls) == 3
"#;
    let status = Command::new("python3")
        .args(["-c", script])
        .env("PYTHONPATH", root.path().join("sdk/python/src"))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn native_async_client_shares_models_and_closes_streams() {
    let root = tempfile::tempdir().unwrap();
    let tree = render_sdk_with_async(
        &api(),
        "sdk/python",
        Some("example-api-sdk"),
        SdkClientStyle::Namespaced,
        true,
    )
    .unwrap();
    tree.write_to(root.path()).unwrap();
    let script = r#"import asyncio, json
from example_api_sdk import AsyncClient, Contact
class Response:
    status_code = 200
    headers = {'content-type': 'application/json'}
    closed = False
    async def aread(self): return b'{"id":"123"}'
    async def aclose(self): self.closed = True
    async def aiter_lines(self):
        yield 'data: {"ok":true}'
        yield ''
        await asyncio.sleep(100)
class Transport:
    def __init__(self): self.responses = []; self.active = 0; self.peak = 0
    def build_request(self, method, url, **options): return (method, url, options)
    async def send(self, request, stream=False):
        self.active += 1; self.peak = max(self.peak, self.active)
        await asyncio.sleep(0.01)
        self.active -= 1
        response = Response(); self.responses.append(response); return response
async def main():
    transport = Transport()
    client = AsyncClient('https://example.com', http_client=transport)
    results = await asyncio.gather(client.get_contact(contact_id='1'), client.contacts.get(contact_id='2'))
    assert all(isinstance(item, Contact) for item in results)
    assert transport.peak == 2
    assert all(item.closed for item in transport.responses)
    stream = await client._event_stream('GET', '/events')
    assert await stream.__anext__() == {'ok': True}
    await stream.aclose()
    assert transport.responses[-1].closed
    stream = await client._event_stream('GET', '/events')
    await stream.__anext__()
    task = asyncio.create_task(stream.__anext__())
    await asyncio.sleep(0)
    task.cancel()
    try: await task
    except asyncio.CancelledError: pass
    assert transport.responses[-1].closed
    try: await client._request('GET', '/', pagination_url='https://evil.example/path')
    except ValueError: pass
    else: raise AssertionError('cross-origin pagination accepted')
asyncio.run(main())
"#;
    let status = Command::new("python3")
        .args(["-c", script])
        .env("PYTHONPATH", root.path().join("sdk/python/src"))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .status()
        .unwrap();
    assert!(status.success());
}
