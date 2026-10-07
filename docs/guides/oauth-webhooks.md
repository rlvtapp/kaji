# Generate OAuth providers and webhook verifiers

These are optional runtime capabilities. Client credentials must stay in your
application's secret configuration; webhook verification must receive the exact
raw bytes before a framework parses or normalizes the request body.

## Go client credentials

Add the helper and optional webhook verifier beside the SDK:

```json
{
  "language": "go",
  "path": "go",
  "name": "example.com/acme/sdk",
  "plugins": [{"name":"sdk"}, {"name":"oauth"}, {"name":"webhooks"}]
}
```

Construct `NewOAuthClientCredentials(OAuthClientCredentialsConfig{TokenURL,
ClientID, ClientSecret, Scopes})`, then set the result as the generated
`ClientConfig.TokenProvider`. The token endpoint must use HTTPS. The default HTTP
client bounds requests and refuses redirects; an injected native driver is your
responsibility. Cache refresh is coordinated across callers and respects context
cancellation. Token issuer failures redact response content and credentials.

The buffered operation runtime can refresh a rejected provider token once per
logical call and replay only when its existing replay-safety rules permit it.
An unsafe POST/PATCH is not replayed. An explicit Authorization header takes
precedence. Refresh/replay is separate from transient backoff retry counts.
This helper does not add authorization-code/device flows or streaming refresh.

Python SDKs already emit sync/async `OAuthClientCredentials` helpers. See the
generated `oauth.py` and package docs for the native constructor and integration.

## TypeScript, Rust, Java and C# client credentials

Select `{"name":"oauth"}` beside the maintained SDK plugin in each language
recipe. TypeScript can bind it to a named SDK transport with
`"uses":{"transport":"client"}`; otherwise composition requires one unambiguous
maintained provider. Native Rust plugins expose `ts::oauth()`, `rust::oauth()`,
`java::oauth()` and `csharp::oauth()`. This is generated source: SDK customers use
the emitted helper without installing Kaji.

| Target | Connect the generated provider |
| --- | --- |
| TypeScript | `new OAuthClientCredentials({tokenUrl, clientId, clientSecret, scopes})`, then `createOAuthClient(config, provider)`; pass the resulting client to operations |
| Rust | `OAuthClientCredentials::new(token_url, client_id, client_secret)?`, then `client.with_token_provider(Arc::new(provider))`; custom providers implement `BearerTokenProvider` |
| Java | `new OAuthClientCredentials(issuerHttpClient, tokenUrl, clientId, clientSecret)`, then wrap the API driver with `new OAuthHttpClient(apiHttpClient, provider, apiOrigin)` |
| C# | `new OAuthClientCredentials(issuerHttpClient, tokenUrl, clientId, clientSecret)`, then use `OAuthClientCredentialsHandler(provider, apiOrigin)` with the native inner handler |

Use a separate issuer driver and HTTPS endpoints in production. Providers cache
until expiry and coordinate concurrent refresh. An explicit Authorization header
wins. Unauthorized recovery is bounded to one safe replay; unsafe mutations and
nonrepeatable/streaming bodies are not silently replayed. Java buffered-body
replay requires its explicit constructor option, and Java/C# authorize only the
configured API origin. Injected issuer drivers must refuse redirects.

Native tests exercise sixteen concurrent callers, expiry, cancellation or
interruption, issuer error redaction, explicit headers and unsafe-mutation
protection. Rust also tests a dropped refresh leader; TypeScript tests response
envelopes and cancellation. Swift, PHP and Elixir do not yet emit these providers.

## Verify signed webhook bodies

All ten SDK targets accept an opt-in `webhooks` plugin. Python and Ruby recipes add `{"name":"webhooks"}` beside `sdk`; Go uses the same
plugin name. Each implementation follows the Standard Webhooks raw-body HMAC
contract, accepts rotated signing secrets and checks a bounded timestamp window.
Unknown signature versions are ignored. Missing, ambiguous or invalid required
headers fail verification without exposing the payload or secret in errors.

| Target | Generated entry point |
| --- | --- |
| Go | `VerifyWebhook(raw, headers, secrets, options...)`, `VerifyWebhookAndDecode[T]` |
| Ruby | `require '<import>/webhooks'`; `<Module>::Webhooks.verify` / `verify_and_decode(model:)` |
| Python | `<module>.webhooks.verify_webhook` / `verify_and_decode` |
| TypeScript | `verifyWebhook(Uint8Array, headers, secrets, options)` using Web Crypto |
| Rust | `verify_webhook` / generic `verify_webhook_and_decode` |
| Swift | `StandardWebhooks.verify(Data, headers, secrets, now:, tolerance:)` / generic `verifyAndDecode` |
| Java | `StandardWebhooks.verify` / `verifyAndDecode` with a caller decoder |
| C# | `StandardWebhooks.Verify` / generic `VerifyAndDecode<T>` |
| PHP | `<Namespace>\Webhooks::verify` / `verifyAndDecode` with a caller decoder |
| Elixir | `<Module>.Webhooks.verify` / `verify_and_decode` with a caller decoder |

Go options expose `Now` and `Tolerance`; Ruby exposes `now:` and `tolerance:` for
deterministic tests. A signature check establishes authenticity within the time
window, not exactly-once delivery. Store/reject duplicate event IDs in your own
application if needed. Decode only after verification succeeds.

Native Go tests exercise concurrent refresh, canceled refresh leaders/waiters,
bounded unauthorized replay and mutation safety. Go/Ruby verifier probes use the
same checked-in vectors as Python and test raw-byte changes, rotation, clock
bounds and malformed signatures. These are protocol fixtures, not live issuer or
webhook service trials.

TypeScript requires native Web Crypto (Node 22 or a suitable browser); Rust adds
pinned-family HMAC/SHA-256/base64 dependencies. Swift adds exact swift-crypto
3.12.3 only when selected, using CryptoKit on Apple and Crypto on Linux. Apple
execution is verified; Linux crypto execution remains unverified. Java uses
JDK crypto, C# uses .NET crypto, PHP requires its hash extension, and Elixir
requires OTP 25+ crypto. Java/C#/PHP/Elixir executable probes have passed with disposable native toolchains and are repeated in CI.

Verifier return/decode semantics remain native: some return authenticated raw
bytes and offer an explicit decoder; Python/TypeScript return decoded JSON after
verification. Do not parse first and reserialize when calculating signatures.
Trusted `whsec_` keys and HMAC v1 are supported; this is not asymmetric signature
verification or durable replay storage.

## Ruby client credentials

Select `{"name":"oauth"}` beside Ruby's SDK plugin, or use `ruby::oauth()` in the
Rust package API. Require the generated `<package>/oauth` module, then configure:

```ruby
provider = MySdk::OAuthClientCredentials.new(
  token_url: 'https://auth.example.com/token',
  client_id: ENV.fetch('CLIENT_ID'),
  client_secret: ENV.fetch('CLIENT_SECRET'),
  scopes: ['read']
)
client = MySdk::Client.new(base_url: 'https://api.example.com', token_provider: provider)
```

The provider caches tokens and coordinates concurrent refreshes. Expiry, canceled
leaders/waiters and obsolete rejected-token observations are handled without
exposing credentials in errors or inspection. A buffered operation may recover
from a rejected token once when its replay-safe/idempotency policy permits; an
unkeyed POST/PATCH is not replayed. The refresh recovery has its own one-replay
bound, independent of configured transient retries. Explicit Authorization headers
remain customer-owned. Synchronous HTTP interruption is transport-owned; a
cancellation callback stops waiting or token publication. Native fake-issuer tests
cover these semantics; no live identity provider has been used.
