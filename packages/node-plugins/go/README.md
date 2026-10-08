# @relevate/kaji-plugin-go

Select Kaji's go SDK renderer in a JavaScript config. Install this package
with `@relevate/kaji`, then add `pluginGo()` to the config's
`plugins` array. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/kaji/sdk'
import { pluginGo } from '@relevate/kaji-plugin-go'

export default defineConfig({
  input: './openapi.yaml',
  output: { path: './generated' },
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginGo({ path: 'go' })],
})
```
