# GraphQL → Ruby clients

Unreleased. Ruby 3.1+ gems include selected model classes, RBS signatures and a
standard-library Net::HTTP transport. No GraphQL client dependency is required.

```sh
poolster generate schema.graphql --input-format graphql \
  --operation operations.graphql --language ruby --output generated
```

Use npm `pluginRuby({ contracts: { graphql: { style: 'flat' } } })` or
Rust `ruby::graphql(Some(input.handle())).flat()` in a Ruby package.

## Calling operations

For a gem named `example_graphql` and operation `ReadUser`:

```ruby
require 'example_graphql'
client = ExampleGraphql::Client.new(endpoint, timeout: 10)
response = client.read_user('id' => '42')
puts response.data if response.status == :partial
response.require_data # raises GraphqlErrors when application errors exist
```

Raw calls `ExampleGraphql.read_user(client.transport, variables)`.
Grouped calls `client.query.read_user(variables)` or mutation groups;
custom `groups: { users: { read: 'ReadUser' } }` exposes `client.users.read(variables)`.
Generated module names follow package configuration.

Models validate required fields, nullability and wire shapes, and expose selected
getters. Nested objects, lists and named inputs have concrete model classes and
RBS signatures. `.present?(field)` distinguishes absent fields from explicit nil;
`.to_h` serializes nested models to wire values. Empty optional variables can be omitted.

Envelopes preserve errors, partial data, extensions and data-key presence.
Transport/HTTP/JSON failures remain separate. Headers and timeouts are configurable.
Custom scalars retain JSON values without codecs. Subscriptions, incremental
responses and dynamic caller-selected fields are unsupported.

Generated code and RBS were checked using Ruby 3.3.12 / RBS 3.4.0, including all
four surfaces against pinned GraphQL.js 16.14.2. See the [support matrix](../../plugin-support-matrix.md).

## Generated source layout

The gem entry point loads individual model, operation, client and group files. Corresponding RBS signatures are split too. Public module/class/method paths and nested model identities are preserved.

The layout targets source files below **128 KiB**, grouping declarations and export
parts at semantic boundaries. An indivisible model or operation that exceeds this
budget is retained and listed in `.poolster/source-layout-diagnostics.json`;
this is a size diagnostic, not a claim that every possible schema produces small files.
Regeneration removes obsolete unchanged owned files and preserves unrelated user files.
Source customizations referring to old monolithic paths must be retargeted.
