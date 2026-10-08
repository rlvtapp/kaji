# Microsoft Graph: Poolster demo

This is a real config-first Poolster project using the public Microsoft Graph v1.0
OpenAPI document directly by URL. It generates two independently configured,
namespaced SDKs:

- `generated/typescript`: Fetch-based TypeScript client, exported as
  `@poolster/microsoft-graph` with `new MicrosoftGraph(...)`.
- `generated/go`: standard-library Go client package named `graph`, split into
  bounded model, operation, and service files for the large contract.

From this directory, run:

```sh
npx poolster generate --config poolster.json
```

The first run downloads the roughly 42 MiB public contract. Poolster stores the
download only in its temporary compiler workspace; the generated SDKs are the
only durable output in `generated/`. That directory is intentionally ignored by
Git because the current Graph document produces tens of thousands of files.

After generation, start with:

- `generated/typescript/README.md` and `generated/typescript/client.ts`
- `generated/go/README.md` and `generated/go/client.go`

The source is public and changes over time, so exact file counts and operation
names can change. No Microsoft account, credentials, or live API call is needed
to generate either package.

For a reproducible full verification that also compiles the Go SDK, run:

```sh
bash ../../scripts/test-large-graph.sh
```

See the [large-spec guide](../../docs/large-specs.md) for the performance and
validation details, and the [`poolster.json` reference](../../docs/config-file.md)
for remote URL, headers, and authentication settings.
