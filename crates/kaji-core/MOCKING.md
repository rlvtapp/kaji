# Kaji mock scenarios

Kaji mock outputs are driven by the OpenAPI document. Normal OpenAPI response
examples and schemas remain the default source of fixture data. Use
`x-kaji-mock` on an operation only when a test needs named, conditional
behaviour such as a rate-limit or a declared failure.

```yaml
paths:
  /v1/contacts/{contact_id}:
    get:
      operationId: getContact
      x-kaji-mock:
        scenarios:
          - name: rate-limited
            when:
              headers:
                x-test-scenario: rate-limited
              path:
                contact_id: contact_123
            response:
              status: 429
              headers:
                retry-after: "1"
              body:
                message: Too many requests
              delay_ms: 25
```

Each scenario has a non-empty `name`, optional exact `when` predicates, and a
required `response`. `when.headers`, `when.query`, and `when.path` are maps of
string values; `when.body` is an exact JSON body predicate. A response has an
HTTP `status` from 100 through 599, optional string headers, optional JSON
body, and optional `delay_ms` (at most 600,000 milliseconds).

Names are unique per operation. Kaji validates this extension before any mock
fixture is emitted, so typos do not silently turn a requested failure scenario
into a default successful response. The Go OpenAPI sidecar preserves
`x-kaji-mock` verbatim and the Rust adapter exposes it through each operation's
annotations.

The contract itself is backend-neutral: MSW, httpmock YAML, and a future
hosted Kaji mock service consume the same typed scenario model.
