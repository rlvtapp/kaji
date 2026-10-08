# @relevate/poolster-plugin-csharp

Select Poolster's csharp SDK renderer in a JavaScript config. Install this package
with `@relevate/poolster`, then add `pluginCSharp()` to the config's
`plugins` array. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/poolster'
import { pluginCSharp } from '@relevate/poolster-plugin-csharp'

export default defineConfig({
  input: './openapi.yaml',
  output: { path: './generated' },
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginCSharp({ path: 'csharp' })],
})
```
