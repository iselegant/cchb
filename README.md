# cchb - Claude Code History Browser

A fast TUI tool for browsing and resuming past [Claude Code](https://docs.anthropic.com/en/docs/claude-code) sessions.

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Release](https://img.shields.io/github/v/release/iselegant/cchb)](https://github.com/iselegant/cchb/releases)
[![CI](https://github.com/iselegant/cchb/actions/workflows/ci.yml/badge.svg)](https://github.com/iselegant/cchb/actions/workflows/ci.yml)

> [!NOTE]
> This is **not** an official Anthropic product. It is a community-built tool that reads Claude Code's local session data.
> Claude Code updates may change the internal data format or file layout, which could affect this tool's behavior. If you encounter issues after a Claude Code update, please check for a new release or open an issue.

## Why cchb?

Claude Code keeps every session as a JSONL file under `~/.claude/projects/`. They pile up fast — across branches, across repos — and the only built-in way to revisit one is to remember the right `claude --resume <id>`.

cchb gives those sessions a home:

- **Find** the session you want by content, project, branch, or date.
- **Read** it inline with proper Markdown rendering — no `cat`-ing JSONL.
- **Resume** it in one keystroke, straight back into Claude Code.

It is a single static binary, opens instantly, and stays out of your way.

## Highlights

### Browse
- **Cross-project session list** — every session under `~/.claude/projects/`, sorted by last modified.
- **Live preview** — moving the cursor loads the conversation in the right panel; no extra keystroke.
- **Project / branch / first-prompt** at a glance in the list.

### Search & filter
- **Fuzzy search** over conversation content with in-view match highlighting and `n` / `N` to jump between hits — across sessions, not just within one.
- **Date range filter** with arrow-key date stepping (no manual typing required).

### Read
- **Markdown rendering** for headings, code blocks, tables, lists, links, and inline emphasis.
- **Mouse text selection** with auto-copy to clipboard on release; `y` works too.

### Resume
- `Enter` exits the TUI and runs `claude --resume <session-id>` via `exec` — the session reopens in the same terminal as if you had typed it.

### Performance
- Reads `sessions-index.json` (Claude Code's own metadata index) when available, falls back to a parallel JSONL scan, and lazy-loads conversations behind an LRU cache. See [ADR-0002](docs/adr/0002-merge-index-and-jsonl-scan-for-session-discovery.md).

## Installation

### Quick install

```sh
curl -fsSL https://raw.githubusercontent.com/iselegant/cchb/main/install.sh | sh
```

This auto-detects your OS and architecture, downloads the latest binary, and places it in `~/.local/bin/`.

### Homebrew (macOS)

```sh
brew install iselegant/tap/cchb
```

Or tap once and reuse the short name:

```sh
brew tap iselegant/tap
brew install cchb
```

### mise

```sh
mise use -g github:iselegant/cchb
```

This uses [mise](https://mise.jdx.dev/)'s `github` backend to install the latest release binary.

### Cargo — coming soon

```sh
cargo install cchb
```

### Manual download

Download a pre-built binary from [GitHub Releases](https://github.com/iselegant/cchb/releases) and place it in your `$PATH`.

| Platform | Binary |
|----------|--------|
| macOS (Apple Silicon) | `cchb-aarch64-apple-darwin.tar.gz` |
| macOS (Intel) | `cchb-x86_64-apple-darwin.tar.gz` |
| Linux (x86_64) | `cchb-x86_64-unknown-linux-gnu.tar.gz` |

### Build from source

```sh
cargo build --release
cp target/release/cchb /usr/local/bin/
```

Requires Rust 2024 edition (1.85+).

## Usage

```sh
cchb
```

### Quick tour

1. `cchb` — TUI opens with the most recent session selected and previewed.
2. `f` — fuzzy search across all sessions; type, then `Enter` to keep the filter.
3. `n` / `N` — jump to the next / previous match, crossing session boundaries.
4. `Enter` — resume the highlighted session in Claude Code.
5. `?` — full keybinding reference.

### CLI Flags

| Flag | Action |
|------|--------|
| `--version`, `-v` | Print version and exit |

### Keybindings

#### Session list (Normal mode)

| Key | Action |
|-----|--------|
| `j` / `k` | Move down / up |
| `g` / `G` | Jump to top / bottom |
| `Right` / `Left` | Next / previous page |
| `Ctrl+d` / `Ctrl+u` | Half-page down / up |
| `Enter` | Resume selected session in Claude Code |
| `l` | Reload conversation |
| `f` | Fuzzy search |
| `d` | Date range filter |
| `c` | Clear all filters |
| `R` | Reload session list |
| `Tab` | Switch panel focus |
| `h` / `?` | Help |
| `Esc` / `q` | Quit |

#### Conversation viewer (Viewing mode)

| Key | Action |
|-----|--------|
| `j` / `k` | Scroll down / up |
| `g` / `G` | Scroll to top / bottom |
| `Ctrl+d` / `Ctrl+u` | Half-page down / up |
| `[` / `]` | Previous / next session |
| `n` / `N` | Next / previous search match |
| `c` | Clear all filters |
| `f` | Search |
| `d` | Date range filter |
| `Enter` | Resume selected session |
| `Tab` | Switch panel focus |
| `h` / `?` | Help |
| `Esc` / `q` | Back to list |

## UI Layout

```
┌─ cchb - Claude Code History Browser ─────────────┐
├──────────────────┬───────────────────────────────┤
│ Sessions (35%)   │ Conversation (65%)            │
│                  │                               │
│ > project-a      │ │ You:                        │
│   (main)         │ │   Run terraform plan        │
│   2026-04-08     │ └─                            │
│   "terraform..." │                               │
│                  │ │ Claude:                     │
│   project-b      │ │   Here are the results...   │
│   (feature/x)    │ └─                            │
│   2026-04-07     │                               │
│   "API design.." │                               │
├──────────────────┴───────────────────────────────┤
│ 42 sessions        f:search d:date h:help q:quit │
└──────────────────────────────────────────────────┘
```

## Architecture

cchb is built on [ratatui](https://ratatui.rs/) + [crossterm](https://github.com/crossterm-rs/crossterm), with [nucleo](https://github.com/helix-editor/nucleo) for fuzzy matching and [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark) for Markdown rendering.

For the data format, module layout, and design rationale see:

- [`docs/SPECIFICATION.md`](docs/SPECIFICATION.md) — full functional and non-functional spec.
- [`docs/adr/`](docs/adr/) — architecture decision records.

## Acknowledgments

cchb stands on the shoulders of [**ccresume**](https://github.com/sasazame/ccresume) by [@sasazame](https://github.com/sasazame).

ccresume was the first tool I am aware of to recognize that Claude Code's local session files deserved a real browser, and it shaped how I think this category of tool should feel — fast, keyboard-first, and respectful of the terminal. cchb is a Rust-based reimagining in the same spirit, with a few additional features (Markdown rendering, cross-session search navigation, mouse selection, etc.). If you are on the Node.js side of the fence, **please go check out ccresume** — it is excellent, and a lot of cchb's UX exists because ccresume showed the way.

## Contributing

Bug reports and PRs are welcome. Before opening a PR, please:

- Read [`CLAUDE.md`](CLAUDE.md) — this project follows TDD strictly.
- Run `cargo test`, `cargo clippy`, and `cargo fmt --check`.
- For non-trivial changes, add or update an [ADR](docs/adr/).

## License

[Apache-2.0](LICENSE) © iselegant
