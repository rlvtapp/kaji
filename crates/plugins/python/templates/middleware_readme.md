
## Customer middleware

Pass native transport wrappers in declaration order; the first is outermost.

```python
def add_header(request, next):
    request.add_header("X-Customer", "acme")
    return next(request)

client = Client("https://api.example.com", middleware=(add_header,))
```

Wrappers receive a urllib request and may rewrite it, replace/recover a response or error, or return a native response without calling `next`. Raise `HTTPError` for synthetic HTTP errors. The SDK closes returned responses; close discarded responses yourself. The chain runs for each retry/auth replay and SSE connection establishment.

When the optional async client is generated, use native httpx objects and async wrappers instead:

```python
async def add_async_header(request, next):
    request.headers["X-Customer"] = "acme"
    return await next(request)

# Import AsyncClient from this SDK's package when async generation is enabled.
client = AsyncClient("https://api.example.com", async_middleware=(add_async_header,))
```

Preserve cancellation and avoid logging authentication headers. Shared middleware state must support concurrent calls. Existing lifecycle observation hooks remain available.

Middleware supplied by the SDK author during generation is bundled as readable source and registered automatically. Instantiate the client normally to use those defaults. Constructor middleware adds wrappers after the bundled defaults.
