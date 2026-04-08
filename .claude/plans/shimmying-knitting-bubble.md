# Plan: Add colored vertical border lines to conversation view

## Context
The conversation view shows "You:" in green and "Claude:" in magenta, but it's hard to visually distinguish where one message ends and another begins. Adding colored vertical lines on the left side of each message block will make boundaries clear.

## Visual Design
```
│ You:
│   message content here
│   more content
└─

│ Claude:
│   response here
│   more response
└─
```
- User messages: green `│` and `└─` on the left
- Assistant messages: magenta `│` and `└─` on the left
- The border color matches the label color for each role

## Changes

### 1. `src/color.rs` — Add border styles to Theme
- Add `user_border: Style` (fg: Green) and `assistant_border: Style` (fg: Magenta)
- Initialize them in `default_theme()`

### 2. `src/ui.rs` — Modify `render_conversation_view()`
- Adjust `md_width`: subtract 2 more chars for the border prefix (`│ `)
- For each message, track the role's border style
- Prepend `│ ` (colored) to label line and all content lines
- After content lines, add a `└─` line in the same color
- Keep the blank separator line (no border)

### 3. `docs/SPECIFICATION.md` — Update NFR-1 color scheme
- Document the new border indicator behavior

## Verification
1. `cargo clippy` — no warnings
2. `cargo fmt --check` — formatting clean
3. `cargo test` — all tests pass
4. `cargo run` — visually confirm green/magenta borders in conversation view
