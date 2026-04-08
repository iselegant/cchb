#!/usr/bin/env bash
# .claude/hooks/stop-quality-gate.sh
# Stop hook (Layer 2): Final quality gate before agent completion.
# Runs cargo fmt --check, cargo clippy, and cargo test on changed .rs files.
set -euo pipefail

# Skip if not inside a git repository
git rev-parse --show-toplevel >/dev/null 2>&1 || exit 0

cd "$(git rev-parse --show-toplevel)"
changed="$(git diff --name-only HEAD)" || exit 0
[ -z "$changed" ] && exit 0

# --- Rust checks ---
if echo "$changed" | grep -q '\.rs$'; then
  cargo_root="$(git rev-parse --show-toplevel)"
  if [ -f "$cargo_root/Cargo.toml" ]; then
    errors=""

    # cargo fmt --check
    fmt_out="$(cd "$cargo_root" && cargo fmt --check 2>&1)" || true
    if [ -n "$fmt_out" ]; then
      errors="${errors}=== cargo fmt ===\n${fmt_out}\n\n"
    fi

    # cargo clippy (deny warnings)
    clippy_out="$(cd "$cargo_root" && cargo clippy -- -D warnings 2>&1 | tail -20)" || true
    if echo "$clippy_out" | grep -q '^error'; then
      errors="${errors}=== cargo clippy ===\n${clippy_out}\n\n"
    fi

    # cargo test
    test_out="$(cd "$cargo_root" && cargo test 2>&1 | tail -20)" || true
    if echo "$test_out" | grep -q 'FAILED\|error\['; then
      errors="${errors}=== cargo test ===\n${test_out}\n\n"
    fi

    if [ -n "$errors" ]; then
      printf '%b' "$errors"
      exit 1
    fi
  fi
fi
