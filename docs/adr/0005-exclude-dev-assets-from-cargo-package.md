# ADR-0005: Exclude Development Assets from `cargo package`

## Status

Accepted

## Context

`cchb` is published as a crate on crates.io, so `cargo package` (and therefore `cargo publish`) determines what every consumer downloads. By default, Cargo includes every tracked file that is not gitignored. For this repository the default behaviour produced 35 files in the package tarball, of which roughly half are unrelated to building or running `cchb`:

- `.claude/hooks/*.sh`, `.claude/settings.json` — Claude Code development hooks and settings, only meaningful to contributors using Claude Code locally.
- `.github/workflows/*.yml`, `.github/pinact.yaml` — CI/CD configuration, only executed against the GitHub repository.
- `CLAUDE.md` — AI agent instructions for contributors, never read at build or run time.
- `Makefile`, `install.sh`, `scripts/bump-tap-formula.sh`, `scripts/release.sh` — release tooling and a `curl | sh` installer for users who do *not* go through Cargo.
- `docs/`, including `SPECIFICATION.md` and ADRs — repository-internal documentation. The canonical place for users to read these is the GitHub repository (linked from `Cargo.toml`'s `repository` and `homepage` fields), not a tarball pinned to a single version.

Shipping these files inflates the tarball, leaks development-only configuration to every crates.io mirror and downstream packager, and forces users of `cargo install cchb` to download assets they will never use.

`Cargo.toml` offers two mechanisms to control this: `include` (allow-list) and `exclude` (deny-list). `include` requires enumerating every source file and is easy to break when adding new modules — a missing entry silently produces a broken crate. `exclude` is additive on top of Cargo's default selection and stays correct as `src/` evolves.

## Decision

- **Use `[package].exclude` in `Cargo.toml`** to remove development-only assets from the published tarball. The list is:

  ```toml
  exclude = [
      ".claude/",
      ".github/",
      "CLAUDE.md",
      "Makefile",
      "install.sh",
      "scripts/",
      "docs/",
  ]
  ```

- **The package tarball contains only what is needed to build, test, and license the crate**: `Cargo.toml`, `Cargo.lock`, `LICENSE`, `README.md`, `src/`, `tests/`, plus the metadata files Cargo generates automatically (`.cargo_vcs_info.json`, `Cargo.toml.orig`, `.gitignore`).
- **`docs/` (including ADRs and `SPECIFICATION.md`) is excluded** even though it is human-readable. The GitHub repository — referenced from `Cargo.toml`'s `repository` and `homepage` fields — is the authoritative location for design documentation; embedding a frozen copy in every published version creates drift without benefit.
- **`README.md` and `LICENSE` remain included** because crates.io renders the README on the crate page and the license file is required by `Cargo.toml`'s `license` declaration to be reproducible alongside the source.

## Consequences

- **Positive**: The published tarball drops from 35 files to 17 — only build-relevant content reaches crates.io and downstream consumers.
- **Positive**: Development-only configuration (Claude Code hooks, CI workflows, release scripts) no longer leaks into mirrors, vendor directories, or `cargo install` caches.
- **Positive**: `exclude` is additive on Cargo's default selection, so adding new files under `src/` or `tests/` does not require updating `Cargo.toml`.
- **Negative**: Users browsing crates.io cannot read the ADRs or `SPECIFICATION.md` directly from the tarball; they must follow the `repository` link. This is acceptable because GitHub is already the canonical home for those documents.
- **Operational**: When introducing a new top-level directory or file that is development-only (e.g. a new `benches/` script that should not ship), the contributor must add it to `exclude`. Verify with `cargo package --list` before tagging a release.
