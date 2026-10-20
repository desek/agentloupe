#!/usr/bin/env bash
# @agents-index Vendors the LiteLLM price file and its JSON schema from one pinned commit into vendor/litellm/, validated before write.
#
# Usage: prices.vendor.sh [--ref COMMIT_SHA] [--dry-run]
#   --ref      Commit of BerriAI/litellm to vendor from. Default: the newest commit that changed the price file or the schema.
#   --dry-run  Download and validate, but write nothing.
#
# Environment: PRICES_VENDOR_SOURCE_DIR, if set, reads the three files from that local directory
# instead of GitHub (ref "local"); the tests use it with a fixture, so they need no network.
#
# Output (stdout, key: value): ref, committed, sha256 and bytes per file, entries, validation, action.
# Exit codes: 0 success, 2 bad usage, 3 download failed, 4 validation failed.
set -euo pipefail

REPO="BerriAI/litellm"
DATA="model_prices_and_context_window.json"
SCHEMA="model_prices_and_context_window.schema.json"
# Pinned validator. The schema uses Python regex syntax such as (?i), so validate with the python regex variant.
VALIDATOR="check-jsonschema==0.33.0"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DEST="$ROOT/vendor/litellm"
ref=""; dry_run=0

usage() { sed -n '4,13p' "$0" | sed 's/^# \{0,1\}//'; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --ref) ref="${2:?--ref needs a commit SHA}"; shift 2 ;;
    --dry-run) dry_run=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags." >&2; exit 2 ;;
  esac
done

fail() {
  echo "error: $1. Fix: check the network and GitHub rate limits, or pass --ref with a known commit. Verify: re-run and check that 'action: written' or 'action: unchanged' prints." >&2
  exit 3
}

# Prints "<sha> <date>" of the newest commit that changed a path, at or before the ref.
last_change() {
  curl -sf "https://api.github.com/repos/$REPO/commits?path=$1&per_page=1${ref:+&sha=$ref}" \
    | python3 -I -c 'import json,sys; c=json.load(sys.stdin)[0]; print(c["sha"], c["commit"]["committer"]["date"])'
}
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
if [[ -n "${PRICES_VENDOR_SOURCE_DIR:-}" ]]; then
  pin="local"; committed="local"
  for f in "$DATA" "$SCHEMA" LICENSE; do
    cp "$PRICES_VENDOR_SOURCE_DIR/$f" "$tmp/$f" || fail "could not read $f from $PRICES_VENDOR_SOURCE_DIR"
  done
else
  read -r data_sha data_date < <(last_change "$DATA") || fail "could not find a commit for $DATA"
  read -r schema_sha schema_date < <(last_change "$SCHEMA") || fail "could not find a commit for $SCHEMA"
  # Both files come from one commit, so the data and the schema always match. ISO dates compare as strings.
  if [[ "$data_date" > "$schema_date" ]]; then pin="$data_sha"; committed="$data_date"; else pin="$schema_sha"; committed="$schema_date"; fi
  for f in "$DATA" "$SCHEMA" LICENSE; do
    curl -sf -o "$tmp/$f" "https://raw.githubusercontent.com/$REPO/$pin/$f" || fail "could not download $f at $pin"
  done
fi
entries="$(python3 -I -c 'import json,sys; print(len(json.load(open(sys.argv[1]))))' "$tmp/$DATA")" \
  || fail "the downloaded $DATA is not valid JSON"

if ! command -v uvx >/dev/null; then
  echo "error: uvx is not installed, so the price file cannot be validated. Fix: install uv (https://docs.astral.sh/uv/). Verify: 'validation: passed' prints." >&2
  exit 4
fi
if ! uvx -q --from "$VALIDATOR" check-jsonschema --regex-variant python --schemafile "$tmp/$SCHEMA" "$tmp/$DATA" >"$tmp/validation.log" 2>&1; then
  cat "$tmp/validation.log" >&2
  echo "error: $DATA at $pin does not match $SCHEMA. Fix: pass --ref with an older commit, or report the mismatch upstream. Verify: 'validation: passed' prints." >&2
  exit 4
fi

# The required models, as model:provider, and the price fields each entry must hold. The test
# tests/price.required.rs holds the same list. A tier field is required only if the entry has it.
REQUIRED_MODELS="claude-haiku-5-5:anthropic"
REQUIRED_FIELDS="input_cost_per_token output_cost_per_token cache_read_input_token_cost cache_creation_input_token_cost cache_creation_input_token_cost_above_1hr"
if ! python3 -I -c '
import json, sys
data = json.load(open(sys.argv[1]))
bad = 0
for pair in sys.argv[2].split():
    model, provider = pair.split(":")
    e = data.get(model)
    if not isinstance(e, dict) or e.get("litellm_provider") != provider:
        print(f"missing: model {model} with litellm_provider {provider}", file=sys.stderr); bad = 1; continue
    for f in sys.argv[3].split():
        if not isinstance(e.get(f), (int, float)):
            print(f"missing: model {model} field {f}", file=sys.stderr); bad = 1
sys.exit(bad)
' "$tmp/$DATA" "$REQUIRED_MODELS" "$REQUIRED_FIELDS"; then
  echo "error: $DATA at $pin does not hold a complete entry for each required model ($REQUIRED_MODELS); nothing was written. Fix: pass --ref with a LiteLLM commit that holds the entry and every field of $REQUIRED_FIELDS. Reference: docs/specification/cost.md, section Required model. Verify: re-run and see 'validation: passed'." >&2
  exit 4
fi

sum() { shasum -a 256 "$1" | cut -d' ' -f1; }
size() { wc -c <"$1" | tr -d ' '; }

cat >"$tmp/SOURCE.md" <<EOF
<!-- @agents-index Records where the vendored LiteLLM price file and schema come from, so the pin can be checked and refreshed. -->
# Vendored: LiteLLM model prices

| Field | Value |
|---|---|
| Upstream | https://github.com/$REPO/tree/$pin |
| Commit | \`$pin\` |
| Committed | $committed |
| \`$DATA\` SHA-256 | \`$(sum "$tmp/$DATA")\` |
| \`$SCHEMA\` SHA-256 | \`$(sum "$tmp/$SCHEMA")\` |
| Entries | $entries |
| Validation | Passed with \`$VALIDATOR --regex-variant python\` |
| License | MIT (see \`LICENSE\`; the files are outside the upstream \`enterprise/\` folder) |

Do not edit these files by hand. Refresh them with \`.agents/scripts/prices.vendor.sh\`.
EOF

action="written"
if [[ -f "$DEST/SOURCE.md" ]] && cmp -s "$tmp/SOURCE.md" "$DEST/SOURCE.md"; then action="unchanged"; fi
if [[ $dry_run -eq 1 ]]; then action="dry-run"; else
  mkdir -p "$DEST"
  cp "$tmp/$DATA" "$tmp/$SCHEMA" "$tmp/LICENSE" "$tmp/SOURCE.md" "$DEST/"
fi

printf 'ref: %s\ncommitted: %s\ndata_sha256: %s\ndata_bytes: %s\nschema_sha256: %s\nschema_bytes: %s\nentries: %s\nvalidation: passed\naction: %s\n' \
  "$pin" "$committed" "$(sum "$tmp/$DATA")" "$(size "$tmp/$DATA")" "$(sum "$tmp/$SCHEMA")" "$(size "$tmp/$SCHEMA")" "$entries" "$action"
