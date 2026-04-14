#!/usr/bin/env bash
# Fetches public repos from 89jobrien updated in the last 3 months
# and writes them to repos.json as a reference snapshot.

set -euo pipefail

OWNER="89jobrien"
CUTOFF=$(date -v-3m +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -d "3 months ago" +%Y-%m-%dT%H:%M:%SZ)
OUT="$(dirname "$0")/../repos.json"

gh repo list "$OWNER" \
  --visibility=public \
  --limit 100 \
  --json name,description,url,pushedAt,createdAt,primaryLanguage,repositoryTopics,stargazerCount,licenseInfo,latestRelease,homepageUrl,defaultBranchRef \
  | jq --arg cutoff "$CUTOFF" '
    [
      .[] | select(.pushedAt > $cutoff and .primaryLanguage.name == "Rust") |
      {
        name,
        description: (if .description == "" then null else .description end),
        url,
        pushed_at: .pushedAt,
        created_at: .createdAt,
        topics: [(.repositoryTopics // [])[].topic.name],
        stars: .stargazerCount,
        license: .licenseInfo.spdxId,
        latest_release: .latestRelease.tagName,
        homepage: (if .homepageUrl == "" then null else .homepageUrl end),
        default_branch: .defaultBranchRef.name
      }
    ] | sort_by(.pushed_at) | reverse
  ' > "$OUT"

echo "wrote $(jq length "$OUT") repos to $OUT"
