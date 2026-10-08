# @relevate/poolster-plugin-php

Select Poolster's php SDK renderer in a JavaScript config. Install this package
with `@relevate/poolster`, then add `pluginPhp()` to the config's
`plugins` array. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/poolster'
import { pluginPhp } from '@relevate/poolster-plugin-php'

export default defineConfig({
  input: './openapi.yaml',
  output: { path: './generated' },
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginPhp({ path: 'php' })],
})
```
