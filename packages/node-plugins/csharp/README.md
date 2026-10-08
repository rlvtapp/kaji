# @relevate/kaji-plugin-csharp

Select Kaji's csharp SDK renderer in a JavaScript config. Install this package
with `@relevate/kaji`, then add `pluginCSharp()` to the config's
`plugins` array. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/kaji/sdk'
import { pluginCSharp } from '@relevate/kaji-plugin-csharp'

export default defineConfig({
  input: './openapi.yaml',
  output: { path: './generated' },
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginCSharp({ path: 'csharp' })],
})
```
