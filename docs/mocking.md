# Contract mocking

`ProfileSet::mock_server()` writes a standalone `httpmock` Docker package next
to SDK outputs. Its happy-path fixtures are derived from declared OpenAPI
responses and schema examples.

Use `x-kaji-mock` only for named conditional behaviour:

```yaml
x-kaji-mock:
  scenarios:
    - name: rate-limited
      when:
        headers:
          x-test-scenario: rate-limited
      response:
        status: 429
        headers:
          retry-after: "1"
        body:
          message: Too many requests
        delay_ms: 25
```

Scenarios validate request headers, query values, path parameters, or an exact
JSON body. Responses can set a status, headers, body, and bounded delay. The
fixture generator places specific scenarios before the default route, so the
fallback cannot swallow them.
