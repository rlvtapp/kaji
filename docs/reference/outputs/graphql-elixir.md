# GraphQL → Elixir clients

Unreleased. Mix packages use Finch 0.24.0 and Jason 1.4.5, with typed structs for
selected results and operation variables.

```sh
poolster generate schema.graphql --input-format graphql \
  --operation operations.graphql --language elixir --output generated
```

Use npm `pluginElixir({ contracts: { graphql: { style: 'flat' } } })` or
Rust `elixir::graphql(Some(input.handle())).flat()` in an Elixir package.

## Calling operations

For a package module `Example`:

```elixir
client = Example.Client.new(endpoint)
variables = %Example.Models.ReadUserVariables{id: "42"}
{:ok, response} = Example.read_user(client, variables)
# Inspect response.errors and response.data together before requiring clean data.
Example.Runtime.require_data(response)
```

Raw exposes `Example.Operations.read_user(client, variables)`.
Grouped exposes `Example.Query.read_user(client, variables)` and mutation modules.
Custom `groups: { users: { read: 'ReadUser' } }` exposes `Example.Users.read(...)`.
This module-based grouping follows Elixir conventions rather than object methods.

Optional struct fields use `:poolster_absent` for omission and `nil` for explicit
null. Required fields and wire scalar/list shapes are validated. Responses preserve
data-key presence, errors, extensions and HTTP status; `require_data` returns an
error containing the envelope when GraphQL errors exist. Transport failures return
error tuples independently. Headers, timeout, Finch instance and transport are configurable.

Custom scalars retain JSON terms and support direction-specific `scalar_codecs`
callbacks. Abstract variants require selected `__typename`. Opt-in subscriptions
use distinct-connection graphql-sse; incremental inputs use experimental multipart
deferSpec=20220824. Dynamic selections remain unsupported.
See the [support matrix](../../plugin-support-matrix.md) for the verification boundary.

## Advanced capabilities

See [subscriptions, scalar callbacks and incremental delivery](graphql-capabilities.md)
for opt-in configuration, native stream lifetime and the tested protocol boundary.
These additions are unreleased; historical checks below predate them.

## Generated source layout

Mix source files separate runtime, client, application, envelope, individual models and operations. Public facade/group exports use bounded parts. Existing public module, struct and function names are preserved.

The layout targets source files below **128 KiB**, grouping declarations and export
parts at semantic boundaries. An indivisible model or operation that exceeds this
budget is retained and listed in `.poolster/source-layout-diagnostics.json`;
this is a size diagnostic, not a claim that every possible schema produces small files.
Regeneration removes obsolete unchanged owned files and preserves unrelated user files.
Source customizations referring to old monolithic paths must be retargeted.
