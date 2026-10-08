# @relevate/poolster-plugin-typescript

Select Poolster's typescript SDK renderer in a JavaScript config. Install this package
with `@relevate/poolster`, then add `pluginTypeScript()` to the config's
`plugins` array. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/poolster'
import { pluginTypeScript } from '@relevate/poolster-plugin-typescript'

export default defineConfig({
  input: './openapi.yaml',
  output: { path: './generated' },
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginTypeScript({ path: 'typescript' })],
})
```
