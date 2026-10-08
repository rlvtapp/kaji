# Existing npm manifest

From the repository root, seed a user-owned manifest and generate:

```sh
mkdir -p examples/manifest-merging/generated/typescript
cp examples/manifest-merging/package.seed.json examples/manifest-merging/generated/typescript/package.json
cargo run -p poolster-cli -- generate --config examples/manifest-merging/poolster.json
```

Inspect the resulting package.json: the test script, private flag, repository,
pinned TypeScript version, and optional React Query peer survive. Generated
exports and files are appended. React Query is not duplicated as a runtime
dependency. Run generation again to confirm the manifest remains stable.

This example demonstrates manifest ownership, not production query-hook usage.
