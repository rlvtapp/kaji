# Generated SDKs

Every selected target is emitted as an isolated package. This lets a release
publish, version, and test language SDKs independently while all of them come
from the same API contract.

## Choose: raw API building blocks or a full SDK

This is the most important output decision.

**Raw output** gives your application the generated types and one function per
OpenAPI operation. It is best when your app already has an HTTP client,
dependency-injection setup, or API wrapper convention and only wants Kaji to
own the contract types and request serialization.

```rust
use kaji::{prelude::*, ts, generate};

let tree = generate(
    &api,
    ProfileSet::new("sdk")
        .package(ts::package("typescript").with(ts::sdk().fetch().raw())),
)?;
```

The TypeScript package exports operation functions and types:

```ts
import { getContact, type Contact } from "@relevate/email-api";

const contact: Contact = await getContact({
  client: myConfiguredClient,
  path: { contactId: "contact_123" },
});
```

There is no generated `new RelevateEmail(...)` class in raw mode. You supply
an optional configured transport client to each operation, or use the generated
default transport and per-request options. The typed request and
response definitions are still generated normally.

**Full SDK output** adds the product-style instantiated client on top of the
same raw functions and types. It is the default for TypeScript and is the
normal shape for native-language targets.

```rust
use kaji::{prelude::*, ts, generate};

let tree = generate(
    &api,
    ProfileSet::new("sdk")
        .package(ts::package("typescript")
            .with(ts::sdk().fetch().client_name("RelevateEmail"))),
)?;
```

```ts
import { RelevateEmail } from "@relevate/email-sdk";

const client = new RelevateEmail({
  baseUrl: "https://api.relevate.example",
  apiKey: process.env.RELEVATE_API_KEY,
});

const contact = await client.contacts.get({ path: { contactId: "contact_123" } });
```

The full client owns its configured base URL, credentials, retries, hooks, and
operation binding. It is the ergonomic choice for most SDK consumers.

TypeScript full clients also expose their configured `transport`. This is useful
when a generated framework helper calls the raw operation functions: pass
`client: sdk.transport`, rather than constructing a second HTTP client.

```ts
const sdk = new RelevateEmail({ baseUrl: "https://api.relevate.example" });
const query = useGetContact({ client: sdk.transport, path: { contactId: "contact_123" } });
```

Path, query, header, and body values use the generated operation's grouped
options rather than a completed URL. Ordinary TypeScript operations resolve to
the decoded success body. Pass `throwOnError: false` when the caller needs the
typed status/result envelope for declared success and error responses. Streaming
operations have a separate stream result surface.

### TypeScript transport and validation controls

The Fetch and Axios runtimes preserve declared parameter styles for path,
query, header, and cookie values, select the operation's request media type,
and return a status-discriminated response envelope internally. By default,
non-2xx responses throw `ApiError`; pass `throwOnError: false` when the caller
needs to inspect declared success and error responses by `status`.

Each result also exposes the actual `contentType`. When a response status has
multiple declared representations, `contentType` narrows `data` to that media
type's generated shape. Multipart and urlencoded bodies additionally honor
OpenAPI's per-property `encoding` settings for content type, style, explode,
and reserved characters.

Use `codecs` for representations the runtime cannot safely parse by itself,
such as XML or YAML. A codec may supply `encode` and/or `decode` and is keyed
by media type (with `*/*` as a fallback):

```ts
const client = createClient({
  codecs: {
    'application/xml': { decode: xml => parseXml(xml), encode: value => toXml(value) },
  },
})
```

Runtime validation is opt-in and uses the Standard Schema V1 interface, so it
does not require a direct Zod dependency. Generate `zod.ts` alongside the SDK,
then install its per-operation request or response validator globally or for a
single call:

```ts
import { kajiOperationSchemas } from './zod'

const client = createClient({
  validation: { response: kajiOperationSchemas.getContact.responses['200']['application/json'] },
})
```
Names and exact request fields in these examples depend on your OpenAPI document.

### Full-client layouts

The full client can be namespaced or flat:

```text
namespaced (default)  client.contacts.get({ path: { contactId } })
flat                  client.getContact({ path: { contactId } })
```

Select this with `language::sdk().flat()` or `.namespaced()` for every maintained
SDK language, including Rust. Shared defaults can use
`Common::default().client_style(SdkClientStyle::Flat)`.
In the CLI, use `--client-style flat`; TypeScript raw output uses
`--typescript-surface raw`.

## Public shapes

The default is resource namespaces. Kaji chooses the first OpenAPI tag; if an
operation has no tag, it uses a stable meaningful path segment.

```text
namespaced: client.contacts.list(...)
flat:       client.listContacts(...)
raw TS:     listContacts({ client, ... })
```

Direct operation APIs remain available in TypeScript even when the class
client is generated. Native targets retain their direct methods alongside
namespaces for callers who prefer direct operation access.

## Language packages

| Target | Package highlights | Consumer requirements |
| --- | --- | --- |
| Rust | Cargo crate, Serde models, Reqwest client | Rust and Cargo |
| TypeScript Fetch | ESM package, generated models/clients/runtime | modern Fetch-capable JS runtime or browser |
| TypeScript Axios | ESM package with Axios transport | Node/browser plus Axios package dependency |
| Go | `go.mod`, typed models, standard-library HTTP | Go 1.22+ |
| Python | `pyproject.toml`, dataclasses, standard-library HTTP | Python 3.10+ |
| PHP | `composer.json`, PSR-18/PSR-7 transport interfaces | PHP 8.2+ and Composer dependencies |
| Java | Gradle and Maven metadata, typed JDK client | Java 17+ and declared Maven dependencies |
| .NET | `.csproj`, typed `HttpClient` client | .NET SDK compatible with the generated project |
| Elixir | `mix.exs`, Finch client, typed modules | Elixir/Mix and declared Hex dependencies |

Install and compile the generated package with its own ecosystem tooling. Kaji
does not silently fetch third-party dependencies during generation.

## Common runtime contract

Kaji uses native APIs in every language. The retry policy below is shared by
the maintained SDK targets; the lifecycle ordering is the stable contract for
the TypeScript, Rust, and Go runtimes. Other targets expose their native hook
surfaces and are documented by the generated package while they converge on
this contract. This is the behavior to rely on when writing a wrapper, an
observability adapter, or a cross-language integration.

### Credentials and request lifecycle

The generated client first resolves its configured base URL, caller headers,
and OpenAPI security requirements. It then serializes the request and invokes
the transport. Static credentials are configured at client construction; use a
custom transport or the target's lifecycle hook when credentials need dynamic
refreshing or signing.

For the TypeScript, Rust, and Go hook APIs, the events have these meanings:

- `beforeRequest` / `before_request` runs once for a logical operation, after
  Kaji has assembled the request and before its first transport attempt.
- `afterResponse` / `after_response` runs once for the final HTTP response,
  including a final non-2xx response. Retryable intermediate responses are not
  reported.
- `onError` / `on_error` runs once when the final outcome is a transport,
  decoding, validation, or HTTP error. A final non-2xx response therefore
  invokes both `afterResponse` and `onError`.

Hook payloads are target-native. Treat them as observability and policy
boundaries: avoid logging headers or bodies unless your application has made a
deliberate redaction decision. Hooks must not assume they receive one event per
retry attempt.

### Retries, cancellation, and streams

All maintained SDKs default to three **total** attempts, with a 250 ms initial
delay, exponential backoff, and an 8 second cap. Set `maxAttempts` (or the
target-native equivalent) to `1` to disable retries. Kaji retries only `GET`,
`PUT`, `PATCH`, and `DELETE`, plus `POST` carrying an `Idempotency-Key` header.
It retries transport failures and HTTP `408`, `429`, `500`, `502`, `503`, and
`504`. A valid `Retry-After` value takes precedence, subject to the same cap.

In targets that expose caller cancellation/deadlines, cancellation stops
retrying. Event streams use this retry policy only while establishing the
initial connection; reconnecting after events have started is application
policy, because Kaji will not invent a resume token or `Last-Event-ID`
behavior.

### Supported runtime features

Where declared by the OpenAPI contract and supported by the target, Kaji
generates:

- operation request/response models and declared non-2xx errors;
- OpenAPI security metadata and configured credentials;
- conservative retries for safe methods and idempotent POSTs;
- declared cursor, offset/limit, or URL pagination helpers;
- SSE/event-stream and binary upload/download surfaces;
- lifecycle hooks or transport configuration in the target's idiom.

Read the generated package's `README.md` and `STYLE_GUIDE.md` first: they are
the exact API for that particular contract and target. Kaji intentionally does
not promise that two languages use identical method spellings; it aims for a
native public API in each ecosystem while retaining one behavior contract.

## Generated versus user-owned files

Treat generated models, operations, manifests, and runtime helpers as
replaceable output. Make durable changes in the OpenAPI source, configuration,
or a wrapper package.

For structured TypeScript packages, `custom/index.ts` is special: Kaji creates
it if absent and preserves it on future materialization. Use it for stable
exports, product helpers, or a small wrapper around the generated class.

## Existing npm manifests

When materializing generated output, Kaji reads an existing `package.json` and
appends missing generated requirements. Custom scripts, repository metadata,
workspace settings, and other user-owned fields are preserved. Required file
lists are combined without duplicates.

Existing dependency declarations keep their version range and category. For
example, TypeScript pinned in `devDependencies` remains pinned, and a TanStack
package already configured as an optional peer is not duplicated in
`dependencies`. Missing dependencies are added where the generator declares
them. Kaji does not infer whether a custom version range is compatible; run
your package build after generation.

The recipe's generated package name and version are authoritative, as are
matching generated export entries. Custom export entries remain available.
Invalid existing JSON or malformed dependency maps fail generation before
any generated files are written; the existing manifest is retained.

This merge happens when writing the generated tree to disk, including through
the Rust library's `GeneratedTree::write_to`. In-memory generation still
returns the standalone generated manifest. Artifact-only profiles that do not
emit a manifest do not modify an existing one.

## Per-operation documentation

The embedded compiler preserves OpenAPI operation metadata and extensions in
Kaji's normalized Rust AST. This gives documentation tooling and future package
README generation the same source contract as SDK generation. Today, rely on
the generated package README and the source OpenAPI operation for precise
request examples.
