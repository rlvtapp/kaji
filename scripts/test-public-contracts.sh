#!/usr/bin/env bash
# Compile checksum-pinned public contracts; never call the production APIs.
set -euo pipefail
project_root="$(cd "$(dirname "$0")/.." && pwd)"
mode="${1:-generate}"
contract_root="${KAJI_PUBLIC_CONTRACT_ROOT:?Set KAJI_PUBLIC_CONTRACT_ROOT to a disposable output directory}"
manifest="$project_root/scripts/fixtures/public-contracts.json"
if [[ "$mode" == generate ]]; then
  mkdir -p "$contract_root/specs"
  python3 - "$manifest" "$contract_root/specs" "${KAJI_PUBLIC_SPEC_DIR:-}" <<'PY'
import hashlib,json,pathlib,shutil,sys,urllib.request
manifest=json.load(open(sys.argv[1]));root=pathlib.Path(sys.argv[2]);cache=sys.argv[3]
for contract in manifest['contracts']:
    dest=root/(contract['name']+'.yml')
    if cache: shutil.copyfile(pathlib.Path(cache)/dest.name,dest)
    else:
        with urllib.request.urlopen(contract['url'],timeout=60) as response: data=response.read(2*1024*1024+1)
        if len(data)>2*1024*1024: raise SystemExit('Contract exceeds 2 MiB')
        dest.write_bytes(data)
    if hashlib.sha256(dest.read_bytes()).hexdigest()!=contract['sha256']: raise SystemExit('Contract checksum mismatch: '+contract['name'])
    print('Verified pinned contract:',contract['name'])
PY
  if [[ -z "${KAJI_BINARY:-}" ]]; then
    cargo build --manifest-path "$project_root/Cargo.toml" --locked -p kaji-cli
  fi
  kaji_binary="${KAJI_BINARY:-$project_root/target/debug/kaji}"
  for contract in forecast air-quality; do
    "$kaji_binary" generate "$contract_root/specs/$contract.yml" --output "$contract_root/$contract" \
      --language rust,typescript,go,python,php,java,csharp,elixir,ruby,swift \
      --name 'Public Contract' --color never
  done
elif [[ "$mode" == check ]]; then
  language="${2:?Supply a native language directory name}"
  case "$language" in
    rust|typescript|go|python|ruby|swift|php|java|csharp|elixir) ;;
    *) printf 'Unsupported language: %s\n' "$language" >&2; exit 1 ;;
  esac
  for contract in forecast air-quality; do
    printf 'Compile %s / %s\n' "$contract" "$language"
    (
      cd "$contract_root/$contract/$language"
      case "$language" in
        rust) cargo check --quiet ;;
        typescript)
          if [[ -n "${KAJI_TSC_JS:-}" ]]; then node "$KAJI_TSC_JS" -p tsconfig.json
          else npm install --ignore-scripts && npm run build; fi ;;
        go) go test ./... ;;
        python) python3 -m compileall -q src && PYTHONPATH=src python3 -c 'import public_contract_sdk' ;;
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
  done
else
  printf 'Usage: %s generate | check LANGUAGE\n' "$0" >&2
  exit 1
fi
