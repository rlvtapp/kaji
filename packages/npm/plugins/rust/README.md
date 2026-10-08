# @relevate/poolster-plugin-rust

Select Poolster's rust SDK renderer in a JavaScript config. Install this package
with `@relevate/poolster`, then add `pluginRust()` to the config's
`plugins` array. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/poolster'
import { pluginRust } from '@relevate/poolster-plugin-rust'

export default defineConfig({
  input: './openapi.yaml',
  output: { path: './generated' },
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginRust({ path: 'rust' })],
})
```
