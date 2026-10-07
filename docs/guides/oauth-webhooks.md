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
requires OTP 25+ crypto. Java/C#/PHP/Elixir executable probes await native CI.

Verifier return/decode semantics remain native: some return authenticated raw
bytes and offer an explicit decoder; Python/TypeScript return decoded JSON after
verification. Do not parse first and reserialize when calculating signatures.
Trusted `whsec_` keys and HMAC v1 are supported; this is not asymmetric signature
verification or durable replay storage.
