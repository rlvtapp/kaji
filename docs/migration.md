# Move an existing SDK project to Kaji

[Quickstart](cli/quickstart.md) · [Configure packages](cli/config.md) · [Feature catalog](features.md)

Use the configuration and OpenAPI files you already maintain for Stainless, Fern
or Speakeasy. You do not need to start with a blank Kaji recipe. Kaji imports
supported settings and reports the rest for review. Generated SDK interfaces and
runtime behavior follow Kaji's native generators; this is not a promise of
source-compatible replacement for an existing SDK.

## Try generation from your existing project

Build the current Kaji sources as described in [source customization](source-customization.md).
From the existing project directory:

```sh
kaji generate --config stainless.yml
kaji generate --config fern/generators.yml
kaji generate --config .speakeasy/workflow.yaml
```

Each command creates SDKs under `kaji-generated` in the project root.
It reads
and bundles the source contract into temporary storage, prints settings that
need review, and preserves your existing configuration and SDK output.
It does
not execute vendor hooks or publish packages.
With no `kaji.json`, `kaji generate`
can detect a single vendor configuration automatically.

If several tools are
present, pass the configuration filename explicitly.
A Speakeasy workflow takes
precedence over its companion `gen.yaml` during detection.

## Save an editable migration

```sh
kaji migrate . --output ./kaji-project
# Select the input explicitly if the vendor configuration does not identify it:
kaji migrate stainless.yml --input ./specs/api.yaml --output ./kaji-project
kaji generate --config ./kaji-project/kaji.json
```

The output must be a new directory. Migration writes:

- `kaji.json`: native language packages and supported package names.
- `openapi.json`: the contract with bundled references and translated annotations.
- `migration-report.json`: converted settings and items requiring manual review.

Original files remain intact.
Local referenced files are bundled so moving the
output does not break their relative paths.
Generator versions are never used as
SDK package versions; imported SDKs start at `0.1.0`.
Edit that version before
shipping an existing package.
Credentials and vendor publication settings are
not copied into the recipe or report.

Configure delivery through [SDK automation](sdk-automation.md)
and [publishing](sdk-publishing.md).

Use `--strict` to reject migrations with any manual-review items before creating
the output directory. The default produces a reviewable partial conversion.
Remote inputs, multiple merged specs, source graphs and overlays must first be
exported to one resolved OpenAPI file and supplied with `--input`. Fern Definition
files and custom vendor generators need an OpenAPI export or a Kaji plugin.

## What is imported

| Source | Supported settings |
| --- | --- |
| Stainless | Native SDK targets, `package_name`, resource/subresource methods using explicit `get /path` bindings, dotted `x-stainless-method` strings or their `path` field. |
| Fern | `api.specs[].openapi`, supported `fern-<language>-sdk` generators, output package names, method/group names, ignored operations, simple query cursor pagination, and one declared idempotency header for marked endpoints. |
| Speakeasy | Workflow targets pointing to one local source, target languages, standalone `gen.yaml` package names, method/group names, ignored operations, and portable pagination declarations. |

Supported SDK languages are TypeScript, Python, Go, Rust, Ruby, PHP, Java, C#,
Swift and Elixir. Unsupported target types are listed for review. Imported
packages include the native SDK plugin; add other Kaji plugins or bundled runtime
middleware through [configuration](configuration.md).

Kaji also recognizes these operation annotations during ordinary generation from
an OpenAPI file. You can keep supported vendor annotations while moving your
workflow incrementally. Explicit `x-kaji-pagination` and `x-kaji-idempotency`
settings take precedence over their translated equivalents. Wire parameter names
and paths remain unchanged. Conflicting names and unsupported behavior are
reported; duplicate SDK operation names fail generation.

Fern cursor conversion currently requires direct `$request.<query-parameter>`,
`$response.<results>` and `$response.<next_cursor>` bindings.
Offset/step rules,
body-bound cursors and relative next-page URLs require an explicit
[portable pagination declaration](guides/pagination.md).
Stainless's automatic
pagination scheme matching, vendor retry policies, model/property naming,
streaming conventions, examples, custom templates and proprietary generators
are not automatically reproduced.

Review the migration report and native
[feature catalog](features.md), then test your generated SDK before switching
customers.
Unknown vendor operation annotations are preserved and diagnosed.

The import formats follow the official [Stainless configuration reference](https://www.stainless.com/docs/reference/config/),
[Fern generator configuration](https://buildwithfern.com/learn/sdks/reference/generators-yml)
and [Speakeasy workflow reference](https://www.speakeasy.com/docs/speakeasy-reference/workflow-file).
