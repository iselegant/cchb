#!/usr/bin/env bash
# .claude/hooks/block-destructive.sh
# PreToolUse hook: blocks forbidden commands in agent sessions.
set -euo pipefail

input="$(cat)"
cmd="$(jq -r '.tool_input.command // empty' <<< "$input")"

# Rust - publishing (irreversible). `--dry-run` is read-only and is allowed
# so the agent can validate packaging locally before a human runs the real publish.
case "$cmd" in
  *"cargo publish"*)
    case "$cmd" in
      *"--dry-run"*) ;;
      *)
        echo "BLOCKED: 'cargo publish' is prohibited in agent sessions. Use 'cargo publish --dry-run' for local validation, or publish via CI/CD pipeline." >&2
        exit 2
        ;;
    esac
    ;;
esac
