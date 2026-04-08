# ADR-0001: Page Navigation with Arrow Keys

## Status

Accepted

## Context

The session list currently supports single-item navigation (j/k), jump-to-ends (g/G), and half-page scrolling (Ctrl+d/Ctrl+u). For users with large numbers of sessions, browsing the list requires many key presses. A full-page navigation mechanism is needed for faster browsing.

Additionally, the existing `visible_height` parameter used by half-page scrolling is hardcoded to 20, which does not reflect the actual terminal size.

## Decision

- Use **Right arrow** for next-page and **Left arrow** for previous-page navigation in Normal mode.
- Page size is dynamically calculated from the actual terminal panel height: `items_per_page = panel_inner_height / 4` (each session item occupies 4 lines).
- Store `items_per_page` in `AppState` so that both page navigation and half-page scrolling can use the actual value.
- Replace the hardcoded `visible_height = 20` with the dynamically calculated value.

## Consequences

- **Positive**: Users can quickly page through large session lists. Ctrl+d/Ctrl+u now also adapt to actual terminal height.
- **Negative**: Left/Right arrow keys are now reserved in Normal mode and cannot be used for other features in the future without reassignment.
- **Neutral**: The `items_per_page` value updates each render cycle, so it adapts to terminal resizing.
