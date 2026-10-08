# @relevate/poolster-plugin-faker

Select Poolster's compiled Rust faker plugin from a JavaScript config. Install
with `@relevate/poolster` and `@relevate/poolster-plugin-typescript`, then
list both factories in `plugins`. Nothing is registered automatically.

```js
import { defineConfig } from '@relevate/poolster'
import { pluginTypeScript } from '@relevate/poolster-plugin-typescript'
import { pluginFaker } from '@relevate/poolster-plugin-faker'

export default defineConfig({
  input: './openapi.yaml',
  output: './generated',
  name: 'Example',
  version: '1.0.0',
  plugins: [pluginTypeScript(), pluginFaker({ target: 'typescript' })],
})
```

The `target` names the TypeScript SDK package path; use it when the package
path differs from the default or multiple TypeScript packages are selected.
