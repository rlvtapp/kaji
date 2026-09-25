# SDK verification

Kaji protects generated SDKs at two levels.

## Output snapshots

`crates/kaji/tests/golden_output.rs` generates Rust, both TypeScript
transports, Go, Python, PHP, Java, .NET, Elixir, and the mock-server target
from one representative OpenAPI-derived API. The approved snapshot stores the
path, byte length, and deterministic content fingerprint for every emitted
file. A changed generator output fails CI until the fixture is consciously
reviewed and updated.

The snapshot is deliberately a file-level contract: a model, package manifest,
README, runtime helper, or mock fixture cannot change silently.

## Live SDK contract suite

`crates/kaji/tests/sdk_to_mock_contract.rs` materializes generated Go and
Python SDKs and calls a loopback contract mock. It verifies the real generated
clients make the OpenAPI-derived route, send configured bearer authentication,
and decode the returned model. The same generation pass also requires the
standalone `httpmock` fixture to be emitted.

This test is ignored in the ordinary local Rust suite because it opens a
loopback port. CI runs it explicitly:

```sh
cargo test -p kaji --test sdk_to_mock_contract -- --ignored
```

Adding another runtime to this suite means adding a small native test program
for its generated package. The source-output snapshot already covers every
maintained target; live execution is expanded only when its compiler/runtime
is available in CI without a network-dependent package installation.
