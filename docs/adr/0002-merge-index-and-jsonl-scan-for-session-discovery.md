# ADR-0002: Merge Index and JSONL Scan for Session Discovery

## Status

Accepted

## Context

`discover_sessions()` currently treats `sessions-index.json` and JSONL file scanning as **mutually exclusive** paths. When a project directory contains `sessions-index.json`, the function loads sessions from it and skips JSONL scanning entirely via `continue`.

This design assumes `sessions-index.json` is always complete and up-to-date. However, Claude Code frequently adds new session `.jsonl` files without updating the index. Analysis of actual data shows:

| Metric | Value |
|--------|-------|
| Total unique sessions in `history.jsonl` | 718 |
| Sessions discovered by cchist | 116 |
| March+ sessions with JSONL in indexed projects (invisible) | 60 |
| March+ sessions with JSONL in non-indexed projects (visible) | 113 |

The 60 sessions in indexed projects are invisible because `sessions-index.json` exists (with stale data up to Feb 2026) and the JSONL scan is skipped.

## Decision

Change `discover_sessions()` to **merge** results from both `sessions-index.json` and JSONL file scanning, instead of treating them as either/or:

1. Load sessions from `sessions-index.json` when available (fast path).
2. **Always** scan for `.jsonl` files in the same directory.
3. Use a `HashSet<String>` of session IDs from the index to deduplicate — index entries take priority when a session appears in both sources.

This approach:
- Preserves the fast path benefit (index provides pre-parsed metadata).
- Catches any sessions added after the index was last updated.
- Avoids duplicates by tracking seen session IDs.

## Consequences

- **Positive**: All sessions with `.jsonl` files are discovered regardless of index staleness. Users see the complete session list.
- **Negative**: Slight performance overhead from always running JSONL scan even when an index exists. This is acceptable because the JSONL scan only reads the first 50 lines of each file for metadata extraction.
- **Neutral**: Sessions that exist only in `history.jsonl` (with no `.jsonl` file) remain undiscoverable. This is out of scope for this change as there is no conversation data to display.
