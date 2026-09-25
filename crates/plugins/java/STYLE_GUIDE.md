# Java SDK client styles

Kaji supports two public styles:

- `SdkClientStyle::Flat` keeps operations on `Client`, for example
  `client.getContact(input)`.
- `SdkClientStyle::Namespaced` additionally exports resource accessors, for
  example `client.contacts().get(input)`. Direct operations remain available
  for source-compatible migration.

Use `generate_java_sdk_with_style(api, output_dir, package_name, style)` to
choose. Each generated package includes its selected style in `STYLE_GUIDE.md`.
