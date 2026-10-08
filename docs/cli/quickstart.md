# Generate your first SDK from a local contract

Generate a TypeScript Fetch SDK from the sample Notes API. Then check, build and try it.

**You need:** Node/npm for `npx` and the TypeScript build. If you already have the native or pip CLI, use `poolster` instead.

Generation itself does not need the SDK’s dependencies installed.

## 1. Start with the two sample files

From the repository root:

```sh
cd examples/cli-basic
```

Or copy [openapi.yaml](../../examples/cli-basic/openapi.yaml) and [poolster.json](../../examples/cli-basic/poolster.json) into an empty directory.

The contract has `GET /notes/{noteId}` and a `Note` with required `id` and `body`. The recipe picks one Fetch SDK:

```json
{
  "openapi": { "input": "./openapi.yaml", "name": "Notes", "version": "1.0.0" },
  "output": { "path": "./generated" },
  "packages": [
    {
      "language": "typescript",
      "path": "typescript",
      "name": "@example/notes",
      "plugins": [{ "name": "sdk", "transport": "fetch", "client_name": "Notes" }]
    }
  ]
}
```

Input and output paths are resolved from the recipe location. Keep the contract and recipe outside `generated/`.

## 2. Generate and check the result

```sh
npx poolster generate --config poolster.json
npx poolster generate --config poolster.json --check
```

- **Generate:** writes `generated/typescript` and generation bookkeeping.
- **Check:** succeeds without writes when the destination matches.

Open the generated README for exports, build steps and runtime requirements.

To inspect proposed changes as JSON:

```sh
npx poolster generate --config poolster.json --check --format json
```

The report lists `added`, `modified` and `removed` paths. Drift makes the check fail.

This checks generation consistency. Native builds and API tests come next.

**Try a change:** rename `getNote` to `fetchNote` in the contract. Run the check to see the diff, then generate to apply it. Restore `getNote` for the example below.

## 3. Build the generated package

Once you want to use the SDK, install its declared dependencies and run its native build:

```sh
cd generated/typescript
npm install
npm run build
```

This is separate from Poolster generation. Dependency installation may need registry access. Your application's API call then uses the generated contract names:

```ts
import { Notes } from "@example/notes";

const api = new Notes({ baseUrl: "https://your-notes-api.example" });
const note = await api.notes.getNote({ path: { noteId: "note_123" } });
```

Install or link the built package in your application. Replace the example URL with your server.

Need a local API? Use [contract mocking](../mocking.md).

## 4. Keep author changes in the recipe

Keep custom source beside your recipe. [SDK customization](../sdk-customization.md) covers bundled middleware, added files and replacement patches.

Regeneration protects edited owned files and preserves unrelated or create-once files. Stale generated files are removed only when their fingerprints match.

[Resolve ownership conflicts →](../safe-regeneration.md) Writing is not an atomic directory swap; an I/O failure can interrupt it.

## 5. Prepare delivery when the package is ready

Commit your contract, recipe, custom source and chosen generated output/bookkeeping. Add build and behavior tests.

[Prepare SDK PRs](../sdk-automation.md) → [Release and publish](../sdk-publishing.md)

For your own local contract, `poolster init --input ./openapi.yaml --output ./generated` creates a starter recipe without overwriting an existing one. Continue with [configuration](config.md) for exact package settings, [multi-package recipes](recipes.md) for more outputs, or [commands](commands.md) for direct mode and filtering.
