use super::*;

#[test]
fn native_middleware_rewrites_recovers_and_short_circuits_sync_and_async() {
    let documentation = render_readme(
        &api(),
        "example-api-sdk",
        "example_api_sdk",
        SdkClientStyle::Flat,
    );
    assert!(documentation.contains("middleware=(add_header,)"));
    assert!(documentation.contains("async_middleware=(add_async_header,)"));
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
from email.message import Message
from urllib.error import URLError
from example_api_sdk.runtime import BaseClient
from example_api_sdk.async_runtime import AsyncBaseClient
import example_api_sdk.runtime as runtime
class Response:
    status = 200
    def __init__(self):
        self.headers = Message(); self.headers['Content-Type'] = 'application/json'; self.closed = False
    def read(self): return b'{"rewritten": true}'
    def __enter__(self): return self
    def __exit__(self, *args): self.closed = True
order = []
def failing(request, **kwargs):
    assert request.get_header('X-custom') == 'yes'
    raise URLError('offline')
runtime.urlopen = failing
def outer(request, following):
    order.append('outer-before'); request.add_header('X-Custom', 'yes')
    response = following(request); order.append('outer-after'); return response
def recovery(request, following):
    order.append('inner')
    try: return following(request)
    except URLError: return Response()
client = BaseClient('https://example.test', middleware=(outer, recovery))
assert client._request('GET', '/') == {'rewritten': True}
assert order == ['outer-before', 'inner', 'outer-after']
response = Response()
assert BaseClient('https://example.test', middleware=(lambda request, following: response,))._request('GET', '/') == {'rewritten': True}
assert response.closed
# A middleware that delegates must preserve terminal failures.
try: BaseClient('https://example.test', middleware=(outer,))._request('GET', '/')
except URLError as error: assert error.reason == 'offline'
else: raise AssertionError('transport error swallowed')
class AsyncResponse:
    status_code = 200
    headers = {'content-type': 'application/json'}
    closed = False
    async def aread(self): return b'{"async": true}'
    async def aclose(self): self.closed = True
class Transport:
    def build_request(self, method, url, **options): return options
    async def send(self, request, **options):
        assert request['headers']['X-Custom'] == 'yes'
        raise ValueError('offline')
async def main():
    seen = []; result = AsyncResponse()
    async def first(request, following):
        seen.append('before'); request['headers']['X-Custom'] = 'yes'
        value = await following(request); seen.append('after'); return value
    async def recover(request, following):
        try: return await following(request)
        except ValueError: return result
    client = AsyncBaseClient('https://example.test', http_client=Transport(), async_middleware=(first, recover))
    assert await client._request('GET', '/') == {'async': True}
    assert seen == ['before', 'after'] and result.closed
    async def shortcut(request, following): return AsyncResponse()
    client = AsyncBaseClient('https://example.test', http_client=Transport(), async_middleware=(shortcut,))
    assert await client._request('GET', '/') == {'async': True}
    # Transport would reject the absent header if short circuit delegated.
    client = AsyncBaseClient('https://example.test', http_client=Transport(), async_middleware=(first,))
    try: await client._request('GET', '/')
    except ValueError as error: assert str(error) == 'offline'
    else: raise AssertionError('async transport error swallowed')
    async def cancelled(request, following): raise asyncio.CancelledError()
    client = AsyncBaseClient('https://example.test', http_client=Transport(), async_middleware=(cancelled,))
    try: await client._request('GET', '/')
    except asyncio.CancelledError: pass
    else: raise AssertionError('cancellation swallowed')
asyncio.run(main())
"#;
    assert!(
        Command::new("python3")
            .args(["-c", script])
            .env("PYTHONPATH", root.path().join("sdk/python/src"))
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .status()
            .unwrap()
            .success()
    );
}
#[test]
fn author_bundled_middleware_is_registered_without_customer_configuration() {
    use poolster_core::{customization::BundledMiddleware, engine::Packages};
    let middleware = BundledMiddleware { path: "src/example_api_sdk/customer.py".into(), contents: "def add_header(request, next):\n    request.add_header('X-Bundled', 'yes')\n    return next(request)\nasync def add_async_header(request, next):\n    request['headers']['X-Bundled'] = 'async'\n    return await next(request)\n".into(), symbol: "add_header".into(), async_symbol: Some("add_async_header".into()) };
    let tree = Packages::new()
        .package(
            crate::package("sdk")
                .name("example-api-sdk")
                .with(crate::sdk().async_client(true))
                .middleware(middleware.clone()),
        )
        .generate(&api(), None)
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let script = r#"import asyncio
from email.message import Message
from example_api_sdk import Client, AsyncClient
import example_api_sdk.runtime as runtime
class Response:
    status = 200
    def __init__(self): self.headers = Message(); self.headers['Content-Type'] = 'application/json'
    def read(self): return b'{}'
    def __enter__(self): return self
    def __exit__(self, *args): pass
def send(request, **kwargs):
    assert request.get_header('X-bundled') == 'yes'
    return Response()
runtime.urlopen = send
assert Client('https://example.test')._request('GET', '/') == {}
class AsyncResponse:
    status_code = 200
    headers = {'content-type': 'application/json'}
    async def aread(self): return b'{}'
    async def aclose(self): pass
class Transport:
    def build_request(self, method, url, **options): return options
    async def send(self, request, **options):
        assert request['headers']['X-Bundled'] == 'async'
        return AsyncResponse()
async def main():
    client = AsyncClient('https://example.test', http_client=Transport())
    assert await client._request('GET', '/') == {}
asyncio.run(main())
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
    let mut invalid = middleware.clone();
    invalid.async_symbol = None;
    assert!(
        Packages::new()
            .package(
                crate::package("sdk")
                    .name("example-api-sdk")
                    .with(crate::sdk().async_client(true))
                    .middleware(invalid)
            )
            .generate(&api(), None)
            .is_err()
    );
    let mut collision = middleware;
    collision.path = "src/example_api_sdk/runtime.py".into();
    assert!(
        Packages::new()
            .package(
                crate::package("sdk")
                    .name("example-api-sdk")
                    .with(crate::sdk())
                    .middleware(collision)
            )
            .generate(&api(), None)
            .is_err()
    );
}
