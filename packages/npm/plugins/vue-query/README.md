# @relevate/poolster-plugin-vue-query

Select Poolster's compiled Rust vue-query plugin from a JavaScript config. Install
with `@relevate/poolster` and `@relevate/poolster-plugin-typescript`, then
list both factories in `plugins`. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/poolster'
import { pluginTypeScript } from '@relevate/poolster-plugin-typescript'
import { pluginVueQuery } from '@relevate/poolster-plugin-vue-query'

export default defineConfig({
  input: './openapi.yaml',
  output: './generated',
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginTypeScript(), pluginVueQuery({ target: 'typescript' })],
})
```

The `target` names the TypeScript SDK package path; use it when the package
path differs from the default or multiple TypeScript packages are selected.
