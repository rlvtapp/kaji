# CLI quickstart

This guide generates a TypeScript Fetch SDK from a local OpenAPI document. The
same recipe can later emit other languages and helper artifacts.

## 1. Create a recipe

```sh
npx @relevate/kaji init \
  --input ./openapi.yaml \
  --output ./generated \
  --name "Pet Store" \
  --sdk-version 1.0.0
```

The command never overwrites an existing recipe. For a ready-to-run contract,
copy [examples/cli-basic](../../examples/cli-basic/README.md).

## 2. Generate

```sh
npx @relevate/kaji generate
```

Generated files are overwritten on future runs; unrelated files remain. Use a
fresh output directory after removing operations or changing generated paths,
because stale generated files are intentionally not pruned.

## 3. Build and use the package

```sh
cd generated/typescript
npm install
npm run build
```

```ts
import { PetStore } from "@acme/pet-store";

const api = new PetStore({ baseUrl: "https://api.example.com" });
const pet = await api.pets.getPet({ path: { petId: "pet_123" } }).unwrap();
```

Exact exports come from the contract. Each generated package has a local README
with its build and runtime requirements.

Next: [recipes](recipes.md), [`kaji.json`](config.md), or
[direct-mode commands](commands.md).
