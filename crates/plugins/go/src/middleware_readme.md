
## Customer middleware

The first configured wrapper is outermost. Import `net/http` for this example:

```go
addHeader := func(next KajiHTTPClient) KajiHTTPClient {
    return KajiHTTPClientFunc(func(request *http.Request) (*http.Response, error) {
        rewritten := request.Clone(request.Context())
        rewritten.Header.Set("X-Customer", "acme")
        return next.Do(rewritten)
    })
}
client, err := NewClient(ClientConfig{
    BaseURL: "https://api.example.com",
    Middleware: []KajiMiddleware{addHeader},
})
```

Wrappers may rewrite native requests/responses, recover errors, or return a response without calling `next`. They run per attempt, including retries and SSE establishment. Preserve request context for cancellation and support concurrent calls. The SDK closes returned bodies; middleware must close discarded bodies. Existing `HTTPClient` injection and lifecycle hooks remain available. Authentication headers are visible to wrappers.

Middleware supplied by the SDK author during generation is bundled as readable source and registered automatically. Instantiate the client normally to use those defaults. Constructor middleware adds wrappers after the bundled defaults.
