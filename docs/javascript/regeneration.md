# Preview and regenerate

← [JavaScript](README.md)

Load the same config for each run.

```js
import { generate, loadConfig } from '@relevate/poolster';
const config = await loadConfig('./poolster.config.mjs');

const preview = await generate(config, { write: false });
console.log(preview.changes); // added, modified, removed

await generate(config); // write the owned output
```

Poolster tracks file ownership. It preserves unrelated files and refuses to
overwrite locally edited generated files. A plugin can emit a create-once file
with `preserveExisting: true` for code the user will maintain.

## Use the CLI in CI

Install the separate command package if needed:

```sh
npm install -D poolster
npx poolster generate --config poolster.config.mjs --check
```

`--check` reports drift without writing. `--dry-run` previews changes.

**Next:** [File ownership](../internals/files.md) · [CI reference](../reference/automation/ci-integration.md)
