# @relevate/poolster-plugin-python

Select Poolster's python SDK renderer in a JavaScript config. Install this package
with `@relevate/poolster`, then add `pluginPython()` to the config's
`plugins` array. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/poolster'
import { pluginPython } from '@relevate/poolster-plugin-python'

export default defineConfig({
  input: './openapi.yaml',
  output: { path: './generated' },
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginPython({ path: 'python' })],
})
```
