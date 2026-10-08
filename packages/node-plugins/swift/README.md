# @relevate/kaji-plugin-swift

Select Kaji's swift SDK renderer in a JavaScript config. Install this package
with `@relevate/kaji`, then add `pluginSwift()` to the config's
`plugins` array. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/kaji/sdk'
import { pluginSwift } from '@relevate/kaji-plugin-swift'

export default defineConfig({
  input: './openapi.yaml',
  output: { path: './generated' },
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginSwift({ path: 'swift' })],
})
```
