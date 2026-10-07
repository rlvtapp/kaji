# Own the generation and delivery sources

For package-scoped source overrides and customer runtime middleware, see
[SDK customization](sdk-customization.md). These extensions do not require a fork.

Kaji's Rust generator/plugin workspace, Go OpenAPI compiler, GitHub App broker,
GitHub Actions and publishing helpers are source projects in this repository.
They can be forked, modified and run independently. Review the repository's
LICENSE and individual dependency licenses when redistributing your fork.

```sh
cargo build -p kaji-cli
(cd openapi && go build -o ../target/debug/kaji-openapi .)
./target/debug/kaji generate --config kaji.json
./target/debug/kaji sdk init --root generated --actions local --dry-run
```

Default `--actions local` exports readable action YAML and Node helpers into the
SDK repository. `--actions remote --action-ref YOUR_OWNER/kaji@YOUR_REVISION`
instead references actions in a fork. Pin `--kaji-version` to a published launcher
containing these commands. For unpublished/forked source builds, replace the
workflow's launcher command with your own built/distributed binary; local builds
do not automatically publish a launcher.

Native custom plugins implement `Language`, `Plugin`, and typed contracts.
Consumers bind provider handles, and package finalization supplies manifests.
Optional `release::metadata::<YourLanguage>(PackageMetadata)` connects a community
plugin to checks/releases without adding a core registry or language enum. A
custom `publisher.registry` uses explicit argument-vector commands. Standard
`npm`, `pypi`, `crates.io`, and `go` publishing is opt-in by omitting commands;
nonempty commands always select the plugin's custom publication path.

The CLI recipe registry currently exposes bundled plugins; installing an arbitrary
Rust plugin does not register it in JSON automatically. Embed your plugin with
Kaji's library API or extend the CLI registry in a fork. See
[plugin authoring](typed-plugins.md) and [native provider contracts](native-sdk-providers.md).

Useful source entry points:

- `crates/kaji-core/src/engine.rs`: capability graph and plugin execution.
- `crates/kaji-core/src/release.rs`: optional package delivery contract.
- `crates/kaji-cli/src/sdk_automation.rs`: editable workflow and sync generation.
- `packages/sdk-check`: per-language setup/check action.
- `packages/sdk-publish`: standard registry action and retry verification.
- `packages/github-app-broker`: policy-enforced OIDC broker and token client.

A reproducible fork should run Rust workspace tests/Clippy, the Go compiler suite,
and each action/broker's Node tests, then exercise a generated SDK against its
native toolchain. Ordinary CI checks have read-only repository permission; release
checks precede publishing and each registry still needs its trusted-publisher
registration. Register/host a GitHub App and broker explicitly to enable them.

TypeScript generated and bundled modules receive ESM-compatible relative import
extensions during package finalization. Explicit source overlays retain your exact
bytes; use `.js` relative specifiers in authored ESM overlays so the installed
package resolves them in Node.
