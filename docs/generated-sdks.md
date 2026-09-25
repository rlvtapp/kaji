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
use kaji::{ProfileSet, TypeScriptOptions, generate};

let tree = generate(
    &api,
    ProfileSet::new("sdk")
        .typescript_fetch()
        .typescript_options(TypeScriptOptions::raw()),
)?;
```

The TypeScript package exports operation functions and types:

```ts
import { getContact, type Contact } from "@relevate/email-api";

const contact: Contact = await getContact({
  client: myConfiguredClient,
  contactId: "contact_123",
});
```

There is no generated `new RelevateEmail(...)` class in raw mode. You supply
the configured transport client to each operation. The typed request and
response definitions are still generated normally.

**Full SDK output** adds the product-style instantiated client on top of the
same raw functions and types. It is the default for TypeScript and is the
normal shape for native-language targets.

```rust
use kaji::{ProfileSet, TypeScriptOptions, generate};

let tree = generate(
    &api,
    ProfileSet::new("sdk")
        .typescript_fetch()
        .typescript_options(TypeScriptOptions {
            client_name: Some("RelevateEmail".into()),
            ..TypeScriptOptions::default()
        }),
)?;
```

```ts
import { RelevateEmail } from "@relevate/email-sdk";

const client = new RelevateEmail({
  baseUrl: "https://api.relevate.example",
  apiKey: process.env.RELEVATE_API_KEY,
});

const contact = await client.contacts.get({ contactId: "contact_123" });
```

The full client owns its configured base URL, credentials, retries, hooks, and
operation binding. It is the ergonomic choice for most SDK consumers.

### Full-client layouts

The full client can be namespaced or flat:

```text
namespaced (default)  client.contacts.get({ contactId })
flat                  client.getContact({ contactId })
```

For TypeScript select this with `TypeScriptOptions::flat_client()` or set
`client_style: SdkClientStyle::Namespaced`. For Go, Python, PHP, Java, .NET,
and Elixir choose `PackageOptions::flat()` or set `client_style` directly.
Rust currently emits its maintained Reqwest client surface from its fixed
profile; it does not expose this choice through `ProfileSet` yet.

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
namespaces so an SDK can migrate without a breaking rewrite.

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

## Common runtime behavior

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

## Per-operation documentation

Kaji preserves OpenAPI operation metadata and extensions in its normalized
Rust AST. This gives documentation tooling and future package README generation
the same source contract as SDK generation. Today, rely on the generated
package README and the source OpenAPI operation for precise request examples.
