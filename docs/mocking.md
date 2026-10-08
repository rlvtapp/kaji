# Contract mocking

| Need | Use |
| --- | --- |
| Run immediately without Docker | [Native mock](#native-mock-inspector-no-docker) |
| Share a reproducible service across SDK tests | [Generated Docker package](#generate-and-run) |
| Return a conditional error or fixture | [`x-kaji-mock`](#full-x-kaji-mock-reference) |
| Configure page continuation | [Pagination declarations](#pagination-declarations) |

> Need help choosing between Faker, MSW, Cypress, and the Docker mock? Start
> with [testing generated SDKs](guides/testing.md).

`mock::package("mock-server").with(mock::server())` writes an `httpmock` Docker package
next to SDK outputs. Its happy-path fixtures are derived from declared OpenAPI responses
and schema examples. This is one service every generated SDK can use: set its normal
base URL to `http://localhost:5000` during a test.

## Native mock inspector (no Docker)

For local development, Kaji can run the contract directly:

```sh
kaji mock serve openapi.yaml --port 4010
```

The mock API is available at `http://127.0.0.1:4010`. `GET /_kaji/health` returns `{
"ok": true }`. Its machine-readable `GET /_kaji/requests` log returns up to the 200 most
recent calls, including the matched operation ID, scenario name, status code, and
request body. This keeps diagnosis useful for people and agents without shipping a
separate dashboard.

Unless an `x-kaji-mock` scenario matches, unconstrained schema fields vary for every
request: strings, IDs, emails, numbers, dates, arrays, and objects are generated from
the response schema. Explicit examples, defaults, constants, and enum values remain
stable.

This gives local apps realistic changing data while retaining contract-owned values
where the API specifies them.

The native server also evaluates declared `x-kaji-mock` scenarios. The first matching
scenario in OpenAPI order wins and returns its exact status, headers, body, and optional
delay; its name is recorded in `/_kaji/requests`.

This gives local development and Docker fixtures the same conditional contract cases,
while only the native server generates a fresh fallback body.

## Generate and run

```rust
use kaji::{mock, prelude::*, python, ts, generate};

let artifacts = generate(
    &api,
    ProfileSet::new("sdk")
        .package(ts::package("typescript").with(ts::sdk().fetch()))
        .package(python::package("python").with(python::sdk()))
        .package(mock::package("mock-server")
            .with(mock::server().image("httpmock/httpmock:0.8.0").port(5000))),
)?;
artifacts.write_to("generated")?;
```

The mock package is written to `generated/sdk/mock-server` and contains a `Dockerfile`,
`compose.yaml`, `run.sh`, `.env.example`, and one YAML file per operation below
`fixtures/`.

```sh
cd generated/sdk/mock-server
cp .env.example .env # optional: set KAJI_MOCK_PORT
docker compose up --build
# or: sh ./run.sh
```

The default happy path chooses the lowest declared numeric `2xx` response, then
`default`, then the first declared response, then `200`. Bodies prefer OpenAPI defaults,
constants, enum values, and examples; otherwise Kaji emits a conservative schema-shaped
value. Change the OpenAPI source and regenerate instead of hand-editing generated
fixtures.

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

Scenarios validate request headers, query values, path parameters, or an exact JSON
body. Responses can set a status, headers, body, and bounded delay. Both the native
server and fixture generator put scenarios ahead of the default route, so the fallback
cannot swallow them.

The native server compares headers case-insensitively and decodes query/path values
before comparison.

For a runnable contract with dynamic fallback responses, a conditional error, and
request-log inspection, use the [mock scenarios
example](../examples/mock-scenarios/README.md).

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

Unknown fields, duplicate or empty names, invalid status codes, and unsafe header values
fail generation. This is deliberate: a broken test scenario must not silently fall back
to a happy-path response.

Use `path` only for declared path parameters. `query` uses the serialized wire value, so
quote numbers and booleans. `body` is an exact JSON match, not a partial-object or
JSONPath matcher. Omit `when` only when a scenario should take precedence for every
request to the operation.

## Pagination declarations

The generated mock and generated pager both come from the same operation. Add
`x-kaji-pagination` when the SDK should offer a pager:

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

This is an HTTP contract mock: request method/path plus explicit scenarios in, then
either a deterministic scenario response or a schema-shaped dynamic fallback. It is not
a stateful database, authentication emulator, or arbitrary code runtime.

Keep durable HTTP contract cases in the OpenAPI document; use a dedicated test service
for stateful product behavior.
