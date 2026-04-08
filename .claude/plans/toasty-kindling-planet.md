# Fix: Sessions after early March not showing in cchist

## Context

cchist currently finds only 116 sessions, but `~/.claude/history.jsonl` has 718 unique sessions (192 in March, 44 in April). The root cause is that `discover_sessions()` skips JSONL file scanning when `sessions-index.json` exists, but those index files are stale (only contain entries up to Feb 2026).

### Data analysis

| Category | Count | Visible? |
|----------|-------|----------|
| March+ sessions with JSONL in **indexed** projects | 60 | **NO** — index is stale, JSONL scan skipped |
| March+ sessions with JSONL in **non-indexed** projects | 113 | YES — fallback works |
| March+ sessions with **no JSONL** file at all | 63 | NO — no data to display |

### Root cause

In `session.rs:237-243`:
```rust
if index_path.exists()
    && let Ok(index_sessions) = load_sessions_from_index(...)
{
    sessions.extend(index_sessions);
    continue;  // ← Skips JSONL scan entirely!
}
```

When `sessions-index.json` exists, cchist trusts it as complete and skips scanning for `.jsonl` files. But Claude Code adds new session `.jsonl` files without updating the index, so any session created after the last index update is invisible.

## Fix

**Merge index and JSONL scan results** instead of treating them as mutually exclusive.

### Changes to `src/session.rs`

1. In `discover_sessions()`: Remove the `continue` after loading from index. Always run JSONL scan, then merge results (index entries take priority to avoid duplicates).

2. Specifically:
   - Load from index (if exists) → collect session IDs into a `HashSet`
   - Always scan JSONL files → skip any already in the index set
   - Combine both result sets

### Implementation detail

```rust
// In discover_sessions(), replace the current either/or logic:
let mut seen_ids: HashSet<String> = HashSet::new();

// Try sessions-index.json fast path
if index_path.exists() {
    if let Ok(index_sessions) = load_sessions_from_index(...) {
        for s in &index_sessions {
            seen_ids.insert(s.session_id.clone());
        }
        sessions.extend(index_sessions);
    }
}

// Always scan JSONL files for sessions not in the index
if let Ok(fallback_sessions) = load_sessions_from_jsonl_scan(...) {
    for s in fallback_sessions {
        if !seen_ids.contains(&s.session_id) {
            sessions.push(s);
        }
    }
}
```

### Files to modify

- `src/session.rs` — `discover_sessions()` function (lines 205-255)

### Tests

- Add test: `test_discover_sessions_merges_index_and_jsonl_scan` — project with index + extra JSONL file not in index → both discovered
- Add test: `test_discover_sessions_index_takes_priority_over_jsonl` — same session in both index and JSONL → index version used (no duplicates)

### Verification

1. `cargo test` — all pass
2. `cargo clippy` — no warnings
3. `cargo fmt --check` — passes
4. `cargo run --release` — verify session count is significantly higher than 116
