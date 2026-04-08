# Plan: Add Left/Right Arrow Key Page Navigation for Session List

## Context

Currently, the session list supports j/k (single item), g/G (top/bottom), and Ctrl+d/Ctrl+u (half-page) navigation. The user wants **Right arrow → next page** and **Left arrow → previous page** to quickly browse through sessions in full-page increments. Additionally, the existing `visible_height` is hardcoded to 20, which should be fixed as part of this work.

## ADR

Create `docs/adr/0001-page-navigation-with-arrow-keys.md`:

- **Status**: Accepted
- **Context**: Session list navigation only supports single-item (j/k) and half-page (Ctrl+d/u) movement. For large session lists, users need a faster way to browse. Left/Right arrow keys are intuitive for page-level navigation and don't conflict with existing keybindings.
- **Decision**: Use Right/Left arrow keys for full-page forward/backward navigation. Page size is dynamically calculated from the actual terminal height (panel height / 4 lines per item).
- **Consequences**: visible_height is no longer hardcoded, improving Ctrl+d/u accuracy as well. Left/Right arrows are now reserved in Normal mode.

## Approach

### 1. Calculate actual visible item count in `ui.rs` and store in `AppState`

Each session list item takes **4 lines** (project+branch, date, preview, blank). The session panel height is available during `render()`. Calculate and store `items_per_page` in `AppState` so event handlers can use it.

- **`src/app.rs`**: Add `pub items_per_page: usize` field to `AppState` (default 5)
- **`src/ui.rs`**: In `render()`, after layout calculation, compute `items_per_page = panel_inner_height / 4` and write it to `app.items_per_page`

### 2. Add page navigation methods to `AppState`

- **`src/app.rs`**: Add `page_down()` and `page_up()` methods
  - `page_down()`: advance `selected_index` by `self.items_per_page`, clamped to last item
  - `page_up()`: go back by `self.items_per_page`, clamped to 0

### 3. Wire Right/Left arrow keys in Normal mode

- **`src/event.rs`**: In Normal mode key handling, add:
  - `KeyCode::Right` → `app.page_down()`
  - `KeyCode::Left` → `app.page_up()`

### 4. Fix hardcoded `visible_height` for Ctrl+d/Ctrl+u

- **`src/event.rs`**: Replace hardcoded `20` with `app.items_per_page * 2` (since half_page methods divide by 2)

### 5. Update specification and help overlay

- **`docs/SPECIFICATION.md`**: Add Right/Left to FR-3 keybindings table
- **`src/ui.rs`**: Add Right/Left to the help overlay text

### 6. Write tests (TDD)

- **`src/app.rs` tests**: `test_page_down`, `test_page_up`, `test_page_down_clamp`, `test_page_up_clamp`
- **`src/event.rs` tests**: `test_right_arrow_page_down`, `test_left_arrow_page_up`

## Files to Modify

| File | Changes |
|------|---------|
| `docs/adr/0001-page-navigation-with-arrow-keys.md` | **New** — ADR for this design decision |
| `docs/SPECIFICATION.md` | Document new keybindings |
| `src/app.rs` | Add `items_per_page` field, `page_down()`, `page_up()` methods, tests |
| `src/event.rs` | Add Right/Left handlers, fix hardcoded visible_height, tests |
| `src/ui.rs` | Calculate `items_per_page` from panel height, update help overlay |

## Verification

1. `cargo test` — all tests pass (including new pagination tests)
2. `cargo clippy` — no warnings
3. `cargo fmt --check` — no diff
4. Manual: run `cargo run --release`, verify Right/Left arrows page through session list correctly
