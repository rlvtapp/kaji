#!/usr/bin/env bash
# Opt-in integration check; downloads ~42 MiB and retains outputs for inspection.
set -euo pipefail
project_root="$(cd "$(dirname "$0")/.." && pwd)"
graph_work="$(mktemp -d "${TMPDIR:-/tmp}/kaji-graph.XXXXXX")"
export GOCACHE="${GOCACHE:-$graph_work/go-cache}"
printf 'Graph test workspace: %s\n' "$graph_work"
curl -fL --retry 2 --max-time 180 https://aka.ms/graph/v1.0/openapi.yaml -o "$graph_work/openapi.yaml"
(cd "$project_root/openapi" && go build -o "$graph_work/kaji-openapi" .)
cargo build --manifest-path "$project_root/Cargo.toml" --target-dir "$project_root/target" --locked --release -p kaji-cli
"$graph_work/kaji-openapi" --out "$graph_work/artifacts" "$graph_work/openapi.yaml"
for jobs in 1 4; do
  "$project_root/target/release/kaji" generate --artifacts "$graph_work/artifacts" \
    --output "$graph_work/sdk-$jobs" --language go --name 'Microsoft Graph' --jobs "$jobs"
done
diff -qr "$graph_work/sdk-1" "$graph_work/sdk-4"
cp "$project_root/crates/plugins/go/tests/fixtures/graph_smoke_test.go" "$graph_work/sdk-4/go/graph_smoke_test.go"
(cd "$graph_work/sdk-4/go" && go test ./...)
cp "$project_root/scripts/fixtures/graph.kaji.json" "$graph_work/kaji.json"
(cd "$graph_work" && "$project_root/target/release/kaji" generate --color never)
test -f "$graph_work/sdk-config/go/go.mod"
test -f "$graph_work/sdk-config/typescript/package.json"
test -f "$graph_work/sdk-config/typescript/client.ts"
printf 'Graph compilation, SDK smoke test, deterministic parallel output and config-first multi-language generation passed. Outputs: %s\n' "$graph_work"
