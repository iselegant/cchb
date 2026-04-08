# Markdown Rendering in Conversation View

## Context

Currently the conversation view renders all message text as plain white text. Claude's responses contain rich Markdown (headings, code blocks, tables, lists, bold/italic) but none of it is visually distinguished. This feature adds Markdown parsing and styled rendering to make conversations much more readable.

## Approach: `pulldown-cmark` + new `markdown.rs` module

Use `pulldown-cmark` (de facto standard Rust Markdown parser, CommonMark-compliant, fast) to parse text into events, then map those events to styled ratatui `Span`/`Line` sequences.

### Supported Markdown Elements

| Element | Terminal Rendering |
|---|---|
| `# Heading` | Bold + color (Cyan for H1, Blue for H2, etc.) |
| `**bold**` | Bold modifier |
| `*italic*` | Italic modifier |
| `` `code` `` | Background color (inline) |
| Code blocks | Background color region + language label |
| Tables | Box-drawing characters (`│`, `─`, `┌`, etc.) |
| Lists (`-`, `1.`) | Bullet/number + indentation |
| `[text](url)` | Underline + cyan |
| `---` | Horizontal rule line |

### File Changes

| File | Change |
|---|---|
| `Cargo.toml` | Add `pulldown-cmark = "0.12"` |
| `src/main.rs` | Add `mod markdown;` |
| `src/color.rs` | Add `MarkdownStyles` struct + `pub markdown` field to `Theme` |
| `src/markdown.rs` | **New.** `render_markdown(&str, Style, &Theme) -> Vec<Line<'static>>` |
| `src/ui.rs` | Replace plain-text loop in `render_conversation_view` with `markdown::render_markdown()` calls |
| `docs/SPECIFICATION.md` | Update FR-2 to document Markdown rendering support |

### Implementation Steps (TDD)

1. Add `pulldown-cmark` dependency to `Cargo.toml`
2. Extend `Theme` with `MarkdownStyles` in `color.rs`
3. Create `src/markdown.rs` with test stubs first (17+ tests)
4. Implement `MarkdownRenderer` incrementally:
   - Plain text passthrough -> inline formatting -> headings -> code blocks -> lists -> links -> rules -> tables
5. Integrate into `ui.rs::render_conversation_view()`
6. Update `docs/SPECIFICATION.md`

### Key Design Decisions

- **New module**: `markdown.rs` is a pure function (`&str -> Vec<Line>`) -- independently testable, keeps `ui.rs` clean
- **Style stacking**: Nested formatting (bold inside italic) handled via a `style_stack: Vec<Style>`
- **Parse at render time initially**: Fast enough for typical conversations. Can optimize to load-time caching later if needed
- **Tables**: Two-pass (accumulate rows, compute column widths, then render with box-drawing characters)

## Verification

1. `cargo test` -- all new markdown tests pass
2. `cargo clippy` -- no warnings
3. `cargo fmt --check` -- passes
4. `cargo run` -- visually verify markdown rendering in conversation view with real session data
