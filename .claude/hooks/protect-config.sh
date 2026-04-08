#!/usr/bin/env bash
# .claude/hooks/protect-config.sh
# PreToolUse hook: prevents agent from modifying linter/toolchain config files.
set -euo pipefail

input="$(cat)"
file="$(jq -r '.tool_input.file_path // .tool_input.path // empty' <<< "$input")"

PROTECTED=(
  "rustfmt.toml"            # Rust formatter config
  ".rustfmt.toml"           # Rust formatter config (dot-prefixed)
  "clippy.toml"             # Rust linter config
  ".clippy.toml"            # Rust linter config (dot-prefixed)
  "rust-toolchain"          # Rust toolchain version pin
  "rust-toolchain.toml"     # Rust toolchain version pin
)

for p in "${PROTECTED[@]}"; do
  case "$file" in
    *"$p"*)
      echo "BLOCKED: $file is a protected config file. Fix the code, not the linter config." >&2
      exit 2
      ;;
  esac
done
