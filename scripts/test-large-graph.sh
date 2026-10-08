#!/usr/bin/env bash
# Opt-in pinned integration check; downloads ~42 MiB and retains outputs.
set -euo pipefail
project_root="$(cd "$(dirname "$0")/.." && pwd)"
graph_work="$(mktemp -d "${TMPDIR:-/tmp}/poolster-graph.XXXXXX")"
export GOCACHE="${GOCACHE:-$graph_work/go-cache}"
printf 'Graph test workspace: %s\n' "$graph_work"
graph_url=https://raw.githubusercontent.com/microsoftgraph/msgraph-metadata/fd42f0e5bcd96b0c5edd5e62e2956a1dc1c5d17a/openapi/v1.0/openapi.yaml
graph_sha=533f6d86985584327109ba2c52c0e51454ce1ba2153c11f4d841e898b88c5b1b
if [[ -n "${POOLSTER_GRAPH_SPEC:-}" ]]; then
  cp "$POOLSTER_GRAPH_SPEC" "$graph_work/openapi.yaml"
else
  curl -fL --retry 2 --max-time 180 "$graph_url" -o "$graph_work/openapi.yaml"
fi
printf '%s  %s\n' "$graph_sha" "$graph_work/openapi.yaml" | shasum -a 256 -c -
(cd "$project_root/openapi" && go build -o "$graph_work/poolster-openapi" .)
if [[ -z "${POOLSTER_BINARY:-}" ]]; then
  cargo build --manifest-path "$project_root/Cargo.toml" --target-dir "$project_root/target" --locked --release -p poolster-cli
fi
graph_poolster="${POOLSTER_BINARY:-$project_root/target/release/poolster}"
"$graph_work/poolster-openapi" --out "$graph_work/artifacts" "$graph_work/openapi.yaml"
for jobs in 1 4; do
  "$graph_poolster" generate --artifacts "$graph_work/artifacts" \
    --output "$graph_work/sdk-$jobs" --language go --name 'Microsoft Graph' --jobs "$jobs"
done
# Run metadata records different job settings/output roots; compare complete SDK packages.
diff -qr "$graph_work/sdk-1/go" "$graph_work/sdk-4/go"
cp "$project_root/crates/plugins/go/tests/fixtures/graph_smoke_test.go" "$graph_work/sdk-4/go/graph_smoke_test.go"
(cd "$graph_work/sdk-4/go" && go test ./...)
cp "$project_root/scripts/fixtures/graph.poolster.json" "$graph_work/poolster.json"
(cd "$graph_work" && "$graph_poolster" generate --color never)
test -f "$graph_work/sdk-config/go/go.mod"
test -f "$graph_work/sdk-config/typescript/package.json"
test -f "$graph_work/sdk-config/typescript/client.ts"
printf 'Graph compilation, SDK smoke test, deterministic parallel output and config-first multi-language generation passed. Outputs: %s\n' "$graph_work"
