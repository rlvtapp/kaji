# @relevate/kaji-plugin-java

Select Kaji's java SDK renderer in a JavaScript config. Install this package
with `@relevate/kaji`, then add `pluginJava()` to the config's
`plugins` array. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/kaji/sdk'
import { pluginJava } from '@relevate/kaji-plugin-java'

export default defineConfig({
  input: './openapi.yaml',
  output: { path: './generated' },
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginJava({ path: 'java' })],
})
```
