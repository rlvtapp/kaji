# @relevate/kaji-plugin-zod

Select Kaji's compiled Rust zod plugin from a JavaScript config. Install
with `@relevate/kaji` and `@relevate/kaji-plugin-typescript`, then
list both factories in `plugins`. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/kaji/sdk'
import { pluginTypeScript } from '@relevate/kaji-plugin-typescript'
import { pluginZod } from '@relevate/kaji-plugin-zod'

export default defineConfig({
  input: './openapi.yaml',
  output: './generated',
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginTypeScript(), pluginZod({ target: 'typescript' })],
})
```

The `target` names the TypeScript SDK package path; use it when the package
path differs from the default or multiple TypeScript packages are selected.
