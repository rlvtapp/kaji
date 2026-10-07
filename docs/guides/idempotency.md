# Idempotency and retry safety

SDK authors can bundle automatic idempotency keys into generated operations. SDK
customers can override the key through the operation's normal optional header
parameter. This works with native SDKs and plugin composition; no consumer
middleware setup is needed.

## Enable only endpoints your server supports

An idempotency key identifies one logical mutation. The server must store the
key and its result, reject conflicting payloads, and replay a prior result according
to its documented retention policy. Generating a header does not implement those
server behaviors.

For a supporting endpoint, annotate the OpenAPI operation:

```yaml
paths:
  /orders:
    post:
      operationId: createOrder
      x-kaji-idempotency:
        header: Idempotency-Key
        auto_generate: true
      responses:
        '201':
          description: Order created
```

Absent configuration is disabled. `true` enables a caller-supplied optional key;
`auto_generate` defaults to `false`. An enabled object defaults its header to
`Idempotency-Key`. Existing compatible optional string header parameters are
reused. Required, referenced, constrained, duplicate, or colliding parameters
fail generation with a diagnostic. Transport/authentication header names cannot
be used as idempotency headers.

## Configure it in kaji.json

Policies belong to individual packages. Add this field to the package containing
your SDK plugin:

```json
{
  "language": "typescript",
  "path": "typescript",
  "api_reference": true,
  "plugins": [{ "name": "sdk", "transport": "fetch" }],
  "idempotency": {
    "defaults": { "enabled": false },
    "operations": {
      "createOrder": {
        "header": "Idempotency-Key",
        "auto_generate": true
      }
    }
  }
}
```

The order is operation recipe rule → package recipe defaults → OpenAPI extension.
Rules replace the whole lower-priority rule. Unknown operation IDs fail generation.
Avoid enabling a package-wide default unless every selected operation supports it.
Different packages can choose different policies without mutating the input API.
The Rust package builder exposes `.idempotency(IdempotencyConfig { ... })` from
`kaji` (also exported by `kaji::prelude`):

```rust
use kaji::prelude::*;
use std::collections::BTreeMap;

let sdk = kaji::rust::package("rust")
    .idempotency(IdempotencyConfig {
        defaults: None,
        operations: BTreeMap::from([(
            "createOrder".into(),
            IdempotencyRule { auto_generate: true, ..Default::default() },
        )]),
    })
    .with(kaji::rust::sdk());
```

## Customer behavior

When automatic generation is enabled, an omitted key gets a secure UUID v4 once
per SDK call. Every automatic retry uses that same key. A second SDK call gets a
new key. Explicit customer keys are preserved.

For retries across SDK calls, jobs, or process restarts, persist your own key and
pass it again. The optional native argument or input field follows each language's
usual parameter naming, for example `idempotency_key` in Rust/Python.
TypeScript customers pass `headers: { "Idempotency-Key": "your-key" }` in the
operation options. A configured `X-Request-Key` follows the same native naming
rules or appears under its exact wire name in TypeScript headers. An explicit empty or whitespace-only key is preserved but does not authorize replay. Use a
nonempty key and follow your server's accepted format.

Do not reuse a key for different payloads. Middleware may inspect or change the
header, but middleware that replaces it independently on each attempt defeats
retry deduplication. Provider plugins receive the prepared optional parameter and
`x-kaji-idempotency-resolved` metadata; custom operation providers must honor the
policy when substituting the maintained renderer.

## Retries and target limits

GET, PUT, and DELETE retain their normal retry eligibility. POST and PATCH need
an idempotency key; PATCH is no longer assumed safe merely because of its method.
Custom headers apply to their declared operation rather than globally enabling
replay. Existing retry configuration still controls whether and how often calls
retry. Ruby and Swift generate keys but do not add an automatic retry loop.

TypeScript, Go, Python, Rust, PHP, Elixir, Java, and C# recognize server
`retry-after-ms` delays alongside `Retry-After`, bounded by their configured maximum
delay. Rust and Java also accept HTTP-date `Retry-After` values. Key generation
happens before the retry loop, including when the server requests a delay.

Rust SDK manifests include UUID v4 support only when automatic keys are needed.
TypeScript requires Web Crypto and fails explicitly if secure randomness is
unavailable. Python, Ruby, Go, Java, C#, PHP, Elixir, and Swift use native secure
randomness facilities.

Select `api_reference: true` to include the resolved per-operation header and
automatic-generation behavior in the generated API reference. Tests should prove
stable retry keys, distinct calls, explicit overrides, custom headers, and no
replay of an unsupported mutation. See [testing](testing.md) and
[runtime middleware](runtime-middleware.md).
