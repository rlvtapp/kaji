# @relevate/poolster-plugin-java

Select Poolster's java SDK renderer in a JavaScript config. Install this package
with `@relevate/poolster`, then add `pluginJava()` to the config's
`plugins` array. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/poolster'
import { pluginJava } from '@relevate/poolster-plugin-java'

export default defineConfig({
  input: './openapi.yaml',
  output: { path: './generated' },
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginJava({ path: 'java' })],
})
```
