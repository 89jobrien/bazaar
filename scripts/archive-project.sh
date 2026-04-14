#!/usr/bin/env bash
# Archive a local ~/dev project to the Extreme SSD, index it in Obsidian, then remove it.
#
# Usage: archive-project.sh <project-name>
# Example: archive-project.sh plugit

set -euo pipefail

PROJECT_NAME="${1:?Usage: archive-project.sh <project-name>}"
SRC="$HOME/dev/$PROJECT_NAME"
SSD_DEST="/Volumes/Extreme SSD/vault/$PROJECT_NAME"
OBSIDIAN_INDEX="$HOME/Documents/Obsidian Vault/archived-projects.md"
ARCHIVE_DATE=$(date +%Y-%m-%d)

# ── Pre-flight ────────────────────────────────────────────────────────────────

if [[ ! -d "$SRC" ]]; then
  echo "error: $SRC does not exist" >&2
  exit 1
fi

if [[ ! -d "/Volumes/Extreme SSD" ]]; then
  echo "error: Extreme SSD is not mounted" >&2
  exit 1
fi

if [[ -d "$SSD_DEST" ]]; then
  echo "error: $SSD_DEST already exists — remove it first if you want to re-archive" >&2
  exit 1
fi

# ── Copy ─────────────────────────────────────────────────────────────────────

echo "copying $PROJECT_NAME → $SSD_DEST"
# --no-perms/--no-owner/--no-group: ExFAT doesn't support Unix metadata
# --exclude: skip macOS resource forks and DS_Store files ExFAT generates
rsync -rl --checksum --no-perms --no-owner --no-group \
  --exclude='._*' --exclude='.DS_Store' \
  "$SRC/" "$SSD_DEST/"

# ── Verify ───────────────────────────────────────────────────────────────────

echo "verifying..."
EXCLUDE_PATTERN='-path */.git/* -o -name ._* -o -name .DS_Store'
SRC_HASH=$(find "$SRC"      -type f ! \( -path '*/.git/*' -o -name '._*' -o -name '.DS_Store' \) -exec shasum {} \; | awk '{print $1}' | sort | shasum | awk '{print $1}')
DST_HASH=$(find "$SSD_DEST" -type f ! \( -path '*/.git/*' -o -name '._*' -o -name '.DS_Store' \) -exec shasum {} \; | awk '{print $1}' | sort | shasum | awk '{print $1}')

if [[ "$SRC_HASH" != "$DST_HASH" ]]; then
  echo "error: checksum mismatch — aborting, local copy untouched" >&2
  rm -rf "$SSD_DEST"
  exit 1
fi

echo "verified ok ($SRC_HASH)"

# ── Index in Obsidian ────────────────────────────────────────────────────────

# Fetch GitHub description if available
GH_DESC=$(gh repo view "89jobrien/$PROJECT_NAME" --json description --jq '.description' 2>/dev/null || echo "")

if [[ ! -f "$OBSIDIAN_INDEX" ]]; then
  cat > "$OBSIDIAN_INDEX" <<'EOF'
# Archived Projects

Projects removed from the machine and stored on the Extreme SSD under `Macbook/`.

| project | archived | ssd path | description |
|---------|----------|----------|-------------|
EOF
fi

# Append row if not already present
if ! grep -q "| $PROJECT_NAME |" "$OBSIDIAN_INDEX"; then
  printf "| %s | %s | %s | %s |\n" \
    "$PROJECT_NAME" \
    "$ARCHIVE_DATE" \
    "Macbook/$PROJECT_NAME" \
    "${GH_DESC:-—}" \
    >> "$OBSIDIAN_INDEX"
  echo "indexed in $OBSIDIAN_INDEX"
fi

# ── Remove local ─────────────────────────────────────────────────────────────

echo "removing $SRC"
rm -rf "$SRC"

echo "done — $PROJECT_NAME archived to SSD and removed from ~/dev"
