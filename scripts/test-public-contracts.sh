#!/usr/bin/env bash
# Compile checksum-pinned public contracts; never call the production APIs.
set -euo pipefail
project_root="$(cd "$(dirname "$0")/.." && pwd)"
mode="${1:-generate}"
contract_root="${POOLSTER_PUBLIC_CONTRACT_ROOT:?Set POOLSTER_PUBLIC_CONTRACT_ROOT to a disposable output directory}"
manifest="${POOLSTER_PUBLIC_CONTRACT_MANIFEST:-$project_root/scripts/fixtures/public-contracts.json}"
if [[ "$mode" == generate ]]; then
  mkdir -p "$contract_root/specs"
  python3 "$project_root/scripts/public-contracts.py" fetch "$manifest" "$contract_root/specs" "${POOLSTER_PUBLIC_SPEC_DIR:-}"
  if [[ -z "${POOLSTER_BINARY:-}" ]]; then
    cargo build --manifest-path "$project_root/Cargo.toml" --locked -p poolster-cli
  fi
  poolster_binary="${POOLSTER_BINARY:-$project_root/target/debug/poolster}"
  listing="$(python3 "$project_root/scripts/public-contracts.py" list "$manifest" "$contract_root/specs")"
  while IFS=$'\t' read -r contract source_path; do
    "$poolster_binary" generate "$source_path" --output "$contract_root/$contract" \
      --language "${POOLSTER_PUBLIC_LANGUAGES:-rust,typescript,go,python,php,java,csharp,elixir,ruby,swift}" \
      --name 'Public Contract' --color never
  done <<< "$listing"
elif [[ "$mode" == check ]]; then
  language="${2:?Supply a native language directory name}"
  case "$language" in
    rust|typescript|go|python|ruby|swift|php|java|csharp|elixir) ;;
    *) printf 'Unsupported language: %s\n' "$language" >&2; exit 1 ;;
  esac
  listing="$(python3 "$project_root/scripts/public-contracts.py" list "$manifest" "$contract_root/specs")"
  while IFS=$'\t' read -r contract source_path; do
    printf 'Compile %s / %s\n' "$contract" "$language"
    (
      cd "$contract_root/$contract/$language"
      case "$language" in
        rust) cargo check --quiet ;;
        typescript)
          if [[ -n "${POOLSTER_TSC_JS:-}" ]]; then node "$POOLSTER_TSC_JS" -p tsconfig.json
          else npm install --ignore-scripts && npm run build; fi ;;
        go) go test ./... ;;
        python) "${POOLSTER_TEST_PYTHON:-python3}" -m compileall -q src && PYTHONPATH=src "${POOLSTER_TEST_PYTHON:-python3}" -c 'import public_contract_sdk' ;;
        ruby)
          while IFS= read -r -d '' source; do ruby -c "$source"; done < <(find lib -name '*.rb' -print0)
          ruby -Ilib -e 'require "public_contract_sdk"' ;;
        swift) swift build --disable-sandbox ;;
        php) find src -name '*.php' -print0 | xargs -0 -n1 php -l ;;
        java) mvn --batch-mode --no-transfer-progress -q -DskipTests compile ;;
        csharp) dotnet build --configuration Release --disable-build-servers -p:UseSharedCompilation=false ;;
        elixir) mix deps.get && mix compile --warnings-as-errors ;;
        *) printf 'Unsupported language: %s\n' "$language" >&2; exit 1 ;;
      esac
    )
  done <<< "$listing"
else
  printf 'Usage: %s generate | check LANGUAGE\n' "$0" >&2
  exit 1
fi
