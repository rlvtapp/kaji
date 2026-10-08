# @relevate/kaji-plugin-typescript

Select Kaji's typescript SDK renderer in a JavaScript config. Install this package
with `@relevate/kaji`, then add `pluginTypeScript()` to the config's
`plugins` array. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/kaji/sdk'
import { pluginTypeScript } from '@relevate/kaji-plugin-typescript'

export default defineConfig({
  input: './openapi.yaml',
  output: { path: './generated' },
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginTypeScript({ path: 'typescript' })],
})
```
