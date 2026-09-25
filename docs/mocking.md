# Contract mocking

`ProfileSet::mock_server()` writes a standalone `httpmock` Docker package next
to SDK outputs. Its happy-path fixtures are derived from declared OpenAPI
responses and schema examples. This is one service every generated SDK can
use: set its normal base URL to `http://localhost:5000` during a test.

## Generate and run

```rust
use kaji::{MockServerOptions, ProfileSet, generate};

let artifacts = generate(
    &api,
    ProfileSet::new("sdk")
        .typescript_fetch()
        .python()
        .mock_server()
        .mock_server_options(MockServerOptions {
            image: "httpmock/httpmock:0.8.0".into(),
            port: 5000,
        }),
)?;
artifacts.write_to("generated")?;
```

The mock package is written to `generated/sdk/mock-server` and contains a
`Dockerfile`, `compose.yaml`, `run.sh`, `.env.example`, and one YAML file per
operation below `fixtures/`.

```sh
cd generated/sdk/mock-server
cp .env.example .env # optional: set KAJI_MOCK_PORT
docker compose up --build
# or: sh ./run.sh
```

The default happy path chooses the lowest declared numeric `2xx` response,
then `default`, then the first declared response, then `200`. Bodies prefer
OpenAPI defaults, constants, enum values, and examples; otherwise Kaji emits a
conservative schema-shaped value. Change the OpenAPI source and regenerate
instead of hand-editing generated fixtures.

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

## Full `x-kaji-mock` reference

```yaml
x-kaji-mock:
  scenarios:
    - name: string                 # required; unique in this operation
      when:                        # optional; every supplied predicate matches
        headers: { name: value }   # string values only
        query: { name: value }     # string values only
        path: { name: value }      # binds declared {name} placeholders
        body: { any: json }        # exact JSON body match
      response:                    # required
        status: 429                # required integer, 100 through 599
        headers: { name: value }   # optional strings
        body: { any: json }        # optional JSON value
        delay_ms: 25               # optional, 0 through 600000
```

Unknown fields, duplicate or empty names, invalid status codes, and unsafe
header values fail generation. This is deliberate: a broken test scenario must
not silently fall back to a happy-path response.

Use `path` only for declared path parameters. `query` uses the serialized wire
value, so quote numbers and booleans. `body` is an exact JSON match, not a
partial-object or JSONPath matcher. Omit `when` only when a scenario should
take precedence for every request to the operation.

## Pagination declarations

The generated mock and generated pager both come from the same operation.
Add `x-kaji-pagination` when the SDK should offer a pager:

```yaml
# Cursor in a query or request body field.
x-kaji-pagination:
  type: cursor
  inputs:
    - { name: cursor, in: parameters, type: cursor }
  outputs:
    nextCursor: $.next_cursor

# For a nested request-body cursor, Kaji needs an explicit RFC 6901 pointer.
x-kaji-pagination:
  type: cursor
  inputs:
    - { name: cursor, in: requestBody, type: cursor, bodyPath: /page/cursor }
  outputs:
    nextCursor: $.next_cursor

# Offset pagination; `limit` is optional. Page stepping may use numPages.
x-kaji-pagination:
  type: offsetLimit
  inputs:
    - { name: offset, in: parameters, type: offset }
    - { name: limit, in: parameters, type: limit }
  outputs:
    results: $.items

# A server-returned URL continuation. Kaji only follows same-origin URLs.
x-kaji-pagination:
  type: url
  outputs:
    nextUrl: $.links.next
```

Kaji also accepts `x-speakeasy-pagination` for existing specifications. Use
`x-kaji-pagination` in new documents.

## Scope

This is a static HTTP contract mock: request method/path plus explicit
scenarios in, deterministic response out. It is not a stateful database,
authentication emulator, or arbitrary code runtime. Keep durable HTTP
contract cases in the OpenAPI document; use a dedicated test service for
stateful product behavior.
