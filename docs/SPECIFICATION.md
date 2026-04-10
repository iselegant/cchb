# cchb - Claude Code Session History Browser

## Overview

A Rust-based TUI CLI tool for browsing and restoring past Claude Code session information.
Inspired by [ccresume](https://github.com/sasazame/ccresume), it provides session listing, conversation preview, fuzzy search, and date range filtering capabilities.

## Data Source

### Claude Code Session Storage

Claude Code stores session data in the following locations:

| Path | Format | Content |
|------|--------|---------|
| `~/.claude/projects/<encoded-path>/<sessionId>.jsonl` | JSONL | Session conversation data |
| `~/.claude/projects/<encoded-path>/sessions-index.json` | JSON | Session metadata (when available) |
| `~/.claude/history.jsonl` | JSONL | Global history index |

### Path Encoding

Project directories are stored with dash-encoded original paths:

```
/Users/foo/Documents/project → -Users-foo-Documents-project
```

### Session JSONL Message Types

Each JSONL file consists of one message per line:

| type | role | Content | Display Target |
|------|------|---------|----------------|
| `user` | `user` | User input text | Yes |
| `assistant` | `assistant` | AI response (text/thinking/tool_use blocks) | text blocks only |
| `file-history-snapshot` | - | File tracking snapshots | No |
| `system` | - | System messages | No |
| `agent-name` | - | Agent names | No |
| `custom-title` | - | Custom titles | No |

#### User Message Structure

```json
{
  "type": "user",
  "uuid": "message-uuid",
  "parentUuid": "parent-uuid or null",
  "isSidechain": false,
  "message": {
    "role": "user",
    "content": "User text (string or array of content blocks)"
  },
  "timestamp": "2026-04-08T13:47:45.241Z",
  "cwd": "/current/working/directory",
  "sessionId": "uuid",
  "version": "2.1.85",
  "gitBranch": "main",
  "slug": "human-readable-session-name"
}
```

#### Assistant Message Structure

```json
{
  "type": "assistant",
  "uuid": "message-uuid",
  "parentUuid": "parent-uuid",
  "message": {
    "role": "assistant",
    "model": "claude-opus-4-6",
    "content": [
      { "type": "thinking", "thinking": "", "signature": "..." },
      { "type": "text", "text": "Response text" },
      { "type": "tool_use", "id": "toolu_...", "name": "ToolName", "input": {} }
    ],
    "stop_reason": "end_turn",
    "usage": { "input_tokens": 100, "output_tokens": 200 }
  },
  "timestamp": "2026-04-08T13:47:50.000Z"
}
```

#### sessions-index.json Structure (Fast Path)

```json
{
  "version": "1",
  "originalPath": "/Users/foo/project",
  "entries": [
    {
      "sessionId": "uuid",
      "fullPath": "/path/to/session.jsonl",
      "fileMtime": 1234567890,
      "firstPrompt": "First user message",
      "summary": "Session summary",
      "messageCount": 42,
      "created": "2026-04-08T10:00:00Z",
      "modified": "2026-04-08T12:00:00Z",
      "gitBranch": "main",
      "projectPath": "/Users/foo/project",
      "isSidechain": false
    }
  ]
}
```

### Display Rules

- **Displayed**: `type: "user"` text + `type: "assistant"` `text` blocks only
- **Hidden**: `thinking` blocks, `tool_use` blocks, `tool_result`, sidechains (`isSidechain: true`)
- **Message order**: File appearance order (follows parentUuid/uuid chain)

## Functional Requirements

### FR-1: Session List Panel (Left Panel)

- List all sessions across all projects
- Each session entry displays:
  - Project name (last component of the path)
  - Date/time (creation date)
  - Git branch name
  - Preview of the first user message (truncated to 60 chars, shows "(no prompt)" if empty)
- Sorted by modified date in descending order
- Uses stateful list widget (`ListState`) for proper scroll tracking — selected item is always visible
- Selected item is highlighted with `highlight_style` and `> ` indicator

### FR-2: Conversation Viewer (Right Panel)

- Display conversation content of the currently focused session
- Conversation is automatically loaded when the user navigates the session list (real-time preview)
- Loaded session is tracked by `loaded_session_id` to avoid redundant re-loading
- `Enter` / `l` switches to Viewing mode for dedicated scroll navigation
- Extract only important elements (per Display Rules)
- Visually distinguish user messages from assistant responses
- Word-wrap long text
- **Scroll clamping**: Scroll position is clamped so that content cannot scroll past the last line (prevents blank/empty view when at end of conversation)
- **Markdown Rendering**: Message text is parsed and rendered with visual styling:

| Markdown Element | Rendering |
|---|---|
| `# Heading` (H1/H2/H3) | Bold + color differentiation (Cyan/Blue/Green) |
| `**bold**` | Bold modifier |
| `*italic*` | Italic modifier |
| `` `inline code` `` | Yellow text on dark gray background |
| Fenced code blocks | White text on dark gray background, with language label |
| Tables | Box-drawing characters (│, ─, ┌, etc.) with header row in bold |
| Unordered lists (`-`, `*`) | Bullet character (•) with indentation |
| Ordered lists (`1.`) | Numbered with indentation |
| `[text](url)` | Underlined cyan text with URL shown |
| `---` (horizontal rule) | Line of ─ characters |

### FR-3: Vim Keybindings

| Key | Mode | Action |
|-----|------|--------|
| `j` / `Down` | Normal/Viewing | Panel-aware: select next session (Session panel) / scroll conversation down (Conversation panel) |
| `k` / `Up` | Normal/Viewing | Panel-aware: select previous session (Session panel) / scroll conversation up (Conversation panel) |
| `g` | Normal/Viewing | Panel-aware: jump to top of session list or conversation |
| `G` | Normal/Viewing | Panel-aware: jump to bottom of session list or conversation |
| `Right` | Normal | Next page of sessions (Session panel) / scroll conversation (Conversation panel) |
| `Left` | Normal | Previous page of sessions (Session panel) / scroll conversation (Conversation panel) |
| `Ctrl+d` | Normal/Viewing | Panel-aware: half-page scroll down in active panel |
| `Ctrl+u` | Normal/Viewing | Panel-aware: half-page scroll up in active panel |
| `Enter` | Normal | Open session (enter Viewing mode) / Toggle panel (Conversation panel) |
| `Enter` | Viewing | Toggle panel focus |
| `Esc` / `q` | Viewing | Return to list |
| `q` | Normal | Quit application |

### FR-4: Fuzzy Search (f / / key)

- Press `f` or `/` in Normal or Viewing mode to activate search mode (Viewing mode exits first)
- Substring matching against conversation content (case-insensitive)
- Search targets: full conversation content (all displayed user and assistant text blocks, excluding sidechains)
- **Metadata fallback**: When conversation content cache is not yet loaded, search falls back to session metadata (project name, first prompt, summary, git branch)
- Real-time filtering of session list as user types (immediate via metadata, refined when content cache loads)
- `Enter` to confirm, `Esc` to cancel
- **Active search indicator**: When a search query is active after confirmation, it is displayed in the status bar
- **Conversation highlight**: Matching substrings in the conversation view are highlighted (case-insensitive, black text on yellow background)

### FR-5: Date Range Filter (d key)

- Press `d` in Normal or Viewing mode to activate date filter mode (Viewing mode exits first)
- **Preset values**: `From` defaults to 7 days before today, `To` defaults to today
- **Up/Down cursor keys**: Increment/decrement the active date field by 1 day
- Manual text input is also supported (YYYY-MM-DD format)
- `Tab` to switch between From/To fields
- `Enter` to apply, `Esc` to cancel

### FR-6: Help Overlay (h key)

- Press `h` or `?` to display help overlay
- Show complete keybinding reference
- Press any key to dismiss

### FR-7: Additional Shortcuts

| Key | Mode | Action |
|-----|------|--------|
| `Tab` | Normal/Viewing | Toggle focus between left/right panels |
| `/` | Normal/Viewing | Fuzzy search (same as `f` key) |
| `c` | Normal | Clear all filters |
| `l` | Viewing | Reload conversation (re-read JSONL file) |
| `R` (Shift+r) | Normal | Reload session list |
| `[` / `]` | Viewing | Navigate to previous/next session without returning to list |
| `n` / `N` | Normal/Viewing | Jump to next/previous search match — navigates within session matches first, then crosses to next/previous session when at the boundary. In Normal mode with Session panel, auto-enters Viewing mode. Wraps around session list. |

### FR-8: Session Restore (r key)

- Press `r` in Normal or Viewing mode to restore the currently selected session
- Exits cchb TUI, then launches `claude --resume <session-id>` via `exec`
- Terminal is properly restored before launching Claude
- If no session is selected, the key press is ignored

## Non-Functional Requirements

### NFR-1: Color Scheme

Color scheme designed for readability:

| Element | Color |
|---------|-------|
| Project name | Cyan |
| Date/time | Dim White |
| Branch name | Green |
| Message preview | Gray/Dim |
| User label "You:" | Bold Green |
| User message border | Green ("│" left border with "└─" terminator) |
| Assistant label "Claude:" | Bold Magenta |
| Assistant message border | Magenta ("│" left border with "└─" terminator) |
| Selected row | Reverse/Highlight |
| Search input | Yellow |
| Active panel border | Bright |
| Inactive panel border | Dim |

### NFR-2: Performance

- Session discovery merges `sessions-index.json` (fast path) with JSONL file scanning
  - Index entries are loaded first and take priority (pre-parsed metadata)
  - JSONL scan always runs to catch sessions not in the index (deduplication by session ID)
  - See ADR-0002 for rationale
- Conversation content is lazy-loaded on selection (not parsed at startup)
- LRU cache holds the last 3-5 session conversations in memory

### NFR-3: Security

- tool_use blocks (which may contain file paths, credentials) are not displayed
- No special handling for clipboard copy of sensitive data (user responsibility)

### NFR-4: Robustness

- Empty session files: skip silently
- Sessions with no conversation messages (message_count == 0): excluded from listing
- Sessions without meaningful prompts: excluded from listing
  - Empty or whitespace-only `firstPrompt`
  - Placeholder text (`"No prompt"`)
  - Error-only prompts (starting with `<local-command-stderr>`)
- Sessions whose JSONL file no longer exists on disk: excluded from listing (index path only)
- Malformed JSONL lines: skip without crashing
- Very long messages: truncate in list view, wrap in conversation view
- Unicode/CJK text: render correctly
- Missing `~/.claude` directory: show friendly error message

## Technical Stack

| Component | Crate | Version | Purpose |
|-----------|-------|---------|---------|
| TUI framework | ratatui | 0.30 | UI rendering |
| Terminal backend | crossterm | 0.29 | Input/output control |
| Serialization | serde + serde_json | 1.0 | JSONL parsing |
| Date/time | chrono | 0.4 | Timestamps, date filtering |
| Directories | directories | 5 | Platform-standard paths |
| Fuzzy search | nucleo | 0.5 | fzf-like search |
| Markdown parser | pulldown-cmark | 0.12 | CommonMark parsing for rich text rendering |
| Error handling | anyhow | 1 | Error chaining |

## UI Layout

```
┌─ cchb - Claude Code History ──────────────────┐
├──────────────────┬──────────────────────────────┤
│ Sessions (35%)   │ Conversation (65%)           │
│                  │                              │
│ > project-a      │ │ You:                       │
│   (main)         │ │   Run terraform plan       │
│   2026-04-08     │ └─                           │
│   "terraform..." │                              │
│                  │ │ Claude:                     │
│   project-b      │ │   Here are the results...  │
│                  │ └─                           │
│   project-b      │                              │
│   (feature/x)    │                              │
│   2026-04-07     │                              │
│   "API design.." │                              │
├──────────────────┴──────────────────────────────┤
│ 42 sessions │ f:search d:date h:help q:quit     │
└─────────────────────────────────────────────────┘
```

## Module Architecture

```
src/
  main.rs       -- Entry point, terminal setup/teardown, panic handler
  app.rs        -- AppState, AppMode, Panel, state transitions
  session.rs    -- SessionIndex, ConversationMessage, ContentBlock,
                   discover_sessions(), load_conversation(), display_messages()
  ui.rs         -- render(), layout construction, widget rendering, overlays
  markdown.rs   -- Markdown-to-ratatui rendering (pulldown-cmark event mapping)
  event.rs      -- Event loop, key dispatch by mode
  filter.rs     -- SearchEngine (nucleo), date range filter, combined filter
  color.rs      -- Theme struct, default_theme()
```

## Development Approach

TDD (Test-Driven Development):
1. Write tests first
2. Verify tests fail (Red)
3. Implement minimum code to pass tests (Green)
4. Refactor

Verification after each phase:
- `cargo test` -- all tests pass
- `cargo clippy` -- no warnings
- `cargo fmt --check` -- passes

## Implementation Phases

### Phase 1: Project Initialization
- `cargo init --name cchb`
- Add dependencies to Cargo.toml
- Create module skeleton files

### Phase 2: Data Layer (`session.rs`) - Tests First
1. Write tests: JSONL parsing, path decoding, message filtering
2. Define types: `SessionIndex`, `ConversationMessage`, `ContentBlock`
3. Implement: `discover_sessions()`, `load_conversation()`, `display_messages()`
4. sessions-index.json fast path + JSONL fallback

### Phase 3: App State (`app.rs`) - Tests First
1. Write tests: state transitions (selection movement, mode switching, scrolling)
2. Define types: `AppState`, `AppMode`, `Panel`
3. Implement: state transition methods

### Phase 4: Filter Engine (`filter.rs`) - Tests First
1. Write tests: fuzzy search, date filter, combined filter
2. Implement: nucleo integration, date range filtering

### Phase 5: Color Theme (`color.rs`)
- Define theme constants (no tests needed)

### Phase 6: UI Rendering (`ui.rs`)
- Layout construction (35%/65% split)
- Session list rendering
- Conversation viewer rendering
- Search/date/help overlays

### Phase 7: Event Handling (`event.rs`) - Tests First
1. Write tests: key dispatch
2. Implement event loop
3. Mode-specific key handling

### Phase 8: Main Entry Point (`main.rs`)
- Terminal initialization/restoration (including panic handler)
- Session discovery -> sort -> state creation -> event loop

### Phase 9: Integration Tests & Polish
- Integration tests (with mock data)
- Edge case handling (empty files, malformed JSON, long text, CJK text)
- Performance optimization (lazy loading, LRU cache)

## Distribution

### Binary Release

The tool is distributed as a pre-built binary, not via `cargo install` from source.

- Build optimized release binary with `cargo build --release`
- Binary is output to `target/release/cchb`
- Users install by placing the binary in their `$PATH` (e.g., `/usr/local/bin/`)
- GitHub Releases are used for distribution with pre-built binaries for each platform

### Supported Platforms

| Platform | Target |
|----------|--------|
| macOS (Apple Silicon) | `aarch64-apple-darwin` |
| macOS (Intel) | `x86_64-apple-darwin` |
| Linux (x86_64) | `x86_64-unknown-linux-gnu` |

### CI/CD

- GitHub Actions workflow builds release binaries for all supported platforms on tag push
- Binaries are automatically attached to GitHub Releases
