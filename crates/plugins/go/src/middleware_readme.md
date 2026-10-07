
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

## Structural response validation

Set `ValidateResponses: true` in `ClientConfig` to check buffered JSON responses against generated named model shapes after middleware has returned the final response. Missing required response fields (excluding direct or referenced `writeOnly` fields), nonnullable `null`, incorrect scalar types, typed additional properties, and malformed nested objects/arrays return `*ResponseValidationError`. Its `Path` and `Expected` fields identify the mismatch without including response values. Future enum strings remain accepted, and models with additional properties retain unknown fields.

Validation accepts exactly one JSON value, limits the buffered response to 10 MiB, and bounds schema traversal to 128 levels. Anonymous scalars and arrays/maps receive native shape checks; containers holding named models receive schema checks too. Inline anonymous response objects retain native Go decoding checks. This is structural checking: enum membership, formats, numeric/string constraints, and unresolved composition constraints are not enforced. Text, binary, and SSE responses use their existing decoding and ownership rules. Nullable pointer destinations accept `null`. Named nullable object schemas represented by value structs cannot retain a root null separately from their zero value; validation does not change the generated method return types or repair that representation limit. The default remains compatible with native Go decoding.
