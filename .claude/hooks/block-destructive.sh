#!/usr/bin/env bash
# .claude/hooks/block-destructive.sh
# PreToolUse hook: blocks forbidden commands in agent sessions.
set -euo pipefail

input="$(cat)"
cmd="$(jq -r '.tool_input.command // empty' <<< "$input")"

BLOCKED_PATTERNS=(
  # Rust - publishing (irreversible)
  "cargo publish"
)

for pattern in "${BLOCKED_PATTERNS[@]}"; do
  case "$cmd" in
    *"$pattern"*)
      echo "BLOCKED: '$pattern' is prohibited in agent sessions. Publish via CI/CD pipeline." >&2
      exit 2
      ;;
  esac
done
