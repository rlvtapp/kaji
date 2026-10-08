# @relevate/kaji-plugin-cypress

Select Kaji's compiled Rust cypress plugin from a JavaScript config. Install
with `@relevate/kaji` and `@relevate/kaji-plugin-typescript`, then
list both factories in `plugins`. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/kaji/sdk'
import { pluginTypeScript } from '@relevate/kaji-plugin-typescript'
import { pluginCypress } from '@relevate/kaji-plugin-cypress'

export default defineConfig({
  input: './openapi.yaml',
  output: './generated',
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginTypeScript(), pluginCypress({ target: 'typescript' })],
})
```

The `target` names the TypeScript SDK package path; use it when the package
path differs from the default or multiple TypeScript packages are selected.
