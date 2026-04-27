# cchb - Claude Code History Browser

A fast TUI tool for browsing and resuming past [Claude Code](https://docs.anthropic.com/en/docs/claude-code) sessions.

Inspired by [ccresume](https://github.com/sasazame/ccresume).

> [!NOTE]
> This is **not** an official Anthropic product. It is a community-built tool that reads Claude Code's local session data.
> Claude Code updates may change the internal data format or file layout, which could affect this tool's behavior. If you encounter issues after a Claude Code update, please check for a new release or open an issue.

## Features

- **Session list** — Browse all Claude Code sessions across projects, sorted by last modified date
- **Conversation viewer** — Preview conversations with Markdown rendering (headings, code blocks, tables, lists, links)
- **Fuzzy search** — Search through conversation content with real-time filtering and in-conversation match highlighting
- **Date range filter** — Filter sessions by date with arrow-key date stepping
- **Session resume** — Resume any session directly with `claude --resume`
- **Vim keybindings** — Navigate with `j`/`k`, `g`/`G`, `Ctrl+d`/`Ctrl+u`, and more

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

## License

Apache-2.0
