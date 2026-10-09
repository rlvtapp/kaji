# Poolster TypeScript plugin

Generate a Fetch or Axios SDK, a standalone types package, or auxiliary
validation/frontend artifacts. Generation runs entirely in Rust.

```rust
use poolster::prelude::*;
use poolster_plugin_ts::PackageExt as _;
use poolster_plugin_ts as ts;

let release = ProfileSet::new("sdk")
    .package(ts::package("typescript")
        .name("@acme/email")
        .with(ts::sdk()
            .axios()
            .client_name("Email")
            .model_options(ts::ModelOptions {
                enum_type: ts::EnumType::AsConst,
                ..Default::default()
            })));
let tree = poolster::generate(&api, release)?;
tree.write_to("generated")?;
```

Fetch is the default transport. `.raw()` removes the instantiated class but
keeps direct operations and models. Full clients default to namespaced;
`.flat()` selects direct class methods. Transport choices belong to the SDK
plugin; use separate packages for Fetch and Axios.

`ts::types()` emits models and publishes the `TsTypes` symbol contract for
community consumers. Do not combine it with `ts::sdk()` in one package.

`ts::artifacts` exposes Zod, TanStack React/Vue Query, SWR, Faker, MSW, Cypress,
ReDoc, and MCP manifest renderers. These return files directly, not ready-made
typed plugins. They are not CLI targets. Review their dependencies, import
configuration, and limitations before using generated output.

- [Every SDK/model option](../../../docs/reference/configuration/configuration.md#typescript-sdk)
- [Raw and full client usage](../../../docs/reference/outputs/generated-sdks.md)
- [Auxiliary artifact API and limitations](../../../docs/reference/outputs/auxiliary-generators.md)
- [Contracts and plugin composition](../../../docs/internals/typed-plugins.md)

## Composable SDK providers

`composition` exposes model, transport, operation and flat client providers.
They reuse the maintained SDK rendering implementation; `sdk()` keeps its
existing layout and publishes the same model, transport and operation contracts.
Query consumers resolve actual published symbols rather than constructing imports.

```rust
use poolster_plugin_typescript::{self as ts, composition as c};
use poolster_core::engine::Packages;

let models = c::models().output("domain/models");
let runtime = c::transport().output("runtime/request");
let calls = c::operations()
    .using_models(models.models_handle())
    .using_transport(runtime.transport_handle())
    .output("api/calls");
let client = c::client()
    .using_operations(calls.operations_handle())
    .using_transport(runtime.transport_handle())
    .client_name("ContactsClient");
let queries = c::react_query()
    .using_operations(calls.operations_handle())
    .output("ui/queries");
let packages = Packages::new().package(ts::package("sdk")
    .with(models).with(runtime).with(calls).with(client).with(queries));
```

Providers may be registered in any order. Select handles explicitly when more
than one provider publishes a contract. Model options belong to the selected
model provider and are propagated to the operation provider.

A community transport implements `Plugin<TypeScript>`, declares
`Provision::of::<composition::Transport>()`, emits its runtime module, and calls
`cx.publish(composition::Transport::new("my/runtime"))`. Its module
must implement the documented runtime ABI, including `Options`, `RequestResult`,
`ResponseResult`, `ClientConfig`, `ClientInstance`, `client`, `createClient`,
`resolveResponse`, and streaming support for APIs that use streams. The operation
and client providers import that module directly. `Workspace::dependency` and
`Workspace::export` register dependencies and public modules before finalization.

React Query, Vue Query and SWR are available as typed consumer plugins. For
example, `sdk().raw()` can be combined with `composition::vue_query()` without
changing the SDK's existing grouped operation layout.

The composable client supports `.namespaced()` as well as its default flat
facade. Zod, Faker, MSW and Cypress are native consumers through
`composition::{zod,faker,msw,cypress}`; `.using_models(handle)` selects their model
provider, and MSW/Cypress also accept `.using_operations(handle)`. Faker imports
actual schema symbols, including custom aliases. Validators and factories follow
selected integer representations and optional-property removal.

Composed models, operations, transports and auxiliary modules are exported in
namespaces derived from their output module paths. For example, `api/calls`
becomes `apiCalls`, and `hooks/react` becomes `hooksReact`. This allows multiple
providers and query frameworks to coexist without duplicate root exports. The
complete `sdk()` retains its existing root export layout. Framework packages are
peer dependencies; Cypress is a development dependency. Faker is a runtime
dependency because generated factory exports import it.

## Lossless integer JSON

Select `ModelOptions { int64_type: Int64Type::BigInt, ..Default::default() }` or
`Int64Type::String` for OpenAPI integers with `format: int64`. The default remains
`Number`. The existing `integer_as_string` option applies to every integer schema.

Fetch and Axios transports parse numeric tokens before converting them to
JavaScript values. Schema-directed plans convert only selected integer fields,
including nested arrays, references, unions, additional properties, errors and
SSE events. Number schemas and ordinary integers retain numeric values. Axios
preserves raw response text before its default JSON transformer can round digits.
Requests encode string/bigint integer fields as JSON numeric tokens. Unsafe
numeric inputs are rejected for fields that require exact integer encoding.
Custom JSON codecs retain precedence and are responsible for their own precision.

A custom transport must advertise `Transport::new("my/runtime").lossless_json(true)`
when it implements the generated JSON-plan ABI. Selecting an exact integer model
representation with an incompatible transport produces a generation diagnostic.
The runtime also exports `parseJson` and `stringifyJson` for wrappers and tests.
Numeric enum/const values use corresponding string or bigint literals; native
TypeScript bigint enums are represented as `as const` objects and type aliases.

Structural tests cover provider selection, relocated modules and unsupported
capabilities. Opt-in compiler/runtime tests use existing local dependencies:

```sh
POOLSTER_TSC_JS=/absolute/path/to/typescript/lib/tsc.js \
POOLSTER_TS_NODE_MODULES=/absolute/path/to/consumer/node_modules \
POOLSTER_AXIOS_NODE_MODULES=/absolute/path/to/axios-consumer/node_modules \
cargo test -p poolster-plugin-typescript --lib -- --ignored
```

The auxiliary dependency directory needs React/Vue Query, SWR, Zod, Faker, MSW
and Cypress for the full consumer test. The executable precision test covers
Fetch/Axios responses, request encoding and SSE at signed 64-bit boundaries.

### Runtime middleware

Generated Fetch and Axios clients accept `middleware` in `ClientConfig`, including
through the SDK class constructor. Import its types from the generated
`.poolster/client` module for the SDK convenience recipe (or the configured
transport output directory). SDK packages also export these types and
`createClient` from their entrypoint.

```ts
import { createClient, type ClientMiddleware, type MiddlewareResponse } from './.poolster/client'

const customerPolicy: ClientMiddleware = async (request, next) => {
  try {
    const result = await next({
      ...request,
      query: { ...request.query, tenant: 'customer-a' },
    }) as MiddlewareResponse
    return { ...result, data: normalizeCustomerResponse(result.data) }
  } catch (cause) {
    throw new Error('Customer API request failed', { cause })
  }
}

const transport = createClient({ middleware: [customerPolicy] })
// The same config works with either the generated Fetch or Axios transport.
const sdk = new GeneratedSdk({ middleware: [customerPolicy] })
```

`normalizeCustomerResponse` and `GeneratedSdk` above represent your application
function and generated SDK class. Request changes passed to `next` take effect
before authentication, URL construction, serialization and request validation.
Middleware can rewrite method, URL, body, query, headers, or request options.
For Fetch headers, preserve native headers with `new Headers(request.headers as
HeadersInit)` when adding a header; Axios uses a header record.

The first middleware is outermost. Each handler calls `next(updatedRequest)` at
most once, awaits it, and can replace its result or catch and replace its error.
Returning without calling `next` short circuits the remaining middleware,
transport, legacy hooks and validators. For example, a cache can return an
ordinary `{ status, contentType, data, headers }` response envelope directly.
Generated operation unwrapping and status handling continue to consume that
envelope. A middleware producing a synthetic response owns its validity and
must preserve the operation's expected shape.

Middleware runs **once per logical SDK call**. Built-in retry attempts execute
inside `next`; request middleware is not repeated for each attempt, and response
middleware receives the final outcome. Legacy `beforeRequest`, `afterResponse`
and `onError` hooks retain their existing timing inside the transport. Errors
raised by outer middleware do not invoke transport hooks. Python and Go runtime
middleware use their documented per-attempt transport semantics instead.

Fetch SSE calls return a native `Response`; Axios SSE calls return an envelope
whose `data` is a readable stream. Preserve that representation when wrapping a
stream result. Consume or tee a stream deliberately; middleware that reads its
body can exhaust it. The middleware list is copied when creating a client, and
request path/query/header/cookie containers are copied for each invocation.
Bodies and nested values remain shared; pass replacement objects when modifying
them. Existing hooks remain available for observation alongside middleware.

Other language runtimes can use their native transport extension points: C#
`DelegatingHandler`, a Java injected `HttpClient`, and PHP PSR-18 decorators.

The opt-in `generated_middleware_fetch_and_axios_execute` test strictly compiles
both generated runtimes and a typed consumer, then executes request/response
rewrites, error replacement and recovery, retry ordering, short circuits,
duplicate-next protection, and streaming through both transports.
