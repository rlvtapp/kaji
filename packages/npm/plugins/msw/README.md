# @relevate/poolster-plugin-msw

Select Poolster's compiled Rust msw plugin from a JavaScript config. Install
with `@relevate/poolster` and `@relevate/poolster-plugin-typescript`, then
list both factories in `plugins`. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/poolster'
import { pluginTypeScript } from '@relevate/poolster-plugin-typescript'
import { pluginMsw } from '@relevate/poolster-plugin-msw'

export default defineConfig({
  input: './openapi.yaml',
  output: './generated',
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginTypeScript(), pluginMsw({ target: 'typescript' })],
})
```

The `target` names the TypeScript SDK package path; use it when the package
path differs from the default or multiple TypeScript packages are selected.
