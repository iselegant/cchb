# ADR-0004: Cargo.toml as the Single Source of Truth for Version

## Status

Accepted

## Context

Releases v0.9.2 and v0.9.3 were tagged and published without bumping `Cargo.toml`, which remained at `0.9.1`. The release pipeline has two independent paths that derive a "version":

1. The build job runs `cargo build --release`, which embeds `Cargo.toml`'s `version` field into the binary. With `--version` introduced in this codebase, the binary prints whatever `Cargo.toml` said at build time.
2. The `update-tap` job derives `VERSION="${REF_NAME#v}"` from the pushed git tag and writes that value into the Homebrew formula's URL and `version` field.

When the two diverge — i.e. `Cargo.toml` is not bumped before tagging — the published artifacts are internally inconsistent: Homebrew advertises v0.9.3, but the binary inside the v0.9.3 release archive identifies itself as 0.9.1. Users see this divergence the moment they run `cchb --version`.

The bump step is currently a manual checklist item, and v0.9.2/v0.9.3 demonstrate that relying on humans to remember it is not enough.

## Decision

- **`Cargo.toml`'s `version` field is the single source of truth.** Every release artifact — the binary, the GitHub release, and the Homebrew formula — must derive its version from `Cargo.toml`, directly or indirectly.
- **Tag names must equal `Cargo.toml`'s version.** Specifically, for a tag `vX.Y.Z` the file must declare `version = "X.Y.Z"` at that commit.
- **The release CI verifies this invariant before any build runs.** A `verify-version` job compares `${REF_NAME#v}` against the version parsed from `Cargo.toml` and fails the workflow on mismatch. The `build` job depends on `verify-version`, so a mismatched tag never produces release artifacts.
- **The `update-tap` job is left unchanged.** It continues to derive its version from the tag name; because `verify-version` guarantees tag and `Cargo.toml` agree, the formula will always match the binary.

## Consequences

- **Positive**: A forgotten `Cargo.toml` bump is caught at the release workflow's first step, before any binary is built or published. The cost of the mistake drops from "republish a release" to "delete the tag and try again".
- **Positive**: Future paths (e.g. `cargo install cchb`, crates.io publishing, `mise` resolution) will all see the same version because they all read `Cargo.toml`.
- **Positive**: `--version` output is trustworthy for support and bug reports.
- **Neutral**: The bump-then-tag sequence remains manual. This ADR does not adopt automation tools like release-please or cargo-release; doing so would require a follow-up decision and is out of scope. The verify step catches the omission but does not prevent it.
- **Negative**: When mismatch is detected, the operator must delete the bad tag (e.g. `git push --delete origin vX.Y.Z`), bump `Cargo.toml`, commit, and re-tag. This is acceptable friction given that the alternative is shipping inconsistent artifacts.
