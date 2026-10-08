# @relevate/poolster-plugin-elixir

Select Poolster's elixir SDK renderer in a JavaScript config. Install this package
with `@relevate/poolster`, then add `pluginElixir()` to the config's
`plugins` array. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/poolster'
import { pluginElixir } from '@relevate/poolster-plugin-elixir'

export default defineConfig({
  input: './openapi.yaml',
  output: { path: './generated' },
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginElixir({ path: 'elixir' })],
})
```
