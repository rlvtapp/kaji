# Generate SDKs from OpenAPI 3.2

Use the normal `kaji generate --config kaji.json` workflow with an OpenAPI 3.2 contract.
No alternate parser or plugin registration is required. Custom generation plugins can
read the same typed transport metadata as the bundled SDKs.

## Whole-query parameters

A parameter with `in: querystring` describes the complete query, rather than a named
field inside it. It has one `content` entry and cannot coexist with another whole-query
or ordinary query parameter on the same operation.

```yaml
parameters:
  - name: filter
    in: querystring
    required: true
    content:
      application/x-www-form-urlencoded:
        schema:
          type: object
          properties:
            tag: { type: array, items: { type: string } }
```

TypeScript accepts `querystring: { filter: { tag: ["a b", "x&y"] } }` and emits
`?tag=a+b&tag=x%26y`. Go and the dynamic Ruby/PHP/Elixir drivers support content-aware
structured values or an explicitly serialized query.

Rust, Java, C#, Swift and Python expose a serialized string argument: pass
`"tag=a+b&tag=x%26y"`, preserving repeated keys and escaped values. These APIs reject
URL fragments and line breaks; they do not encode the whole string again.

For JSON content, pass a structured value to content-aware APIs or serialize and
percent-encode it before passing a raw-query string. Ordinary named parameters with
`content: application/json` use their generated content serializer for path, query,
header and cookie values.

## Sequential JSON

Declare `itemSchema` for each JSON record:

```yaml
responses:
  '200':
    description: Events
    content:
      application/json-seq:
        itemSchema:
          $ref: '#/components/schemas/Event'
```

The native buffered APIs encode and decode arrays record by record for JSON sequences
and NDJSON/JSONL. JSON sequences use a record separator followed by a JSON value and
newline; NDJSON uses one JSON value per line. Malformed records fail the response rather
than returning a partially decoded array.

TypeScript's lossless integer policy also applies to record items.

`text/event-stream` uses the existing native event iterator and the item schema
describes one event payload. An `itemSchema` does not enable streaming for every
arbitrary media type. Custom formats need a native codec or application decoding.

Buffered sequences have explicit limits; use the generated package documentation and an
appropriate event-stream API for a long-lived feed.

## Ordered and nested multipart

Use `prefixEncoding` for the leading parts and `itemEncoding` for subsequent parts.
Nested Encoding Objects can declare another multipart body. Named `encoding` and
positional encodings are mutually exclusive at the same level.

Generators carry the declaration into the operation's native encoder. For languages with
explicit builders, add parts in their intended wire order:

| Target | Ordered input |
| --- | --- |
| TypeScript | An array with the generated operation's multipart plan; Blob/File values retain buffered binary data. |
| Python | `MultipartBody.positional(values)`; nest another body and use `with_headers(part, headers)` for supplied part headers. |
| Go | `KajiMultipartBody` parts; `Nested` holds a child body. |
| Rust | `MultipartBody` with `add_part_with_headers` and `add_nested`. |
| Java | Ordered multipart parts supplied to the generated operation. |
| C# | `OrderedMultipartPart` values with optional `Nested`. |
| Swift | The operation's generated ordered upload body and `KajiOrderedPart` values. |
| Ruby / PHP / Elixir | The native `MultipartBody` builder, including a child body as a part. |

The generated operation applies media types and encoding plans. Supply required MIME
part headers through the builder or transport's part-header input; they are distinct
from HTTP request headers. Explicit builders accept serialized part payloads, so a byte
part is not automatically converted into JSON or split into additional fields.

Nested bodies retain their boundaries and media subtype.

Multipart response bodies are delivered for native decoding; the generator does not turn
arbitrary MIME parts into typed JSON model arrays.

## Metadata and reference identity

`kaji_core::openapi32` exposes typed request, response and parameter content, recursive
encodings and API metadata for custom plugins. The security catalog retains the device
authorization endpoint and OAuth metadata URL. Tag summaries, parents and kinds remain
available without changing your package's plugin graph.

The compiler honors `$self` as the document's reference identity and base URI. Keep it
consistent with the location of relative child documents. Compiler caches include the
artifact format revision, while provenance hashes retain source-byte meaning. See [the
compiler guide](../openapi-compiler.md) for source limits and custom-format boundaries.
