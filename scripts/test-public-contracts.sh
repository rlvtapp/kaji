#!/usr/bin/env bash
# Compile checksum-pinned public contracts; never call the production APIs.
set -euo pipefail
project_root="$(cd "$(dirname "$0")/.." && pwd)"
mode="${1:-generate}"
contract_root="${KAJI_PUBLIC_CONTRACT_ROOT:?Set KAJI_PUBLIC_CONTRACT_ROOT to a disposable output directory}"
manifest="${KAJI_PUBLIC_CONTRACT_MANIFEST:-$project_root/scripts/fixtures/public-contracts.json}"
if [[ "$mode" == generate ]]; then
  mkdir -p "$contract_root/specs"
  python3 "$project_root/scripts/public-contracts.py" fetch "$manifest" "$contract_root/specs" "${KAJI_PUBLIC_SPEC_DIR:-}"
  if [[ -z "${KAJI_BINARY:-}" ]]; then
    cargo build --manifest-path "$project_root/Cargo.toml" --locked -p kaji-cli
  fi
  kaji_binary="${KAJI_BINARY:-$project_root/target/debug/kaji}"
  listing="$(python3 "$project_root/scripts/public-contracts.py" list "$manifest" "$contract_root/specs")"
  while IFS=$'\t' read -r contract source_path; do
    "$kaji_binary" generate "$source_path" --output "$contract_root/$contract" \
      --language "${KAJI_PUBLIC_LANGUAGES:-rust,typescript,go,python,php,java,csharp,elixir,ruby,swift}" \
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
          if [[ -n "${KAJI_TSC_JS:-}" ]]; then node "$KAJI_TSC_JS" -p tsconfig.json
          else npm install --ignore-scripts && npm run build; fi ;;
        go) go test ./... ;;
        python) "${KAJI_TEST_PYTHON:-python3}" -m compileall -q src && PYTHONPATH=src "${KAJI_TEST_PYTHON:-python3}" -c 'import public_contract_sdk' ;;
        ruby)
          while IFS= read -r -d '' source; do ruby -c "$source"; done < <(find lib -name '*.rb' -print0)
          ruby -Ilib -e 'require "public_contract_sdk"' ;;
        swift) swift build --disable-sandbox ;;
        php) find src -name '*.php' -print0 | xargs -0 -n1 php -l ;;
        java) mvn --batch-mode --no-transfer-progress -q -DskipTests compile ;;
        csharp) dotnet build --configuration Release ;;
        elixir) mix deps.get && mix compile --warnings-as-errors ;;
        *) printf 'Unsupported language: %s\n' "$language" >&2; exit 1 ;;
      esac
    )
  done <<< "$listing"
else
  printf 'Usage: %s generate | check LANGUAGE\n' "$0" >&2
  exit 1
fi
