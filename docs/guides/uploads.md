# Upload files and structured multipart parts

Declare `multipart/form-data` on the operation's request body. Kaji generates
native upload APIs for all ten SDK languages; the exact input shape differs.
Generate your contract and inspect the operation signature and emitted multipart
guide before wiring the upload.

| Target | Upload input |
| --- | --- |
| TypeScript | Generated operation input and native FormData/file values through the selected Fetch/Axios transport |
| Go | `KajiMultipartBody` with `AddText`, `AddFile`, `AddJSON` and explicit parts |
| Python | `MultipartBody`, `FilePart`, `JsonPart`, and `RawJsonPart` |
| Rust | `MultipartBody::new()` with `add_text`, `add_file`, `add_json` and `add_part`; mixed JSON operations retain a multipart companion method |
| Java | Generated multipart DTOs and file-part wrappers |
| C# | Generated multipart DTOs and file-part wrappers |
| Swift | Generated typed multipart inputs for supported closed named object roots |
| Ruby | `MultipartBody.new.add_text(...).add_file(...).add_json(...)` |
| PHP | `new MultipartBody()` with `addText`, `addFile`, `addJson` and `addPart` |
| Elixir | `MultipartBody.new()` piped through `add_text`, `add_file`, `add_json` and `add_part` |

## Explicit builders

Ruby, PHP and Elixir let you choose the part names, file names and media types.
Call an add method again with the same name to send repeated fields. Text parts
preserve false and zero; a JSON part represents JSON null explicitly. File bytes
are buffered and are not converted to JSON strings.

These builders bound the body to 64 MiB and 1,024 parts, including encoded output,
and reject newline/null injection in part metadata. They do not automatically
interpret every OpenAPI encoding style or declared per-part header. Choose the
parts required by your API and verify them against its contract.

Generated request execution encodes the body before retries so a safe replay uses
the same boundary and bytes. Multipart does not make a mutation retry-safe: the
usual method/idempotency rules still apply. See [retry safety](idempotency.md).

## Boundaries to verify

Limits and supported shapes differ between languages. Swift currently requires
supported closed named roots and rejects unsupported roots before writing files.
Java/C# typed inputs and explicit builders serve different APIs. Mixed JSON and
multipart operations need the correct native representation; a plain JSON value
is not automatically a multipart upload.

PHP operations currently need a request schema to emit a body argument. Add a
schema to a schema-less multipart contract before using its generated upload
operation. Large streaming file uploads require a custom driver or authored
operation rather than these buffered builders.

Native regression probes exercise binary and Unicode files, repeated fields,
false/zero and JSON/null values, metadata guards, limits and retry bytes. Use your
SDK's generated operation tests and a mock server to verify your actual wire shape.
