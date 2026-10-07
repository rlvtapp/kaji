# Generate your first SDK from a local contract

This walkthrough uses the small Notes API already in the repository. It produces a TypeScript Fetch package, verifies repeatable generation, and gives you a starting point for customization and releases. It does not call a live API or publish anything.

You need a supported Kaji installation. The `npx @relevate/kaji` examples use Node/npm; replace that prefix with `kaji` if you installed the Python launcher or already have the executable. Building TypeScript later needs Node/npm, but the first generation/check requires no target dependency installation.

## 1. Start with the two sample files

From the repository root:

```sh
cd examples/cli-basic
```

Alternatively, copy [openapi.yaml](../../examples/cli-basic/openapi.yaml) and [kaji.json](../../examples/cli-basic/kaji.json) into an empty directory, then run the following commands there. The contract describes `GET /notes/{noteId}` and a `Note` with required `id` and `body` fields. The recipe selects one TypeScript Fetch SDK:

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
npx @relevate/kaji generate --config kaji.json
npx @relevate/kaji generate --config kaji.json --check
```

The first command writes `generated/typescript` and generation bookkeeping. Open that package's README to see its actual exports, build steps, and runtime requirements. The second command succeeds without changing destination files when generation matches the output.

To inspect proposed changes as JSON:

```sh
npx @relevate/kaji generate --config kaji.json --check --format json
```

The report contains `added`, `modified`, and `removed` paths. A nonempty report exits unsuccessfully so CI can detect drift. A clean report verifies generation consistency, not native compilation or API behavior.

Try changing `operationId: getNote` to `operationId: fetchNote` in the sample contract. Run the check to inspect the changed operation output, then run generation to apply it. Restore the name if you want the client example below to remain unchanged.

## 3. Build the generated package

Once you want to use the SDK, install its declared dependencies and run its native build:

```sh
cd generated/typescript
npm install
npm run build
```

This is separate from Kaji generation. Dependency installation may need registry access. Your application's API call then uses the generated contract names:

```ts
import { Notes } from "@example/notes";

const api = new Notes({ baseUrl: "https://your-notes-api.example" });
const note = await api.notes.getNote({ path: { noteId: "note_123" } });
```

Install/link the built package into your application before using this import. The URL is a placeholder for a server implementing the Notes contract; generating the SDK does not start a server. Use [contract mocking](../mocking.md) when you need a local test API.

## 4. Keep author changes in the recipe

Avoid editing materialized generated operations directly. [SDK customization](../sdk-customization.md) shows how to bundle middleware that runs automatically for your customers, add source, or replace/patch a specific generated file. Keep those source files beside the recipe and test their runtime behavior.

Generation refuses to overwrite locally edited owned files. It removes obsolete generated files only when their recorded fingerprints still match, and preserves unrelated/create-once files. Read [safe regeneration](../safe-regeneration.md) before resolving an ownership conflict; an I/O failure can still interrupt materialization, so this is not an atomic directory swap.

## 5. Prepare delivery when the package is ready

Commit the source contract, recipe, customization sources, and your chosen generated output/bookkeeping. Add meaningful build/tests before automation. [SDK repository automation](../sdk-automation.md) walks through package release metadata, local workflow setup, generated SDK PRs, and Release Please's separate version/changelog PR. [Publishing](../sdk-publishing.md) covers registry identity, tagged artifacts, credentials/trust, and protected publication jobs.

For your own local contract, `kaji init --input ./openapi.yaml --output ./generated` creates a starter recipe without overwriting an existing one. Continue with [configuration](config.md) for exact package settings, [multi-package recipes](recipes.md) for more outputs, or [commands](commands.md) for direct mode and filtering.
