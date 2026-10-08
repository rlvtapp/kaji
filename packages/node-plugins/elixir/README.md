# @relevate/kaji-plugin-elixir

Select Kaji's elixir SDK renderer in a JavaScript config. Install this package
with `@relevate/kaji`, then add `pluginElixir()` to the config's
`plugins` array. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/kaji/sdk'
import { pluginElixir } from '@relevate/kaji-plugin-elixir'

export default defineConfig({
  input: './openapi.yaml',
  output: { path: './generated' },
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginElixir({ path: 'elixir' })],
})
```
