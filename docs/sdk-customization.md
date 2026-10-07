# Customize and ship your SDK

As the SDK author, you can ship runtime policies that are active for every
customer, add helpers, or replace one operation's implementation. Keep that
source beside your recipe; Kaji includes it when it generates the package.
Customers do not have to edit generated code or register your bundled policies.

Start with the executable [bundled middleware example](../examples/bundled-middleware/README.md).
It includes a contract, recipe, policy, SDK test, and release metadata. These
APIs are in the current source tree; use a published launcher containing them
once available, or the [source build](source-customization.md).

## Choose the extension that matches your change

| Change | Use | Enabled by |
| --- | --- | --- |
| Ship a default HTTP policy with an SDK | Package `middleware` | Kaji during generation |
| Add a helper, export, or test | `customizations` with `add` | Your imports/exports/test commands |
| Replace a generated operation or file | `customizations` with `replace` | Kaji during generation |
| Make a small guarded source change | `customizations` with `patch` | Kaji during generation |
| Customer-specific HTTP behavior | Runtime middleware/driver configuration | SDK customer |
| Replace a renderer or transport provider | Typed plugin composition | SDK generator author |

Bundled middleware changes HTTP behavior. Source overlays change package files.
A generator plugin changes how those files are produced. For customer-side
registration, see [runtime middleware](guides/runtime-middleware.md).

## Bundle a policy that runs automatically

Create an author-maintained file outside the output directory, for example
`middleware/author-policy.ts`:

```ts
import type { ClientMiddleware } from '../.kaji/client'

export const authorPolicy: ClientMiddleware = async (request, next) => {
  const headers = new Headers(request.headers as HeadersInit)
  headers.set('X-SDK-Policy', 'bundled')
  return next({ ...request, headers })
}
```

The import is resolved from the destination file in the SDK, not from the source
file beside your recipe. This example uses the TypeScript SDK convenience
renderer, whose transport is `.kaji/client.ts`. Use a type-only import to avoid
creating a runtime cycle between your policy and the client that imports it.

Inside your TypeScript package entry in `kaji.json`:

```json
{
  "language": "typescript",
  "path": "typescript",
  "plugins": [{ "name": "sdk", "transport": "fetch", "client_name": "Notes" }],
  "middleware": [{
    "source": "middleware/author-policy.ts",
    "path": "middleware/author-policy.ts",
    "symbol": "authorPolicy"
  }]
}
```

Generate normally. Kaji copies the source into
`<output>/typescript/middleware/author-policy.ts` and imports it into the shared
runtime. Direct operation functions and SDK class instances use the policy.
Customers still construct the ordinary SDK:

```ts
const sdk = new Notes({ baseUrl: 'https://api.example.com' })
// No middleware option: authorPolicy is already registered.
```

The copied policy is normal owned output, not a create-once editable starter.
Edit the original source, regenerate, and review the copied source and runtime
registration together. Native build/tests verify that its export and signature
match the runtime contract.

### Configuration fields

| Field | Meaning |
| --- | --- |
| `source` | UTF-8 source file, relative to `kaji.json` |
| `path` | Destination file, relative to this SDK package's root |
| `symbol` | Exported function, factory, constant, or class; see native contracts below |
| `async_symbol` | Python async middleware function; required for async Python output |

Array order is wrapper order: the first entry is outermost. Bundled layers run
outside additional customer-configured layers. HTTP-client injection remains
available; maintained native constructors preserve authored wrappers around the
injected driver. Middleware is customization, not an enforcement boundary: a
customer controlling the SDK source can change it.

### Native source contracts

Choose the actual source directory and namespace from your generated package.
Kaji rejects unsupported layouts and collisions instead of guessing an import.
Symbols must be simple ASCII identifiers; the language may impose additional
keyword/casing restrictions.

| Language | Destination | Source contract |
| --- | --- | --- |
| TypeScript Fetch/Axios | Any portable `.ts` implementation path in the package | Named export implementing `ClientMiddleware` |
| Python | `.py` module beside generated `runtime.py`, usually `src/<package>/policy.py` | `symbol(request, next)`; optional async output requires `async_symbol(request, next)` as an async function |
| Go | Root `.go` file beside `client.go`, with the SDK's package declaration | Function with `KajiMiddleware` signature: `func(next KajiHTTPClient) KajiHTTPClient` |
| Rust | `src/<module>.rs` | Public `symbol()` factory returning a type implementing generated `Middleware` |
| Ruby | `.rb` file beside `lib/<module>/client.rb` | Top-level callable constant `symbol` accepting `(request, following)` |
| Swift | `.swift` file beside `Sources/<module>/KajiClient.swift` | Free `symbol()` factory returning `KajiMiddleware` |
| Elixir | `lib/<name>.ex` | Module `symbol` exporting `handle(request, next)` for buffered requests |
| Java | `<symbol>.java` beside `ClientBase.java` | Class in generated package with static `wrap(HttpClient)` returning a decorated `HttpClient` |
| C#/DotNet | Compiled `.cs` file in the package, outside `bin`/`obj` | Class in generated namespace with static `Wrap(HttpClient)` returning a decorated `HttpClient` |
| PHP | `.php` file under `src` | Class in generated namespace with static `wrap(ClientInterface)` returning a PSR-18 decorator |

The factory contracts deliberately use native transport interfaces. A Java
wrapper must implement the JDK client's abstract methods and asynchronous
variants; a C# wrapper must handle native request ownership when forwarding to
another client. See each plugin README for its contract and generated SDK README
for registration guidance. An optional Python dependency used only by
`async_symbol` should be imported inside that async path so sync users do not
need it merely to import the package.

Configure middleware on the portable PHP SDK when generating a Symfony wrapper.
Artifact-only packages, API CLI packages, and substituted transports with a
different ABI do not automatically support these contracts. Community language
plugins can implement `Language::bundle_middleware`; unsupported language hooks
fail explicitly.

### Test the policy before shipping it

Build the SDK and call an actual generated operation through a fake native
HTTP driver. Do not pass a middleware option in that test. Assert the request
change and the decoded result; test error recovery and short circuits when your
policy uses them. The checked-in example demonstrates this and supplies those
commands to release automation.

```sh
kaji generate --config kaji.json
kaji generate --config kaji.json --check
```

A source edit causes `--check` to report drift without writing. Regeneration
reapplies it. Removing an entry removes its registration and deletes the
unchanged owned copy. A hand-edited output copy is protected; move that change
back to its source before regenerating. See [safe regeneration](safe-regeneration.md)
for conflict diagnosis.

The automatic default-registration behavior has executable probes for
TypeScript Fetch/Axios, Python sync/async, Go, Ruby, Rust, and Swift. Java/C#/PHP
and Elixir also have source/registration checks and opt-in executable probes;
their new probes were not run on this host because toolchains were unavailable.
This is not evidence of identical runtime behavior across every language.
Elixir SSE uses its separate `stream_transport` callback and does not run these
buffered bundled layers.

## Add or replace source in one SDK

Use `customizations` for arbitrary source, tests, or explicit replacements:

```json
{
  "language": "typescript",
  "path": "web",
  "plugins": [{ "name": "sdk" }],
  "customizations": [
    { "mode": "add", "path": "extensions/helpers.ts", "source": "custom/helpers.ts" },
    { "mode": "replace", "path": "clients/contacts/listContacts.ts", "source": "custom/listContacts.ts" }
  ]
}
```

Use your actual generated operation path; casing and tag directories vary by
renderer. Another package, even one in the same language, stays unaffected.

| Mode | Requirement | Result |
| --- | --- | --- |
| `add` | Destination is not already generated | Introduces an owned file |
| `replace` | Destination exists in the current generation | Replaces the complete file |
| `patch` | Nonempty `find` text occurs exactly once | Replaces that text with the source file's contents |

A patch entry additionally sets `find`. Missing or ambiguous matches fail; they
never silently apply an outdated change. Entries execute in declaration order,
after normal plugins, language finalization, post plugins, and bundled runtime
middleware registration. All changes are assembled before the writer runs.
Ownership preflight protects existing custom work.

An added helper is not automatically imported or exported. Add an explicit
export patch, or use it from another source module you ship. Use `middleware`
instead of a plain `add` entry when the desired result is automatic runtime
registration. Bookkeeping files and create-once user modules cannot be replaced
by overlays. A full replacement owns maintenance of that file, including later
generator fixes.

## Embed the same policy in a Rust generation program

```rust
use kaji::{BundledMiddleware, prelude::*};

let package = kaji::ts::package("web")
    .with(kaji::ts::sdk())
    .middleware(BundledMiddleware {
        path: "middleware/author-policy.ts".into(),
        contents: std::fs::read_to_string("middleware/author-policy.ts")?,
        symbol: "authorPolicy".into(),
        async_symbol: None,
    });
```

`Package::middleware` scopes registration to that package. `Package::customize`
and `CodeCustomization` expose source overlays in embedded generation. Native
plugins implement the runtime bundling hook; the neutral OpenAPI AST does not
contain language-specific policy code. See [plugin authoring](typed-plugins.md)
for replacing providers and extending community language support.

## Continue to delivery

Declare package build/test commands, preview SDK CI/release scaffolding, and
publish immutable release tags through [SDK repository automation](sdk-automation.md)
and [SDK publishing](sdk-publishing.md). The authored middleware source ships
through the package's normal native build and registry artifact, alongside the
maintained runtime.
