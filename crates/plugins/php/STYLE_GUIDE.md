# PHP SDK client styles

Kaji supports two public styles:

- `SdkClientStyle::Flat` preserves direct operations such as
  `$client->getContact($id)`.
- `SdkClientStyle::Namespaced` additionally exports resource accessors such as
  `$client->contacts()->get($id)`. Direct operations remain available for
  incremental migration.

Use `generate_php_sdk_with_style(api, output_dir, package_name, style)` to
choose. Each generated package also contains this choice in its own
`STYLE_GUIDE.md`.
