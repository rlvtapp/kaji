# @relevate/kaji-plugin-php

Select Kaji's php SDK renderer in a JavaScript config. Install this package
with `@relevate/kaji`, then add `pluginPhp()` to the config's
`plugins` array. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/kaji/sdk'
import { pluginPhp } from '@relevate/kaji-plugin-php'

export default defineConfig({
  input: './openapi.yaml',
  output: { path: './generated' },
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginPhp({ path: 'php' })],
})
```
