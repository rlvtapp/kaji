# Postman collection and Terraform provider from one API

This example uses explicit Widgets CRUD bindings and exports a Postman collection
with a blank environment. It requires the new source-checkout plugins; published
Kaji 0.4.0 does not contain them.

Build the generator and embedded compiler from the repository root:

```sh
cargo build -p kaji-cli
(cd openapi && go build -o ../target/debug/kaji-openapi .)
target/debug/kaji generate --config examples/api-artifacts/kaji.json
```

Inspect:

- `generated/postman/widgets.postman_collection.json`: importable Collection 2.1.
- `generated/postman/widgets.postman_environment.json`: create-once credential template.
- `generated/terraform/.kaji/terraform-plan.json`: validated resource bindings and exclusions.
- `generated/terraform/internal/provider`: typed Framework provider/resources.

The Postman base URL is intentionally a non-live example domain. Populate a local
copy of the environment when importing it into Postman. Generate/check does not
send any requests or run collection scripts. Keep populated credentials out of Git.

Check the portable export using the editable checker and jsonschema 4.23.0:

```sh
python3 -m pip install 'jsonschema==4.23.0'
KAJI_COLLECTION=examples/api-artifacts/generated/postman/widgets.postman_collection.json \
KAJI_ENVIRONMENT=examples/api-artifacts/generated/postman/widgets.postman_environment.json \
python3 packages/postman-check/check.py
```

Build and test the provider:

```sh
(cd examples/api-artifacts/generated/terraform && go mod tidy && go test ./... && go build ./...)
target/debug/kaji generate --config examples/api-artifacts/kaji.json --check --format json
```

The generated Go manifest and checksum lock pin the Framework dependency graph;
ordinary dependency setup must leave them unchanged. Read the generated provider README before configuring
Terraform. A populated provider talks to your API only when you deliberately run
Terraform against it.

For CI, generate from the committed recipe first, then copy the editable
`packages/postman-check` action sources into `.github/actions/postman-check` and
`packages/sdk-check/{action.yml,check.mjs}` into `.github/actions/kaji-check`:

```yaml
- uses: ./.github/actions/postman-check
  with:
    collection: examples/api-artifacts/generated/postman/widgets.postman_collection.json
    environment: examples/api-artifacts/generated/postman/widgets.postman_environment.json
- uses: ./.github/actions/kaji-check
  with:
    path: examples/api-artifacts/generated/terraform
    language: terraform
```

See the [Postman guide](../../docs/postman.md) and
[typed Terraform guide](../../docs/terraform-provider.md) for supported behavior,
plugin customization and follow-up scope. These outputs share API metadata, not
Terraform lifecycle assumptions.
