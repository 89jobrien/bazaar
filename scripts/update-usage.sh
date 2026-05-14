#!/usr/bin/env bash
# Update examples/ccusage.json with fresh ccusage data.
# Run before `bz` to get live token/cost stats on the profile page.
set -euo pipefail

OUT="$(dirname "$0")/../examples/ccusage.json"

echo "fetching ccusage --json..." >&2
ccusage --json 2>/dev/null \
  | sed 's/\x1b\[[0-9;]*m//g' \
  | jq '.' > "$OUT"

echo "wrote $OUT" >&2
