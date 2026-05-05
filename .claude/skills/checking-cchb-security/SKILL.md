---
name: checking-cchb-security
description: Performs cchb-specific security audit covering subprocess execution, path handling, JSONL parsing, sensitive-data leakage, Rust safety, supply chain (with active CVE lookup against RUSTSEC and crates.io), CI/CD workflows (with active GitHub Actions advisory lookup), and security policy. Read-only — produces a structured Markdown report. Triggers on "cchb security check", "cchbセキュリティチェック", "audit cchb", "cchb security audit", or `/cchb-security-check`.
---

# Checking cchb Security

Repository-specific security audit for the `cchb` Rust TUI. The attack surface of `cchb` is **local files + subprocess + clipboard**, not network — so generic web-app scanners miss what matters here. This skill orchestrates two specialized subagents in parallel and aggregates their findings.

## When to Use

- Before opening a PR that touches `src/main.rs`, `src/session.rs`, `src/event.rs`, `Cargo.toml`, or `.github/workflows/*`.
- Before cutting a release tag.
- On request: "run a security audit", "cchb security check", "脆弱性チェック".

Do **not** use this for unrelated repositories — it is hard-coded to cchb's structure.

## Pre-flight

Before invoking subagents, verify the working directory is the cchb repo root:

```bash
test -f Cargo.toml && grep -q '^name = "cchb"' Cargo.toml
```

If it fails, abort with: `Not in cchb repository root. Aborting.`

Also capture environment metadata for the report header:

```bash
git rev-parse --short HEAD
git rev-parse --abbrev-ref HEAD
cargo --version
rustc --version
date -u +%Y-%m-%dT%H:%M:%SZ
```

## Workflow

1. Run the pre-flight check above.
2. Launch **both** subagents in parallel — single message, two `Agent` tool calls:
   - `subagent_type: cchb-code-security-auditor` — categories A (subprocess), B (path), C (parsing), D (sensitive data), E (Rust safety), plus H2/H3 (docs).
   - `subagent_type: cchb-supply-chain-auditor` — categories F (deps with active CVE lookup), G (CI/CD with active GH advisory lookup), plus H1 (SECURITY.md).
3. Each subagent returns findings as a JSON array (schema below).
4. Aggregate, sort by severity (Critical → High → Medium → Low → Info) then by category id.
5. Render the final Markdown report to stdout. Do **not** write to files (read-only policy).

### Subagent prompt template

When invoking each subagent, pass:

- The repository root path (current working directory).
- The git ref captured in pre-flight.
- Instruction: "Return findings as a JSON array conforming to the schema in SKILL.md. Output the JSON array first, then a brief human summary."

## Findings schema

Each finding object must contain:

| Field | Type | Notes |
|---|---|---|
| `id` | string | Check id, e.g. `A1`, `F0`, `G0` |
| `category` | string | One of `A`–`H` |
| `severity` | string | `Critical` \| `High` \| `Medium` \| `Low` \| `Info` |
| `title` | string | One-line summary |
| `location` | string | `path:line` for code; `Cargo.toml`, `.github/workflows/release.yml`, etc. for config |
| `evidence` | string | Up to 8 lines of relevant code/config excerpt |
| `recommendation` | string | Concrete fix (file to change, command to run, version to bump to) |
| `references` | string[] | Optional: advisory ids (RUSTSEC-YYYY-NNNN, CVE-YYYY-NNNNN, GHSA-xxxx) and URLs |

When no issues are found for a check id, emit a finding with `severity: Info` and `title: "<id> passed"` so the report shows coverage.

## Report layout

```markdown
# cchb Security Audit Report

- **Repo**: cchb @ <short-sha> (branch: <branch>)
- **Toolchain**: <rustc version>
- **Generated**: <UTC timestamp>

## Summary

| Severity | Count |
|---|---|
| Critical | N |
| High | N |
| Medium | N |
| Low | N |
| Info | N |

## Findings

### [Severity] [id] — Title
- **Category**: <A–H>
- **Location**: `path:line`
- **Evidence**:
  ```
  <excerpt>
  ```
- **Recommendation**: <fix>
- **References**: <advisory ids if any>

(repeat per finding, severity-descending)

## Suggested Next Steps

1. Address Critical/High items first — list top 3 with proposed PRs.
2. Schedule Medium fixes for next minor release.
3. File Info items as backlog if not already tracked.
```

## Examples

### Example 1: Pre-PR audit on a feature branch

User runs `/cchb-security-check` after editing `src/event.rs` (clipboard handling).

Expected behavior:
- Pre-flight passes.
- Both subagents launch in parallel.
- Code auditor flags any new clipboard write that bypasses the existing `tool_use` filter (D2).
- Supply chain auditor reports current state for F0/F1/G0 (likely Info if no new deps).
- Report renders in under ~60s for a clean tree.

### Example 2: Pre-release audit on `main`

User runs the skill before cutting `v0.10.0`.

Expected behavior:
- Subagents run as above.
- F0 actively queries RUSTSEC for every direct dep version in `Cargo.lock`.
- G0 fetches latest releases for every `uses:` action in `.github/workflows/*.yml` and flags EOL versions (e.g. `actions/checkout@v3` would be High; `@v4` is current).
- Report includes a final "release readiness" line listing any Critical/High blockers.

## Evaluation Scenarios

### Scenario 1 — Happy path
Run on a clean cchb tree. Expect: report generated, no Critical, at most a few Mediums (likely H1 SECURITY.md missing, F2 no `cargo audit` step), all Info checks present.

### Scenario 2 — Injected weakness detection
Temporarily add `unsafe { … }` to `src/main.rs` and a `Command::new("sh").arg("-c").arg(format!(...))` call. Expect: code auditor flags E1 (unsafe without justification) and A2 (shell-mediated invocation) as Critical/High.

### Scenario 3 — Wrong repository
Run from `~/Documents` (non-cchb dir). Expect: pre-flight fails, abort message printed, no subagents launched.

### Scenario 4 — Active vulnerability lookup
On a tree where `Cargo.lock` contains a crate version with a known RUSTSEC advisory, F0 must produce a High finding citing the advisory id and the fixed-version range. If `cargo audit` is not installed locally, the WebFetch fallback to `https://rustsec.org/` or crates.io API must still produce the finding.

## Error Handling

- **Subagent timeout / failure**: Mark the missing categories as `Info: "skipped (subagent error)"` in the report and continue rendering. Do not abort the whole skill.
- **Network unavailable**: Active checks (F0, F1, G0) downgrade to Info with note: `"network/CLI unavailable, skipped"`. Static checks (F2–F6, G1–G6, H1) still run.
- **Conflicting findings between subagents**: De-duplicate by `id`; if both report the same id, keep the higher severity entry.

## Quality Gates

The skill is considered passing if:
- All A–H category ids appear at least once in the report (each as a finding or an Info "passed" entry).
- Subagents ran in parallel (verify by interleaved tool-call timestamps).
- Report length is bounded (target ≤ 600 lines for a typical tree).
