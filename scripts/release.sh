#!/usr/bin/env bash
# Release helper: bump Cargo.toml, commit, tag, and push.
# Usage: scripts/release.sh <patch|minor|major>
set -euo pipefail

BUMP="${1:-}"
if [[ -z "$BUMP" ]]; then
  echo "Usage: $0 <patch|minor|major>" >&2
  exit 1
fi

CARGO_TOML="Cargo.toml"
if [[ ! -f "$CARGO_TOML" ]]; then
  echo "Error: $CARGO_TOML not found. Run from the repository root." >&2
  exit 1
fi

CURRENT=$(sed -n 's/^version = "\(.*\)"/\1/p' "$CARGO_TOML" | head -n1)
if ! [[ "$CURRENT" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Error: cannot parse current version from $CARGO_TOML: '$CURRENT'" >&2
  exit 1
fi

IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

case "$BUMP" in
  patch) NEW="$MAJOR.$MINOR.$((PATCH + 1))" ;;
  minor) NEW="$MAJOR.$((MINOR + 1)).0" ;;
  major) NEW="$((MAJOR + 1)).0.0" ;;
  *)
    echo "Error: BUMP must be one of: patch, minor, major (got: '$BUMP')" >&2
    exit 1
    ;;
esac

echo "==> Releasing $CURRENT -> $NEW ($BUMP)"

# Pre-flight checks
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Error: working tree is not clean. Commit or stash changes first." >&2
  git status --short >&2
  exit 1
fi
BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [[ "$BRANCH" != "main" ]]; then
  echo "Error: not on main branch (current: $BRANCH)" >&2
  exit 1
fi
if git rev-parse "v$NEW" >/dev/null 2>&1; then
  echo "Error: tag v$NEW already exists" >&2
  exit 1
fi

# Confirm before any destructive action
echo ""
echo "About to perform the following changes:"
echo "  Cargo.toml version:  $CURRENT  ->  $NEW"
echo "  git commit:          chore(release): bump version to v$NEW"
echo "  git tag:             v$NEW"
echo "  git push:            origin main, origin v$NEW"
echo ""
if [[ ! -t 0 ]]; then
  echo "Error: refusing to run non-interactively (no TTY)." >&2
  echo "Re-run from an interactive shell." >&2
  exit 1
fi
read -r -p "Proceed? [y/N] " ANSWER
case "$ANSWER" in
  y|Y) ;;
  *)
    echo "Aborted."
    exit 1
    ;;
esac

# Bump (BSD/GNU sed compatible)
echo "==> Bumping $CARGO_TOML"
sed -i.bak -E "s/^version = \"[^\"]+\"/version = \"$NEW\"/" "$CARGO_TOML"
rm -f "$CARGO_TOML.bak"

echo "==> Refreshing Cargo.lock"
cargo build --quiet

echo "==> Running quality gates"
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --quiet

echo "==> Committing and tagging"
git add "$CARGO_TOML" Cargo.lock
git commit -m "chore(release): bump version to v$NEW"
git tag "v$NEW"

echo "==> Pushing to origin"
git push origin main
git push origin "v$NEW"

echo ""
echo "Released v$NEW. Watch CI: https://github.com/iselegant/cchb/actions"
