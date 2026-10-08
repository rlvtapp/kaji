# @relevate/poolster-plugin-ruby

Select Poolster's ruby SDK renderer in a JavaScript config. Install this package
with `@relevate/poolster`, then add `pluginRuby()` to the config's
`plugins` array. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/poolster'
import { pluginRuby } from '@relevate/poolster-plugin-ruby'

export default defineConfig({
  input: './openapi.yaml',
  output: { path: './generated' },
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginRuby({ path: 'ruby' })],
})
```
