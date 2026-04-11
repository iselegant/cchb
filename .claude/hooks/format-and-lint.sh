#!/usr/bin/env bash
# .claude/hooks/format-and-lint.sh
# PostToolUse hook: auto-formats with cargo fmt and returns clippy diagnostics.
set -euo pipefail

input="$(cat)"
file="$(jq -r '.tool_input.file_path // .tool_input.path // empty' <<< "$input")"
[ -z "$file" ] && exit 0

diagnostics=""

case "$file" in
  *.rs)
    # Find the Cargo.toml root
    dir="$(dirname "$file")"
    cargo_root=""
    d="$dir"
    while [ "$d" != "/" ] && [ "$d" != "." ]; do
      if [ -f "$d/Cargo.toml" ]; then
        cargo_root="$d"
        break
      fi
      d="$(dirname "$d")"
    done
    [ -z "$cargo_root" ] && exit 0

    # Auto-fix: cargo fmt
    (cd "$cargo_root" && cargo fmt 2>/dev/null) || true

    # Diagnostic: cargo clippy
    if command -v cargo >/dev/null 2>&1; then
      clippy_out="$(cd "$cargo_root" && cargo clippy --message-format short 2>&1 | grep -E '^(warning|error)' | head -15)" || true
      [ -n "${clippy_out:-}" ] && diagnostics="$clippy_out"
    fi
    ;;

  *.sh)
    # Diagnostic: shellcheck
    if command -v shellcheck >/dev/null 2>&1; then
      sc_out="$(shellcheck -f gcc "$file" 2>&1 | head -15)" || true
      [ -n "${sc_out:-}" ] && diagnostics="$sc_out"
    fi
    ;;

  .github/workflows/*.yml|.github/workflows/*.yaml)
    # Diagnostic: actionlint (single file)
    if command -v actionlint >/dev/null 2>&1; then
      al_out="$(actionlint "$file" 2>&1 | head -15)" || true
      [ -n "${al_out:-}" ] && diagnostics="$al_out"
    fi
    # Diagnostic: yamllint
    if command -v yamllint >/dev/null 2>&1; then
      yl_out="$(yamllint -f parsable "$file" 2>&1 | head -15)" || true
      [ -n "${yl_out:-}" ] && diagnostics="${diagnostics:+${diagnostics}\n}${yl_out}"
    fi
    ;;

  *.yml|*.yaml)
    # Diagnostic: yamllint
    if command -v yamllint >/dev/null 2>&1; then
      yl_out="$(yamllint -f parsable "$file" 2>&1 | head -15)" || true
      [ -n "${yl_out:-}" ] && diagnostics="$yl_out"
    fi
    ;;

  *) exit 0 ;;
esac

if [ -n "${diagnostics:-}" ]; then
  jq -Rn --arg msg "$diagnostics" '{
    hookSpecificOutput: {
      hookEventName: "PostToolUse",
      additionalContext: $msg
    }
  }'
fi
