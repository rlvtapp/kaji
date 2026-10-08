# @relevate/poolster-plugin-swift

Select Poolster's swift SDK renderer in a JavaScript config. Install this package
with `@relevate/poolster`, then add `pluginSwift()` to the config's
`plugins` array. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/poolster'
import { pluginSwift } from '@relevate/poolster-plugin-swift'

export default defineConfig({
  input: './openapi.yaml',
  output: { path: './generated' },
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginSwift({ path: 'swift' })],
})
```
