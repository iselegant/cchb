# ADR-0003: Enter Key Resumes the Selected Session

## Status

Accepted

## Context

Previously, the `r` key was bound to "Resume the currently selected session" (FR-9), and `Enter` was bound to either entering Viewing mode (in the Session List panel) or toggling panel focus (in the Conversation panel). Two observations motivated revisiting this:

1. **Resume is the primary action**, but it was hidden behind a non-obvious single-letter key (`r`). New users were not discovering the resume feature.
2. **`Enter` carried two unrelated duties** (open Viewing mode, toggle panel) that overlap with `Tab` (panel toggle) and the auto-loaded conversation preview (no explicit "open" needed for browsing).

We want a more intuitive default: pressing `Enter` on a list item should perform the most consequential action for that item — restoring the session.

## Decision

- **`Enter` (Normal and Viewing modes) restores the currently selected session.** It calls `request_resume()`, exits the TUI, and the main loop launches `claude --resume <session-id>`. This applies regardless of which panel is focused, because the action targets the selected session in the list.
- **The `r` key is removed.** Lowercase `r` is no longer bound. Capital `R` continues to reload the session list (unchanged).
- **Panel toggling is consolidated to `Tab`.** `Enter` no longer toggles panel focus.
- **Viewing mode is no longer entered explicitly via `Enter`.** Users can read conversations via the auto-loaded preview, switch focus to the conversation panel via `Tab` (which gives them dedicated scroll keys in Normal mode), or fall into Viewing mode automatically via `n`/`N` cross-session search navigation.

## Consequences

- **Positive**: Resume is now discoverable and consistent with common TUI conventions (Enter = "do the primary action on the selection").
- **Positive**: Each key has one clear job — `Enter` resumes, `Tab` toggles panels, `l` reloads the conversation, `R` reloads the session list.
- **Negative**: There is no longer an explicit key to "enter Viewing mode" from Normal mode without a search query. Users who relied on Viewing mode for dedicated scrolling must use `Tab` to focus the conversation panel instead. In practice, scroll behavior in Normal mode + Conversation panel is equivalent to Viewing mode + Conversation panel.
- **Neutral**: Existing muscle memory for `r` will be broken. README and in-app help must be updated accordingly.
