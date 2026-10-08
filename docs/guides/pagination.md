# Declare pagination and use a native pager

Kaji already generates pagination through each language's normal iteration API. A Rust
stream, a PHP generator and a C# async enumerable can implement the same contract
without sharing a runtime or public interface. A helper yields whole response pages; it
does not automatically flatten the results into individual items.

## Declare the continuation explicitly

Annotate an operation using `x-kaji-pagination` or its compatible
`x-speakeasy-pagination` spelling. A page-number declaration looks like:

```yaml
x-kaji-pagination:
  type: page
  inputs:
    - name: page
      in: parameters
      type: page
    - name: limit
      in: parameters
      type: limit
  outputs:
    results: /items
```

Declare `page` and `limit` as integer operation parameters and describe the response's
`items` array in its JSON schema. The optional limit can be omitted from the extension.
Optional page controls start at 1; a supplied zero or other valid starting value is
preserved. Required controls retain their required public signatures.

Each continuation calls the original operation, keeping its authentication, middleware,
serialization, errors and HTTP driver.

Page-number iteration stops on an empty result array, or a result array shorter than a
positive declared limit. Helpers stop or report numeric overflow; some targets also
apply a 10,000-page guard. Do not supply an invalid or nonpositive limit. Existing
offset/page contracts that use `numPages` retain their documented behavior.

Selectors support declared object fields and array indices. Portable forms include
`$.items`, `$[0].items[-1]` and RFC 6901 pointers such as `/data/items` or
`/a~1b/~0items/0`. Pointer array indices are nonnegative canonical integers; negative
array indices belong to the supported JSONPath syntax. Wildcard, filter and executable
expressions are unsupported.

Consult the target limits below for legacy paginator selector differences.

## Keep each language's native API

| Target | Generated iteration surface | Declared forms |
| --- | --- | --- |
| TypeScript | Async generator, `<operation>Pages` | Cursor, page, offset/limit, next URL |
| Go | Pager with `Next(ctx)` | Cursor, page, offset/limit, next URL |
| Python | Generator or async generator, `<operation>_pages` | Cursor, page, offset/limit, next URL |
| Rust | Lazy `Stream`, `<operation>_pages` | Cursor, page, offset/limit, absolute same-origin URL |
| Java | Lazy `Iterable`, `<operation>Pages` | Cursor, page, offset/limit, next URL |
| C# | `IAsyncEnumerable`, `<operation>PagesAsync` | Cursor, page, offset/limit, absolute same-origin URL |
| PHP | `Generator`, `<operation>Pages` | Cursor, page, offset/limit, next URL |
| Ruby | `Enumerator`, `<operation>_pages` | Page |
| Swift | Native async sequence, `<operation>Pages` | Cursor, page, offset/limit, absolute same-origin URL |
| Elixir | Lazy `Stream`, `<operation>_pages` | Cursor, page, offset, absolute same-origin URL |

### Supported bindings

These are capability forms, not a guarantee that every schema/control binding can be
represented. Page helpers require buffered JSON response arrays selected by the
declaration. Native parameter controls are supported where representable; TypeScript,
Python, PHP and Ruby also support their documented JSON body controls.

Body controls remain unsupported in Rust, Go, Java, C#, Swift and Elixir page helpers.
Java and C# page/offset helpers currently require integer query controls; C# also
handles `int32` counters with checked conversion. Other listed parameter bindings vary
by target and are diagnosed when unsupported.

Primitive/reference and nullable control restrictions vary by target. Unsupported
declarations are reported through generation errors or package pagination diagnostics in
the targets that provide them; ordinary operation methods remain the escape hatch for
explicit application-managed iteration.

New page helpers use the shared validated declaration plan. Legacy cursor, offset and
URL renderers retain target-specific restrictions; their migration is separate from
adding a shared public runtime. URL continuation helpers must keep credentials on the
configured API origin.

## Verify the behavior you ship

The shared SDK fixture now includes a page-number operation and asserts that all ten
targets emit its helper. The native CI matrix compiles those fixtures. Language tests
exercise defaults, caller-supplied zero, response selectors, termination, immutable
caller inputs and invalid controls where supported.

Java, C#, PHP and Elixir execution remains dependent on their CI toolchains; generation
assertions alone do not prove native runtime behavior.

See [testing generated SDKs](testing.md) and the [runtime contract
coverage](../../packages/runtime-contract/README.md).

Swift cursor helpers accept unconstrained string parameter controls, preserve caller
arguments and middleware, and stop on empty/missing or repeated cursors. Body and
integer cursor controls remain unsupported. Page and cursor iterators are lazy and
retain Swift task cancellation.

Elixir offset helpers advance by the actual result count and preserve caller options.
URL helpers accept absolute HTTP(S) continuations on the configured origin; relative
URLs, userinfo, fragments, scheme/host/port changes are rejected before authentication
or transport. Repeated continuations stop iteration and a page cap bounds traversal.

Body controls remain unsupported. Native execution of these additions is selected in CI
and requires Elixir.

Rust, C# and Swift URL helpers accept absolute same-origin HTTP(S) URLs only. They
reject userinfo, fragments and origin changes before authentication or middleware. Swift
offset controls require inline nonnullable integer parameters; reference and body
controls remain unsupported.
