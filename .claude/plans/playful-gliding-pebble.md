# Harness Engineering for cchist (Rust TUI Project)

## Context

The cchist project's CLAUDE.md enforces `cargo clippy` and `cargo fmt --check` via prompt instructions — this is the "95% enforcement" pattern. The global `~/.claude/hooks/` already implements harness engineering for infrastructure code (Terraform, K8s, YAML, Docker, Shell, Makefile), but **Rust is not covered**. We need to close the 5% gap with mechanical enforcement.

## Current State

- **Global hooks exist** at `~/.claude/hooks/` with `format-and-lint.sh` (PostToolUse), `block-destructive.sh` (PreToolUse), `pre-protect-config.sh` (PreToolUse), `stop-infra-validate.sh` (Stop)
- **CI already enforces**: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo build --release`
- **No Rust-specific hooks exist** — the `format-and-lint.sh` case statement falls through to `exit 0` for `.rs` files
- **rust-analyzer LSP** is enabled globally

## Design: Three-Layer Harness for Rust

### L1: PostToolUse — Per-Edit (ms)

**Modify**: `~/.claude/hooks/format-and-lint.sh`

Add `*.rs` case before the `*) exit 0` fallthrough:

```bash
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

  # Auto-fix: cargo fmt (single file)
  (cd "$cargo_root" && cargo fmt 2>/dev/null) || true

  # Diagnostic: cargo clippy (fast, single check)
  if command -v cargo >/dev/null 2>&1; then
    clippy_out="$(cd "$cargo_root" && cargo clippy --message-format short 2>&1 | grep -E '^(warning|error)' | head -15)" || true
    [ -n "${clippy_out:-}" ] && diagnostics="$clippy_out"
  fi
  ;;
```

**Effect**: Every `.rs` file edit auto-formats and returns clippy diagnostics. The agent self-corrects immediately.

### L2: Stop Hook — Before Completion (s)

**Modify**: `~/.claude/hooks/stop-infra-validate.sh`

Add Rust checks after the Terraform section:

```bash
# --- Rust checks ---
if echo "$changed" | grep -q '\.rs$'; then
  cargo_root="$(git rev-parse --show-toplevel)"
  if [ -f "$cargo_root/Cargo.toml" ]; then
    errors=""

    # cargo fmt --check
    fmt_out="$(cd "$cargo_root" && cargo fmt --check 2>&1)" || true
    [ -n "$fmt_out" ] && errors="${errors}=== cargo fmt ===${fmt_out}\n"

    # cargo clippy (deny warnings)
    clippy_out="$(cd "$cargo_root" && cargo clippy -- -D warnings 2>&1 | tail -20)" || true
    if echo "$clippy_out" | grep -q '^error'; then
      errors="${errors}=== cargo clippy ===\n${clippy_out}\n"
    fi

    # cargo test
    test_out="$(cd "$cargo_root" && cargo test 2>&1 | tail -20)" || true
    if echo "$test_out" | grep -q 'FAILED\|error\['; then
      errors="${errors}=== cargo test ===\n${test_out}\n"
    fi

    if [ -n "$errors" ]; then
      printf '%b' "$errors"
      exit 1
    fi
  fi
fi
```

**Effect**: Before the agent declares "done", it must pass fmt + clippy + test. Failures block completion.

### L3: CI (already exists)

`.github/workflows/ci.yml` already runs `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, and `cargo build --release`. No changes needed.

### Guardrails: Block `cargo publish`

**Modify**: `~/.claude/hooks/block-destructive.sh`

Add to `BLOCKED_PATTERNS`:

```bash
# Rust - publishing (irreversible)
"cargo publish"
```

**Effect**: Prevents accidental crate publication from agent sessions.

### Config Protection: Protect Rust toolchain configs

**Modify**: `~/.claude/hooks/pre-protect-config.sh`

Add to `PROTECTED` array:

```bash
"rustfmt.toml"    # Rust formatter config
".rustfmt.toml"   # Rust formatter config (dot-prefixed)
"clippy.toml"     # Rust linter config
".clippy.toml"    # Rust linter config (dot-prefixed)
"rust-toolchain"  # Rust toolchain version pin
"rust-toolchain.toml"  # Rust toolchain version pin
```

**Effect**: Agent cannot weaken linter rules or change toolchain to bypass checks.

### Stop Hook Rename

The stop hook currently handles more than just infra. Rename for clarity:

- `stop-infra-validate.sh` → `stop-quality-gate.sh`
- Update reference in `~/.claude/settings.json`

## Summary: Risk Classification for Rust

| Action | Classification | Mechanism |
|--------|---------------|-----------|
| `cargo fmt` | ALLOW (auto-fix) | L1 PostToolUse |
| `cargo clippy` | VALIDATE (diagnostic) | L1 PostToolUse + L2 Stop |
| `cargo test` | VALIDATE (gate) | L2 Stop |
| `cargo publish` | BLOCK (irreversible) | PreToolUse |
| Edit `clippy.toml`/`rustfmt.toml` | BLOCK (config tamper) | PreToolUse |
| Edit `.rs` files | ALLOW | No restriction |

## Files to Modify

1. `~/.claude/hooks/format-and-lint.sh` — Add `*.rs` case
2. `~/.claude/hooks/stop-infra-validate.sh` → rename to `stop-quality-gate.sh`, add Rust checks
3. `~/.claude/hooks/block-destructive.sh` — Add `cargo publish`
4. `~/.claude/hooks/pre-protect-config.sh` — Add Rust config files
5. `~/.claude/settings.json` — Update stop hook filename

## Verification

1. Edit a `.rs` file → verify `cargo fmt` auto-runs and clippy diagnostics appear
2. Introduce a clippy warning → verify PostToolUse returns diagnostic feedback
3. Try `cargo publish` via Bash → verify hard block
4. Try editing `clippy.toml` → verify hard block
5. Complete a task with failing test → verify Stop hook blocks completion
