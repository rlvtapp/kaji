# Check your OpenAPI contract

[CLI workflow](README.md) · [Command reference](commands.md)

## Contract checks

```sh
kaji check openapi.yaml
kaji check openapi.yaml --format json
```

Kaji compiles the contract and checks the details that shape generated clients.
Source builds can use `--openapi-compiler <file>`.

| Finding | Why it matters |
| --- | --- |
| Missing or duplicate operation IDs | Methods need stable identities |
| IDs colliding after normalization | Native method names must stay unique |
| Missing 2xx responses | Success decoding needs a declared shape |
| Ambiguous path segments | Resource routing needs a clear layout |
| Invalid path parameters | Requests need complete path bindings |

Human output is the default. `--format json` (or `--json`) reports rule,
severity, request location, hint and a stable diagnostic fingerprint.

## Adopt rules gradually

Checks default to error severity and fail on errors.

```sh
# Keep a finding visible while you clean up the contract.
kaji check openapi.yaml --severity missing-operation-id=warning --fail-on error

# Record known findings, then fail on new ones.
kaji check openapi.yaml --write-baseline .kaji/check-baseline.json --fail-on none
kaji check openapi.yaml --baseline .kaji/check-baseline.json

# Suppress a rule for a temporary migration.
kaji check openapi.yaml --ignore missing-operation-id
```

| Option | Effect |
| --- | --- |
| `--severity RULE=warning` | Keeps a rule visible as advisory |
| `--fail-on warning` | Fails for warnings and errors |
| `--fail-on none` | Reports without failing for findings |
| `--ignore RULE` | Suppresses that entire rule |

## Review the baseline

Baselines match `code:METHOD:path`, so the same rule on a new endpoint still
appears. `--write-baseline` produces a compact, committable JSON file.

[Baseline schema →](../../schemas/v1/check-baseline.schema.json)
