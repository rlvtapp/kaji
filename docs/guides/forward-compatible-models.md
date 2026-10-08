# Models that survive API evolution

An API can add an enum value or object property before every customer upgrades their
SDK. Choose model policies when generating the SDK, then test decoding and re-encoding
actual responses. Runtime middleware is independent of these policies.

## Keep known enum values and accept future values

Enable `open_enums` on the SDK plugin when its generated type needs an open enum:

```json
{
  "language": "rust",
  "path": "sdk/rust",
  "plugins": [{ "name": "sdk", "open_enums": true, "open_unions": true }]
}
```

`open_enums` is available for TypeScript, Python, Rust, Java, C# and Swift. It is an
author setting: customers install the resulting package without configuring the
generator. Go string enums already accept arbitrary wire strings. Ruby and Elixir retain
their decoded scalar values.

PHP keeps known backed enum instances and falls back to the backing scalar for an
unknown value.

| Target | Open enum representation |
| --- | --- |
| TypeScript | Known literal members plus the backing primitive. `Enum` and `ConstEnum` styles become a const value object with an extensible type of the same name. |
| Python | Known `Literal` members plus the backing primitive in type annotations. Wire decoding retains the scalar. |
| Rust | String wrapper with known constants and the original unknown string. |
| Java / C# | Native open string wrappers selected by the SDK plugin. |
| Swift | Known cases and an unknown case carrying the original string. |

These changes affect public model types. Keep the default when a closed enum is
intentional; enable openness before publishing a package whose API must tolerate new
server values. Numeric and boolean enum representations retain their backing wire type
rather than converting those values to strings.

## Preserve an omitted field separately from null

An optional nullable field has three states: omitted, present with null, and present
with a value. PATCH APIs often assign different behavior to each state.

Java and C# offer `"preserve_presence": true` on the SDK plugin. Java uses
`Presence.of(null)` for explicit null and an absent wrapper for omission. C# uses
`Presence<T>.Present(null)` and the default wrapper for omission. This changes optional
property types.

### Presence in each runtime

Rust uses its generated nullable-presence codec with nested options. TypeScript keeps an
absent object property distinct from an explicit `null`. Python, Ruby, PHP, Go, Swift
and Elixir retain decoded presence through their generated model codecs.

Use the generated codec when roundtripping a model; constructing a plain dictionary or
changing properties can deliberately change its presence. Python exports
`with_present_fields(model, "wire-name")` to copy a newly constructed model and
explicitly send a nullable field as null.

The helper validates declared wire names and leaves the original model untouched. Ruby
provides `model.with_present_fields("wire-name")`, PHP provides
`$model->withPresentFields(["wire-name"])`, and Elixir provides
`Models.Example.with_present_fields(model, ["wire-name"])`. These helpers copy the model
and preserve its existing presence information.

## Unknown object fields and union variants

Generated object codecs preserve properties allowed by the schema, including extension
bags in languages that require named storage. A schema with `additionalProperties:
false` does not grant permission to preserve arbitrary fields as part of its typed
contract.

Rust's `open_unions` enables a raw JSON fallback for unmatched union values. Other
targets retain unknown union payloads through their native JSON or transparent union
representation. This preserves JSON values; it does not promise that an unknown variant
satisfies the schema or becomes a known typed variant.

## Test the delivered model contract

Include nested fixtures containing a known enum, a future enum value, an unknown union
branch, a schema-allowed extra property, a missing nullable field, explicit null, zero
and false. Decode them with the generated SDK and encode them again. Compare parsed JSON
objects, since property order is not significant.

Also compile a consumer using the generated model types. A successful JSON roundtrip
alone does not prove that the language's enum type accepts a future value. See [testing
generated SDKs](testing.md) and [verification](../verification.md).
