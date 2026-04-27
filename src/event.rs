use crate::app::{AppMode, AppState, ContentPosition, DateField, Panel, TextSelection};
use crate::filter;
use crate::session;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

/// Handle a key event based on the current app mode.
pub fn handle_key(app: &mut AppState, key: KeyEvent) -> Result<()> {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.should_quit = true;
        return Ok(());
    }
    // Hidden easter egg: sparkle the logo
    if key.code == KeyCode::Char('*') {
        app.start_logo_sparkle();
        return Ok(());
    }
    match app.mode {
        AppMode::Normal => handle_normal_key(app, key)?,
        AppMode::Viewing => handle_viewing_key(app, key)?,
        AppMode::FuzzySearch => handle_search_key(app, key),
        AppMode::DateFilter => handle_date_filter_key(app, key),
        AppMode::Help => handle_help_key(app),
    }
    Ok(())
}

fn handle_normal_key(app: &mut AppState, key: KeyEvent) -> Result<()> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => {
            app.should_quit = true;
        }
        (KeyCode::Char('j'), _) | (KeyCode::Down, _) => {
            if app.active_panel == Panel::ConversationView {
                app.scroll_conversation_down();
            } else {
                app.select_next();
            }
        }
        (KeyCode::Char('k'), _) | (KeyCode::Up, _) => {
            if app.active_panel == Panel::ConversationView {
                app.scroll_conversation_up();
            } else {
                app.select_prev();
            }
        }
        (KeyCode::Char('g'), _) => {
            if app.active_panel == Panel::ConversationView {
                app.scroll_conversation_top();
            } else {
                app.go_top();
            }
        }
        (KeyCode::Char('G'), _) => {
            if app.active_panel == Panel::ConversationView {
                app.conversation_scroll = usize::MAX / 2;
            } else {
                app.go_bottom();
            }
        }
        (KeyCode::Right, _) => {
            if app.active_panel == Panel::ConversationView {
                app.conversation_scroll += app.items_per_page;
            } else {
                app.page_down();
            }
        }
        (KeyCode::Left, _) => {
            if app.active_panel == Panel::ConversationView {
                app.conversation_scroll =
                    app.conversation_scroll.saturating_sub(app.items_per_page);
            } else {
                app.page_up();
            }
        }
        (KeyCode::Char('d'), m) if m.contains(KeyModifiers::CONTROL) => {
            if app.active_panel == Panel::ConversationView {
                let half = app.items_per_page;
                app.conversation_scroll += half;
            } else {
                app.half_page_down(app.items_per_page * 2);
            }
        }
        (KeyCode::Char('u'), m) if m.contains(KeyModifiers::CONTROL) => {
            if app.active_panel == Panel::ConversationView {
                let half = app.items_per_page;
                app.conversation_scroll = app.conversation_scroll.saturating_sub(half);
            } else {
                app.half_page_up(app.items_per_page * 2);
            }
        }
        (KeyCode::Enter, _) => {
            app.request_resume();
        }
        (KeyCode::Char('f') | KeyCode::Char('/'), _) => {
            app.enter_search();
        }
        (KeyCode::Char('d'), _) => {
            app.enter_date_filter();
        }
        (KeyCode::Char('c'), _) => {
            app.clear_filters();
        }
        (KeyCode::Char('n'), _) => {
            if app.active_panel == Panel::ConversationView {
                app.jump_to_next_match();
            } else if !app.search_query.is_empty() {
                enter_viewing_with_search_jump(
                    app,
                    crate::app::SearchJumpDirection::First,
                    Panel::SessionList,
                );
            }
        }
        (KeyCode::Char('N'), _) => {
            if app.active_panel == Panel::ConversationView {
                app.jump_to_prev_match();
            } else if !app.search_query.is_empty() {
                enter_viewing_with_search_jump(
                    app,
                    crate::app::SearchJumpDirection::Last,
                    Panel::SessionList,
                );
            }
        }
        (KeyCode::Char('h'), _) | (KeyCode::Char('?'), _) => {
            app.toggle_help();
        }
        (KeyCode::Char('R'), _) => {
            // Reload is handled by main loop since it needs claude_dir path
        }
        (KeyCode::Char('l'), _) => {
            app.request_reload_conversation();
        }
        (KeyCode::Tab, _) => {
            app.toggle_panel();
        }
        (KeyCode::Char('y'), _) => {
            copy_selection_to_clipboard(app);
        }
        _ => {}
    }
    Ok(())
}

fn handle_viewing_key(app: &mut AppState, key: KeyEvent) -> Result<()> {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) | (KeyCode::Char('q'), _) => {
            app.exit_viewing();
        }
        (KeyCode::Char('j'), _) | (KeyCode::Down, _) => {
            if app.active_panel == Panel::SessionList {
                let prev_idx = app.selected_index;
                app.select_next();
                if app.selected_index != prev_idx {
                    reload_conversation(app);
                }
            } else {
                app.scroll_conversation_down();
            }
        }
        (KeyCode::Char('k'), _) | (KeyCode::Up, _) => {
            if app.active_panel == Panel::SessionList {
                let prev_idx = app.selected_index;
                app.select_prev();
                if app.selected_index != prev_idx {
                    reload_conversation(app);
                }
            } else {
                app.scroll_conversation_up();
            }
        }
        (KeyCode::Char('g'), _) => {
            if app.active_panel == Panel::SessionList {
                let prev_idx = app.selected_index;
                app.go_top();
                if app.selected_index != prev_idx {
                    reload_conversation(app);
                }
            } else {
                app.scroll_conversation_top();
            }
        }
        (KeyCode::Char('G'), _) => {
            if app.active_panel == Panel::SessionList {
                let prev_idx = app.selected_index;
                app.go_bottom();
                if app.selected_index != prev_idx {
                    reload_conversation(app);
                }
            } else {
                // scroll to bottom - set to large value, clamped by render
                app.conversation_scroll = usize::MAX / 2;
            }
        }
        (KeyCode::Char('d'), m) if m.contains(KeyModifiers::CONTROL) => {
            if app.active_panel == Panel::SessionList {
                let prev_idx = app.selected_index;
                let max = app.filtered_indices.len().saturating_sub(1);
                let half = app.items_per_page;
                app.selected_index = (app.selected_index + half).min(max);
                app.sync_list_state();
                if app.selected_index != prev_idx {
                    reload_conversation(app);
                }
            } else {
                app.half_page_down(app.items_per_page * 2);
            }
        }
        (KeyCode::Char('u'), m) if m.contains(KeyModifiers::CONTROL) => {
            if app.active_panel == Panel::SessionList {
                let prev_idx = app.selected_index;
                let half = app.items_per_page;
                app.selected_index = app.selected_index.saturating_sub(half);
                app.sync_list_state();
                if app.selected_index != prev_idx {
                    reload_conversation(app);
                }
            } else {
                app.half_page_up(app.items_per_page * 2);
            }
        }
        (KeyCode::Char(']'), _) => {
            let prev_idx = app.selected_index;
            app.next_session_in_viewing();
            if app.selected_index != prev_idx {
                reload_conversation(app);
            }
        }
        (KeyCode::Char('['), _) => {
            let prev_idx = app.selected_index;
            app.prev_session_in_viewing();
            if app.selected_index != prev_idx {
                reload_conversation(app);
            }
        }
        (KeyCode::Char('l'), _) => {
            app.request_reload_conversation();
        }
        (KeyCode::Char('f') | KeyCode::Char('/'), _) => {
            app.exit_viewing();
            app.enter_search();
        }
        (KeyCode::Char('d'), _) => {
            app.exit_viewing();
            app.enter_date_filter();
        }
        (KeyCode::Right, _) => {
            if app.active_panel == Panel::ConversationView {
                app.conversation_scroll += app.items_per_page;
            } else {
                app.page_down();
            }
        }
        (KeyCode::Left, _) => {
            if app.active_panel == Panel::ConversationView {
                app.conversation_scroll =
                    app.conversation_scroll.saturating_sub(app.items_per_page);
            } else {
                app.page_up();
            }
        }
        (KeyCode::Tab, _) => {
            app.toggle_panel();
        }
        (KeyCode::Enter, _) => {
            app.request_resume();
        }
        (KeyCode::Char('n'), _) => {
            if app.active_panel == Panel::ConversationView {
                app.jump_to_next_match();
            } else if app.jump_to_next_match_cross_session() {
                reload_conversation(app);
            }
        }
        (KeyCode::Char('N'), _) => {
            if app.active_panel == Panel::ConversationView {
                app.jump_to_prev_match();
            } else if app.jump_to_prev_match_cross_session() {
                reload_conversation(app);
            }
        }
        (KeyCode::Char('c'), _) => {
            app.clear_filters();
            app.search_match_positions.clear();
            app.search_match_current = None;
        }
        (KeyCode::Char('h'), _) | (KeyCode::Char('?'), _) => {
            app.toggle_help();
        }
        (KeyCode::Char('y'), _) => {
            copy_selection_to_clipboard(app);
        }
        _ => {}
    }
    Ok(())
}

fn handle_search_key(app: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.cancel_search();
            // Restore full list
            app.filtered_indices = (0..app.sessions.len()).collect();
        }
        KeyCode::Enter if !app.search_cache_loading => {
            // Apply search and return to normal (keep cache for reuse)
            let query = app.search_query.clone();
            let cache = &app.search_content_cache;
            let indices = filter::fuzzy_filter(&app.sessions, &query, cache);
            app.update_filtered_indices(indices);
            app.search_cache_receiver = None;
            app.mode = AppMode::Normal;
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            // Live filter (uses metadata fallback if cache not ready)
            let cache = &app.search_content_cache;
            let indices = filter::fuzzy_filter(&app.sessions, &app.search_query, cache);
            app.update_filtered_indices(indices);
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            // Live filter (uses metadata fallback if cache not ready)
            let cache = &app.search_content_cache;
            let indices = filter::fuzzy_filter(&app.sessions, &app.search_query, cache);
            app.update_filtered_indices(indices);
        }
        _ => {}
    }
}

fn handle_date_filter_key(app: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.cancel_date_filter();
        }
        KeyCode::Tab => {
            app.toggle_date_field();
        }
        KeyCode::Enter => {
            let from = filter::parse_date_input(&app.date_from_input);
            let to = filter::parse_date_input(&app.date_to_input);
            let indices = filter::apply_filters(
                &app.sessions,
                &app.search_query,
                from,
                to,
                &app.search_content_cache,
            );
            app.update_filtered_indices(indices);
            app.mode = AppMode::Normal;
        }
        KeyCode::Up => {
            app.increment_date_field();
        }
        KeyCode::Down => {
            app.decrement_date_field();
        }
        KeyCode::Backspace => {
            let input = match app.date_field {
                DateField::From => &mut app.date_from_input,
                DateField::To => &mut app.date_to_input,
            };
            input.pop();
        }
        KeyCode::Char(c) => {
            let input = match app.date_field {
                DateField::From => &mut app.date_from_input,
                DateField::To => &mut app.date_to_input,
            };
            input.push(c);
        }
        _ => {}
    }
}

fn handle_help_key(app: &mut AppState) {
    app.close_help();
}

/// Enter Viewing mode, load the conversation, and set a pending search jump.
/// The `panel` parameter controls which panel stays active after entering
/// Viewing mode (e.g. `Panel::SessionList` to keep focus on the session list).
fn enter_viewing_with_search_jump(
    app: &mut AppState,
    direction: crate::app::SearchJumpDirection,
    panel: Panel,
) {
    if app.selected_session().is_some() {
        let path = app.selected_session().unwrap().file_path.clone();
        app.enter_viewing();
        app.active_panel = panel;
        if let Ok(messages) = session::load_conversation(&path) {
            app.conversation = session::display_messages(messages);
        }
        app.pending_search_jump = Some(direction);
    }
}

fn reload_conversation(app: &mut AppState) {
    if let Some(session) = app.selected_session() {
        let path = session.file_path.clone();
        if let Ok(messages) = session::load_conversation(&path) {
            app.conversation = session::display_messages(messages);
        }
    }
}

/// Handle a mouse event. Ignored during overlay modes (FuzzySearch, DateFilter, Help).
pub fn handle_mouse(app: &mut AppState, mouse: MouseEvent) {
    // Ignore mouse events during overlay modes.
    match app.mode {
        AppMode::FuzzySearch | AppMode::DateFilter | AppMode::Help => return,
        _ => {}
    }

    let col = mouse.column;
    let row = mouse.row;

    match mouse.kind {
        MouseEventKind::ScrollUp => handle_scroll(app, col, row, ScrollDirection::Up),
        MouseEventKind::ScrollDown => handle_scroll(app, col, row, ScrollDirection::Down),
        MouseEventKind::Down(MouseButton::Left) => handle_mouse_down(app, col, row),
        MouseEventKind::Drag(MouseButton::Left) => handle_mouse_drag(app, col, row),
        MouseEventKind::Up(MouseButton::Left) => handle_mouse_up(app),
        _ => {}
    }
}

enum ScrollDirection {
    Up,
    Down,
}

fn handle_scroll(app: &mut AppState, col: u16, row: u16, direction: ScrollDirection) {
    if is_in_rect(col, row, app.panel_geometry.conversation_body) {
        match direction {
            ScrollDirection::Up => app.scroll_conversation_up(),
            ScrollDirection::Down => app.scroll_conversation_down(),
        }
    } else if is_in_rect(col, row, app.panel_geometry.session_list) {
        match direction {
            ScrollDirection::Up => app.select_prev(),
            ScrollDirection::Down => app.select_next(),
        }
    }
}

fn handle_mouse_down(app: &mut AppState, col: u16, row: u16) {
    if let Some(pos) = mouse_to_content_position(app, col, row) {
        // Start a new text selection in the conversation panel.
        app.text_selection = Some(TextSelection {
            anchor: pos,
            cursor: pos,
            active: true,
        });
    } else if is_in_rect(col, row, app.panel_geometry.session_list) {
        // Click on session list: select the clicked session.
        app.clear_selection();
        if let Some(rect) = app.panel_geometry.session_list {
            let inner_y = rect.y + 1; // account for top border
            if row >= inner_y {
                let relative_row = (row - inner_y) as usize;
                // Each session item is 4 lines tall.
                let clicked_offset = relative_row / 4;
                // Compute absolute index: current page start + clicked offset.
                let page_start =
                    (app.selected_index / app.items_per_page.max(1)) * app.items_per_page.max(1);
                let target = page_start + clicked_offset;
                if target < app.filtered_indices.len() {
                    app.selected_index = target;
                    app.sync_list_state();
                }
            }
        }
    } else {
        app.clear_selection();
    }
}

fn handle_mouse_drag(app: &mut AppState, col: u16, row: u16) {
    let is_active = app.text_selection.as_ref().is_some_and(|sel| sel.active);
    if !is_active {
        return;
    }
    // Clamp mouse coordinates to conversation body bounds and compute position.
    if let Some(rect) = app.panel_geometry.conversation_body {
        let clamped_col = col.clamp(rect.x, rect.x + rect.width.saturating_sub(1));
        let clamped_row = row.clamp(rect.y, rect.y + rect.height.saturating_sub(1));
        if let Some(pos) = compute_content_position(app, clamped_col, clamped_row, rect)
            && let Some(ref mut sel) = app.text_selection
        {
            sel.cursor = pos;
        }
    }
}

fn handle_mouse_up(app: &mut AppState) {
    // Mark selection as inactive first.
    let is_empty = if let Some(ref mut sel) = app.text_selection {
        sel.active = false;
        sel.is_empty()
    } else {
        return;
    };

    if !is_empty {
        // Auto-copy to clipboard.
        if let Some(text) = app.extract_selected_text()
            && let Ok(mut clipboard) = arboard::Clipboard::new()
        {
            let _ = clipboard.set_text(text);
            app.clipboard_flash_at = Some(std::time::Instant::now());
        }
    } else {
        app.clear_selection();
    }
}

/// Copy the current text selection to the system clipboard, then clear the selection.
fn copy_selection_to_clipboard(app: &mut AppState) {
    if app.text_selection.as_ref().is_some_and(|s| !s.is_empty()) {
        if let Some(text) = app.extract_selected_text()
            && let Ok(mut clipboard) = arboard::Clipboard::new()
        {
            let _ = clipboard.set_text(text);
            app.clipboard_flash_at = Some(std::time::Instant::now());
        }
        app.clear_selection();
    }
}

/// Check if (col, row) falls within an optional Rect.
fn is_in_rect(col: u16, row: u16, rect: Option<ratatui::layout::Rect>) -> bool {
    if let Some(r) = rect {
        col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
    } else {
        false
    }
}

/// Map terminal (col, row) to a ContentPosition within the conversation, or None.
fn mouse_to_content_position(app: &AppState, col: u16, row: u16) -> Option<ContentPosition> {
    let rect = app.panel_geometry.conversation_body?;
    if !is_in_rect(col, row, Some(rect)) {
        return None;
    }
    compute_content_position(app, col, row, rect)
}

/// Compute ContentPosition from terminal coordinates and a known content_area rect.
fn compute_content_position(
    app: &AppState,
    col: u16,
    row: u16,
    rect: ratatui::layout::Rect,
) -> Option<ContentPosition> {
    let visual_row = (row.saturating_sub(rect.y)) as usize;
    let line = app.conversation_scroll + visual_row;
    let line = line.min(app.conversation_lines_cache.len().saturating_sub(1));

    // Visual column relative to content area start.
    let visual_col = col.saturating_sub(rect.x) as usize;

    // Subtract border prefix width ("│ " = 2 + "  " = 2 = 4 display cols for content lines).
    // For label lines ("│ You:") the prefix is 2 cols, but we use 4 uniformly
    // since selection columns are relative to the stripped content in extract_selected_text.
    let content_col = visual_col.saturating_sub(4);

    Some(ContentPosition::new(line, content_col))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Panel;
    use crate::session::SessionIndex;
    use chrono::Utc;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use std::path::PathBuf;

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn make_key_ctrl(c: char) -> KeyEvent {
        KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn make_sessions(n: usize) -> Vec<SessionIndex> {
        (0..n)
            .map(|i| {
                SessionIndex {
                    session_id: format!("sess-{i}"),
                    project_path: format!("/test/project-{i}"),
                    project_display: format!("project-{i}"),
                    first_prompt: format!("Prompt {i}"),
                    summary: None,
                    created: Utc::now(),
                    modified: Utc::now(),
                    git_branch: Some("main".into()),
                    message_count: 10,
                    file_path: PathBuf::from(format!("/tmp/sess-{i}.jsonl")),
                    date_display: String::new(),
                    branch_display: String::new(),
                    prompt_preview: String::new(),
                }
                .with_display_fields()
            })
            .collect()
    }

    #[test]
    fn test_j_moves_down_in_normal() {
        let mut app = AppState::new(make_sessions(5));
        handle_key(&mut app, make_key(KeyCode::Char('j'))).unwrap();
        assert_eq!(app.selected_index, 1);
    }

    #[test]
    fn test_k_moves_up_in_normal() {
        let mut app = AppState::new(make_sessions(5));
        app.selected_index = 3;
        handle_key(&mut app, make_key(KeyCode::Char('k'))).unwrap();
        assert_eq!(app.selected_index, 2);
    }

    #[test]
    fn test_g_goes_to_top() {
        let mut app = AppState::new(make_sessions(5));
        app.selected_index = 4;
        handle_key(&mut app, make_key(KeyCode::Char('g'))).unwrap();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_shift_g_goes_to_bottom() {
        let mut app = AppState::new(make_sessions(5));
        handle_key(&mut app, make_key(KeyCode::Char('G'))).unwrap();
        assert_eq!(app.selected_index, 4);
    }

    #[test]
    fn test_ctrl_d_half_page_down() {
        let mut app = AppState::new(make_sessions(30));
        app.items_per_page = 8;
        handle_key(&mut app, make_key_ctrl('d')).unwrap();
        // half of (items_per_page * 2) = items_per_page = 8
        assert_eq!(app.selected_index, 8);
    }

    #[test]
    fn test_ctrl_u_half_page_up() {
        let mut app = AppState::new(make_sessions(30));
        app.items_per_page = 8;
        app.selected_index = 20;
        handle_key(&mut app, make_key_ctrl('u')).unwrap();
        assert_eq!(app.selected_index, 12);
    }

    #[test]
    fn test_right_arrow_page_down() {
        let mut app = AppState::new(make_sessions(30));
        app.items_per_page = 5;
        handle_key(&mut app, make_key(KeyCode::Right)).unwrap();
        assert_eq!(app.selected_index, 5);
        handle_key(&mut app, make_key(KeyCode::Right)).unwrap();
        assert_eq!(app.selected_index, 10);
    }

    #[test]
    fn test_left_arrow_page_up() {
        let mut app = AppState::new(make_sessions(30));
        app.items_per_page = 5;
        app.selected_index = 15;
        handle_key(&mut app, make_key(KeyCode::Left)).unwrap();
        assert_eq!(app.selected_index, 10);
        handle_key(&mut app, make_key(KeyCode::Left)).unwrap();
        assert_eq!(app.selected_index, 5);
    }

    #[test]
    fn test_q_quits_in_normal() {
        let mut app = AppState::new(make_sessions(3));
        handle_key(&mut app, make_key(KeyCode::Char('q'))).unwrap();
        assert!(app.should_quit);
    }

    #[test]
    fn test_esc_quits_in_normal() {
        let mut app = AppState::new(make_sessions(3));
        handle_key(&mut app, make_key(KeyCode::Esc)).unwrap();
        assert!(app.should_quit);
    }

    #[test]
    fn test_f_enters_search() {
        let mut app = AppState::new(make_sessions(3));
        handle_key(&mut app, make_key(KeyCode::Char('f'))).unwrap();
        assert_eq!(app.mode, AppMode::FuzzySearch);
    }

    #[test]
    fn test_slash_enters_search() {
        let mut app = AppState::new(make_sessions(3));
        handle_key(&mut app, make_key(KeyCode::Char('/'))).unwrap();
        assert_eq!(app.mode, AppMode::FuzzySearch);
    }

    #[test]
    fn test_search_typing_updates_query() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::FuzzySearch;
        handle_key(&mut app, make_key(KeyCode::Char('t'))).unwrap();
        handle_key(&mut app, make_key(KeyCode::Char('e'))).unwrap();
        assert_eq!(app.search_query, "te");
    }

    #[test]
    fn test_search_backspace_removes_char() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::FuzzySearch;
        app.search_query = "test".into();
        handle_key(&mut app, make_key(KeyCode::Backspace)).unwrap();
        assert_eq!(app.search_query, "tes");
    }

    #[test]
    fn test_search_enter_applies_and_exits() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::FuzzySearch;
        app.search_query = "project-1".into();
        handle_key(&mut app, make_key(KeyCode::Enter)).unwrap();
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn test_search_typing_preserves_list_while_cache_loading() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::FuzzySearch;
        app.search_cache_loading = true;
        // Cache is empty (still loading)
        assert!(app.search_content_cache.is_empty());
        handle_key(&mut app, make_key(KeyCode::Char('t'))).unwrap();
        // All sessions should remain visible while cache is loading
        assert_eq!(app.filtered_indices.len(), 5);
        assert_eq!(app.search_query, "t");
    }

    #[test]
    fn test_search_backspace_preserves_list_while_cache_loading() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::FuzzySearch;
        app.search_cache_loading = true;
        app.search_query = "te".into();
        handle_key(&mut app, make_key(KeyCode::Backspace)).unwrap();
        assert_eq!(app.filtered_indices.len(), 5);
        assert_eq!(app.search_query, "t");
    }

    #[test]
    fn test_search_enter_blocked_while_cache_loading() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::FuzzySearch;
        app.search_cache_loading = true;
        app.search_query = "project-2".into();
        handle_key(&mut app, make_key(KeyCode::Enter)).unwrap();
        // Enter should be ignored while cache is loading
        assert_eq!(app.mode, AppMode::FuzzySearch);
        assert_eq!(app.search_query, "project-2");
        // Filtered indices should remain unchanged
        assert_eq!(app.filtered_indices, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_search_enter_works_after_cache_loaded() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::FuzzySearch;
        app.search_cache_loading = false;
        app.search_query = "project-2".into();
        handle_key(&mut app, make_key(KeyCode::Enter)).unwrap();
        // Enter should work when cache is not loading
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn test_search_esc_cancels() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::FuzzySearch;
        app.search_query = "test".into();
        handle_key(&mut app, make_key(KeyCode::Esc)).unwrap();
        assert_eq!(app.mode, AppMode::Normal);
        assert_eq!(app.search_query, "");
    }

    #[test]
    fn test_d_enters_date_filter() {
        let mut app = AppState::new(make_sessions(3));
        handle_key(&mut app, make_key(KeyCode::Char('d'))).unwrap();
        assert_eq!(app.mode, AppMode::DateFilter);
    }

    #[test]
    fn test_date_filter_typing() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::DateFilter;
        app.date_field = DateField::From;
        handle_key(&mut app, make_key(KeyCode::Char('2'))).unwrap();
        handle_key(&mut app, make_key(KeyCode::Char('0'))).unwrap();
        assert_eq!(app.date_from_input, "20");
    }

    #[test]
    fn test_date_filter_tab_switches_field() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::DateFilter;
        assert_eq!(app.date_field, DateField::From);
        handle_key(&mut app, make_key(KeyCode::Tab)).unwrap();
        assert_eq!(app.date_field, DateField::To);
    }

    #[test]
    fn test_date_filter_up_increments_date() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::DateFilter;
        app.date_field = DateField::From;
        app.date_from_input = "2026-04-05".to_string();
        handle_key(&mut app, make_key(KeyCode::Up)).unwrap();
        assert_eq!(app.date_from_input, "2026-04-06");
    }

    #[test]
    fn test_date_filter_down_decrements_date() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::DateFilter;
        app.date_field = DateField::To;
        app.date_to_input = "2026-04-08".to_string();
        handle_key(&mut app, make_key(KeyCode::Down)).unwrap();
        assert_eq!(app.date_to_input, "2026-04-07");
    }

    #[test]
    fn test_h_toggles_help() {
        let mut app = AppState::new(make_sessions(3));
        handle_key(&mut app, make_key(KeyCode::Char('h'))).unwrap();
        assert_eq!(app.mode, AppMode::Help);
    }

    #[test]
    fn test_help_any_key_closes() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Help;
        handle_key(&mut app, make_key(KeyCode::Char('x'))).unwrap();
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn test_tab_toggles_panel() {
        let mut app = AppState::new(make_sessions(3));
        handle_key(&mut app, make_key(KeyCode::Tab)).unwrap();
        assert_eq!(app.active_panel, crate::app::Panel::ConversationView);
    }

    #[test]
    fn test_enter_resumes_session_in_normal_mode_session_panel() {
        let mut app = AppState::new(make_sessions(3));
        app.active_panel = Panel::SessionList;
        app.selected_index = 1;
        handle_key(&mut app, make_key(KeyCode::Enter)).unwrap();
        assert!(app.should_quit);
        assert_eq!(app.resume_session_id.as_deref(), Some("sess-1"));
        // Should NOT enter Viewing mode — Enter is now resume
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn test_enter_resumes_session_in_normal_mode_conversation_panel() {
        let mut app = AppState::new(make_sessions(3));
        app.active_panel = Panel::ConversationView;
        app.selected_index = 0;
        handle_key(&mut app, make_key(KeyCode::Enter)).unwrap();
        // Even when focused on conversation panel, Enter resumes the selected session.
        assert!(app.should_quit);
        assert_eq!(app.resume_session_id.as_deref(), Some("sess-0"));
        // Active panel stays where it was; Tab is the panel toggle now.
        assert_eq!(app.active_panel, Panel::ConversationView);
    }

    #[test]
    fn test_l_reloads_conversation_in_normal_mode() {
        let mut app = AppState::new(make_sessions(3));
        assert_eq!(app.mode, AppMode::Normal);
        handle_key(&mut app, make_key(KeyCode::Char('l'))).unwrap();
        // l should reload conversation, not open session
        assert_eq!(app.mode, AppMode::Normal);
        assert!(app.conversation_reloading);
        assert_eq!(app.loaded_session_id, None);
    }

    #[test]
    fn test_l_reloads_conversation_in_viewing_mode() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Viewing;
        handle_key(&mut app, make_key(KeyCode::Char('l'))).unwrap();
        assert!(app.conversation_reloading);
        assert_eq!(app.loaded_session_id, None);
    }

    #[test]
    fn test_enter_resumes_session_in_viewing_mode() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Viewing;
        app.selected_index = 2;
        handle_key(&mut app, make_key(KeyCode::Enter)).unwrap();
        assert!(app.should_quit);
        assert_eq!(app.resume_session_id.as_deref(), Some("sess-2"));
    }

    #[test]
    fn test_c_clears_filters() {
        let mut app = AppState::new(make_sessions(5));
        app.search_query = "test".into();
        app.filtered_indices = vec![0, 2];
        handle_key(&mut app, make_key(KeyCode::Char('c'))).unwrap();
        assert_eq!(app.filtered_indices.len(), 5);
        assert_eq!(app.search_query, "");
    }

    #[test]
    fn test_normal_j_scrolls_conversation_when_panel_is_conversation() {
        let mut app = AppState::new(make_sessions(5));
        app.active_panel = Panel::ConversationView;
        app.selected_index = 0;
        handle_key(&mut app, make_key(KeyCode::Char('j'))).unwrap();
        assert_eq!(app.conversation_scroll, 1);
        assert_eq!(app.selected_index, 0); // session list should NOT move
    }

    #[test]
    fn test_normal_k_scrolls_conversation_when_panel_is_conversation() {
        let mut app = AppState::new(make_sessions(5));
        app.active_panel = Panel::ConversationView;
        app.conversation_scroll = 5;
        handle_key(&mut app, make_key(KeyCode::Char('k'))).unwrap();
        assert_eq!(app.conversation_scroll, 4);
    }

    #[test]
    fn test_normal_right_scrolls_conversation_when_panel_is_conversation() {
        let mut app = AppState::new(make_sessions(5));
        app.active_panel = Panel::ConversationView;
        app.items_per_page = 5;
        app.selected_index = 0;
        handle_key(&mut app, make_key(KeyCode::Right)).unwrap();
        assert!(app.conversation_scroll > 0);
        assert_eq!(app.selected_index, 0); // session list should NOT move
    }

    #[test]
    fn test_normal_left_scrolls_conversation_when_panel_is_conversation() {
        let mut app = AppState::new(make_sessions(5));
        app.active_panel = Panel::ConversationView;
        app.items_per_page = 5;
        app.conversation_scroll = 20;
        handle_key(&mut app, make_key(KeyCode::Left)).unwrap();
        assert!(app.conversation_scroll < 20);
    }

    #[test]
    fn test_normal_g_scrolls_conversation_top_when_panel_is_conversation() {
        let mut app = AppState::new(make_sessions(5));
        app.active_panel = Panel::ConversationView;
        app.conversation_scroll = 10;
        handle_key(&mut app, make_key(KeyCode::Char('g'))).unwrap();
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn test_normal_shift_g_scrolls_conversation_bottom_when_panel_is_conversation() {
        let mut app = AppState::new(make_sessions(5));
        app.active_panel = Panel::ConversationView;
        app.selected_index = 0;
        handle_key(&mut app, make_key(KeyCode::Char('G'))).unwrap();
        assert!(app.conversation_scroll > 0);
        assert_eq!(app.selected_index, 0); // session list should NOT move
    }

    #[test]
    fn test_viewing_j_scrolls_down() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::ConversationView;
        handle_key(&mut app, make_key(KeyCode::Char('j'))).unwrap();
        assert_eq!(app.conversation_scroll, 1);
    }

    #[test]
    fn test_viewing_j_selects_next_session_when_session_panel_active() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::SessionList;
        app.selected_index = 0;
        handle_key(&mut app, make_key(KeyCode::Char('j'))).unwrap();
        assert_eq!(app.selected_index, 1);
        assert_eq!(app.conversation_scroll, 0); // should NOT scroll conversation
    }

    #[test]
    fn test_viewing_k_selects_prev_session_when_session_panel_active() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::SessionList;
        app.selected_index = 3;
        handle_key(&mut app, make_key(KeyCode::Char('k'))).unwrap();
        assert_eq!(app.selected_index, 2);
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn test_viewing_down_selects_next_session_when_session_panel_active() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::SessionList;
        app.selected_index = 0;
        handle_key(&mut app, make_key(KeyCode::Down)).unwrap();
        assert_eq!(app.selected_index, 1);
    }

    #[test]
    fn test_viewing_g_goes_to_top_when_session_panel_active() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::SessionList;
        app.selected_index = 4;
        handle_key(&mut app, make_key(KeyCode::Char('g'))).unwrap();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_viewing_shift_g_goes_to_bottom_when_session_panel_active() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::SessionList;
        app.selected_index = 0;
        handle_key(&mut app, make_key(KeyCode::Char('G'))).unwrap();
        assert_eq!(app.selected_index, 4);
    }

    #[test]
    fn test_viewing_right_arrow_pages_down_conversation() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::ConversationView;
        app.items_per_page = 10;
        app.conversation_scroll = 0;
        handle_key(&mut app, make_key(KeyCode::Right)).unwrap();
        assert_eq!(app.conversation_scroll, 10);
        assert_eq!(app.mode, AppMode::Viewing);
    }

    #[test]
    fn test_viewing_left_arrow_pages_up_conversation() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::ConversationView;
        app.items_per_page = 10;
        app.conversation_scroll = 20;
        handle_key(&mut app, make_key(KeyCode::Left)).unwrap();
        assert_eq!(app.conversation_scroll, 10);
        assert_eq!(app.mode, AppMode::Viewing);
    }

    #[test]
    fn test_viewing_right_arrow_pages_down_session_list() {
        let mut app = AppState::new(make_sessions(30));
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::SessionList;
        app.items_per_page = 10;
        app.selected_index = 0;
        handle_key(&mut app, make_key(KeyCode::Right)).unwrap();
        assert_eq!(app.selected_index, 10);
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn test_viewing_left_arrow_pages_up_session_list() {
        let mut app = AppState::new(make_sessions(30));
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::SessionList;
        app.items_per_page = 10;
        app.selected_index = 20;
        handle_key(&mut app, make_key(KeyCode::Left)).unwrap();
        assert_eq!(app.selected_index, 10);
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn test_viewing_ctrl_d_half_page_down_when_session_panel_active() {
        let mut app = AppState::new(make_sessions(30));
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::SessionList;
        app.items_per_page = 8;
        app.selected_index = 0;
        handle_key(&mut app, make_key_ctrl('d')).unwrap();
        assert_eq!(app.selected_index, 8);
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn test_viewing_ctrl_u_half_page_up_when_session_panel_active() {
        let mut app = AppState::new(make_sessions(30));
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::SessionList;
        app.items_per_page = 8;
        app.selected_index = 20;
        handle_key(&mut app, make_key_ctrl('u')).unwrap();
        assert_eq!(app.selected_index, 12);
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn test_viewing_f_enters_search() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Viewing;
        handle_key(&mut app, make_key(KeyCode::Char('f'))).unwrap();
        assert_eq!(app.mode, AppMode::FuzzySearch);
        assert_eq!(app.active_panel, Panel::SessionList);
    }

    #[test]
    fn test_viewing_slash_enters_search() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Viewing;
        handle_key(&mut app, make_key(KeyCode::Char('/'))).unwrap();
        assert_eq!(app.mode, AppMode::FuzzySearch);
        assert_eq!(app.active_panel, Panel::SessionList);
    }

    #[test]
    fn test_viewing_d_enters_date_filter() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Viewing;
        handle_key(&mut app, make_key(KeyCode::Char('d'))).unwrap();
        assert_eq!(app.mode, AppMode::DateFilter);
        assert_eq!(app.active_panel, Panel::SessionList);
    }

    #[test]
    fn test_viewing_r_does_nothing() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Viewing;
        app.selected_index = 1;
        handle_key(&mut app, make_key(KeyCode::Char('r'))).unwrap();
        // `r` is no longer bound; Enter is the resume key.
        assert!(!app.should_quit);
        assert!(app.resume_session_id.is_none());
        assert_eq!(app.mode, AppMode::Viewing);
    }

    #[test]
    fn test_viewing_n_jumps_to_next_match() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::ConversationView;
        app.search_query = "test".into();
        app.search_match_positions = vec![(5, 0), (15, 0), (25, 0)];
        handle_key(&mut app, make_key(KeyCode::Char('n'))).unwrap();
        assert_eq!(app.search_match_current, Some(0));
        assert_eq!(app.conversation_scroll, 0); // 5 - 5 = 0 (saturating)
        handle_key(&mut app, make_key(KeyCode::Char('n'))).unwrap();
        assert_eq!(app.search_match_current, Some(1));
        assert_eq!(app.conversation_scroll, 10); // 15 - 5 = 10
    }

    #[test]
    fn test_viewing_shift_n_jumps_to_prev_match() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::ConversationView;
        app.search_query = "test".into();
        app.search_match_positions = vec![(5, 0), (15, 0), (25, 0)];
        handle_key(&mut app, make_key(KeyCode::Char('N'))).unwrap();
        assert_eq!(app.search_match_current, Some(2));
        assert_eq!(app.conversation_scroll, 20); // 25 - 5 = 20
        handle_key(&mut app, make_key(KeyCode::Char('N'))).unwrap();
        assert_eq!(app.search_match_current, Some(1));
        assert_eq!(app.conversation_scroll, 10); // 15 - 5 = 10
    }

    #[test]
    fn test_viewing_n_no_matches_does_nothing() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Viewing;
        handle_key(&mut app, make_key(KeyCode::Char('n'))).unwrap();
        assert_eq!(app.search_match_current, None);
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn test_viewing_c_clears_search() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.search_query = "test".into();
        app.filtered_indices = vec![0, 2];
        app.search_match_positions = vec![(5, 0), (15, 0)];
        app.search_match_current = Some(1);
        handle_key(&mut app, make_key(KeyCode::Char('c'))).unwrap();
        // Should clear search query and match state but stay in Viewing mode
        assert_eq!(app.search_query, "");
        assert!(app.search_match_positions.is_empty());
        assert_eq!(app.search_match_current, None);
        assert_eq!(app.filtered_indices.len(), 5);
        assert_eq!(app.mode, AppMode::Viewing);
    }

    #[test]
    fn test_viewing_esc_exits() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Viewing;
        handle_key(&mut app, make_key(KeyCode::Esc)).unwrap();
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn test_r_does_nothing_in_normal() {
        let mut app = AppState::new(make_sessions(3));
        handle_key(&mut app, make_key(KeyCode::Char('r'))).unwrap();
        // `r` is no longer bound; Enter is the resume key.
        assert!(!app.should_quit);
        assert!(app.resume_session_id.is_none());
    }

    #[test]
    fn test_shift_r_does_not_resume() {
        let mut app = AppState::new(make_sessions(3));
        handle_key(&mut app, make_key(KeyCode::Char('R'))).unwrap();
        assert!(!app.should_quit);
        assert!(app.resume_session_id.is_none());
    }

    #[test]
    fn test_ctrl_c_quits_in_normal() {
        let mut app = AppState::new(make_sessions(3));
        handle_key(&mut app, make_key_ctrl('c')).unwrap();
        assert!(app.should_quit);
    }

    #[test]
    fn test_ctrl_c_quits_in_viewing() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Viewing;
        handle_key(&mut app, make_key_ctrl('c')).unwrap();
        assert!(app.should_quit);
    }

    #[test]
    fn test_ctrl_c_quits_in_search() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::FuzzySearch;
        handle_key(&mut app, make_key_ctrl('c')).unwrap();
        assert!(app.should_quit);
    }

    #[test]
    fn test_ctrl_c_quits_in_date_filter() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::DateFilter;
        handle_key(&mut app, make_key_ctrl('c')).unwrap();
        assert!(app.should_quit);
    }

    #[test]
    fn test_ctrl_c_quits_in_help() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Help;
        handle_key(&mut app, make_key_ctrl('c')).unwrap();
        assert!(app.should_quit);
    }

    #[test]
    fn test_viewing_n_cross_session_when_session_panel() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::SessionList;
        app.search_query = "test".into();
        app.search_match_positions = vec![(5, 0), (15, 0)];
        app.search_match_current = Some(1); // at last match
        handle_key(&mut app, make_key(KeyCode::Char('n'))).unwrap();
        assert_eq!(app.selected_index, 1); // advanced to next session
        assert_eq!(
            app.pending_search_jump,
            Some(crate::app::SearchJumpDirection::First)
        );
    }

    #[test]
    fn test_viewing_n_stays_within_conversation_when_conversation_panel() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::ConversationView;
        app.search_query = "test".into();
        app.search_match_positions = vec![(5, 0), (15, 0)];
        // First n: navigate within session
        handle_key(&mut app, make_key(KeyCode::Char('n'))).unwrap();
        assert_eq!(app.search_match_current, Some(0));
        assert_eq!(app.selected_index, 0); // still same session
        handle_key(&mut app, make_key(KeyCode::Char('n'))).unwrap();
        assert_eq!(app.search_match_current, Some(1)); // now at last match
        // Next n at last match: should wrap within conversation, NOT cross session
        handle_key(&mut app, make_key(KeyCode::Char('n'))).unwrap();
        assert_eq!(app.search_match_current, Some(0)); // wrapped to first match
        assert_eq!(app.selected_index, 0); // still same session
        assert_eq!(app.pending_search_jump, None);
    }

    #[test]
    fn test_viewing_shift_n_stays_within_conversation_when_conversation_panel() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::ConversationView;
        app.search_query = "test".into();
        app.search_match_positions = vec![(5, 0), (15, 0)];
        // First N: navigate to last match within session
        handle_key(&mut app, make_key(KeyCode::Char('N'))).unwrap();
        assert_eq!(app.search_match_current, Some(1));
        handle_key(&mut app, make_key(KeyCode::Char('N'))).unwrap();
        assert_eq!(app.search_match_current, Some(0)); // now at first match
        // Next N at first match: should wrap within conversation, NOT cross session
        handle_key(&mut app, make_key(KeyCode::Char('N'))).unwrap();
        assert_eq!(app.search_match_current, Some(1)); // wrapped to last match
        assert_eq!(app.selected_index, 0); // still same session
        assert_eq!(app.pending_search_jump, None);
    }

    #[test]
    fn test_normal_n_jumps_to_next_match_in_conversation_panel() {
        let mut app = AppState::new(make_sessions(3));
        app.active_panel = Panel::ConversationView;
        app.search_match_positions = vec![(5, 0), (15, 0), (25, 0)];
        handle_key(&mut app, make_key(KeyCode::Char('n'))).unwrap();
        assert_eq!(app.search_match_current, Some(0));
        handle_key(&mut app, make_key(KeyCode::Char('n'))).unwrap();
        assert_eq!(app.search_match_current, Some(1));
    }

    #[test]
    fn test_normal_shift_n_jumps_to_prev_match_in_conversation_panel() {
        let mut app = AppState::new(make_sessions(3));
        app.active_panel = Panel::ConversationView;
        app.search_match_positions = vec![(5, 0), (15, 0), (25, 0)];
        handle_key(&mut app, make_key(KeyCode::Char('N'))).unwrap();
        assert_eq!(app.search_match_current, Some(2));
        handle_key(&mut app, make_key(KeyCode::Char('N'))).unwrap();
        assert_eq!(app.search_match_current, Some(1));
    }

    #[test]
    fn test_normal_n_enters_viewing_and_stays_on_session_panel() {
        let mut app = AppState::new(make_sessions(5));
        app.search_query = "test".into();
        assert_eq!(app.active_panel, Panel::SessionList);
        handle_key(&mut app, make_key(KeyCode::Char('n'))).unwrap();
        // Should enter Viewing mode but keep focus on SessionList
        assert_eq!(app.mode, AppMode::Viewing);
        assert_eq!(app.active_panel, Panel::SessionList);
        assert_eq!(
            app.pending_search_jump,
            Some(crate::app::SearchJumpDirection::First)
        );
    }

    #[test]
    fn test_normal_shift_n_enters_viewing_and_stays_on_session_panel() {
        let mut app = AppState::new(make_sessions(5));
        app.search_query = "test".into();
        handle_key(&mut app, make_key(KeyCode::Char('N'))).unwrap();
        // Should enter Viewing mode but keep focus on SessionList
        assert_eq!(app.mode, AppMode::Viewing);
        assert_eq!(app.active_panel, Panel::SessionList);
        assert_eq!(
            app.pending_search_jump,
            Some(crate::app::SearchJumpDirection::Last)
        );
    }

    #[test]
    fn test_normal_n_does_nothing_without_search_query() {
        let mut app = AppState::new(make_sessions(3));
        handle_key(&mut app, make_key(KeyCode::Char('n'))).unwrap();
        assert_eq!(app.mode, AppMode::Normal);
        assert_eq!(app.search_match_current, None);
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn test_normal_n_no_matches_does_nothing() {
        let mut app = AppState::new(make_sessions(3));
        handle_key(&mut app, make_key(KeyCode::Char('n'))).unwrap();
        assert_eq!(app.search_match_current, None);
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn test_viewing_shift_n_cross_session_when_session_panel() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::SessionList;
        app.search_query = "test".into();
        app.selected_index = 2;
        app.sync_list_state();
        app.search_match_positions = vec![(5, 0), (15, 0)];
        app.search_match_current = Some(0); // at first match
        handle_key(&mut app, make_key(KeyCode::Char('N'))).unwrap();
        assert_eq!(app.selected_index, 1); // moved to previous session
        assert_eq!(
            app.pending_search_jump,
            Some(crate::app::SearchJumpDirection::Last)
        );
    }

    // --- Additional edge case tests ---

    #[test]
    fn test_normal_navigation_zero_sessions() {
        let mut app = AppState::new(vec![]);
        // All navigation keys should not panic with empty session list
        handle_key(&mut app, make_key(KeyCode::Char('j'))).unwrap();
        handle_key(&mut app, make_key(KeyCode::Char('k'))).unwrap();
        handle_key(&mut app, make_key(KeyCode::Char('g'))).unwrap();
        handle_key(&mut app, make_key(KeyCode::Char('G'))).unwrap();
        handle_key(&mut app, make_key(KeyCode::Right)).unwrap();
        handle_key(&mut app, make_key(KeyCode::Left)).unwrap();
        handle_key(&mut app, make_key_ctrl('d')).unwrap();
        handle_key(&mut app, make_key_ctrl('u')).unwrap();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_normal_enter_with_zero_sessions() {
        let mut app = AppState::new(vec![]);
        handle_key(&mut app, make_key(KeyCode::Enter)).unwrap();
        // No session to resume — should not quit or set resume target.
        assert_eq!(app.mode, AppMode::Normal);
        assert!(!app.should_quit);
        assert!(app.resume_session_id.is_none());
    }

    #[test]
    fn test_search_mode_empty_query_escape() {
        let mut app = AppState::new(make_sessions(3));
        app.enter_search();
        assert_eq!(app.mode, AppMode::FuzzySearch);

        // Escape with empty query should cancel
        handle_key(&mut app, make_key(KeyCode::Esc)).unwrap();
        assert_eq!(app.mode, AppMode::Normal);
        assert!(app.search_query.is_empty());
    }

    #[test]
    fn test_date_filter_empty_input_enter() {
        let mut app = AppState::new(make_sessions(3));
        app.enter_date_filter();
        assert_eq!(app.mode, AppMode::DateFilter);

        // Enter with empty date inputs should apply (no date filter)
        handle_key(&mut app, make_key(KeyCode::Enter)).unwrap();
        assert_eq!(app.mode, AppMode::Normal);
        assert_eq!(app.filtered_indices.len(), 3);
    }

    #[test]
    fn test_date_filter_invalid_date_enter() {
        let mut app = AppState::new(make_sessions(3));
        app.enter_date_filter();
        app.date_from_input = "invalid".into();

        // Enter with invalid date should still apply (unparseable → no date constraint)
        handle_key(&mut app, make_key(KeyCode::Enter)).unwrap();
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn test_items_per_page_larger_than_sessions() {
        let mut app = AppState::new(make_sessions(3));
        app.items_per_page = 100;

        // Page down should go to last item, not overflow
        handle_key(&mut app, make_key(KeyCode::Right)).unwrap();
        assert_eq!(app.selected_index, 2);

        // Page up should go back to first
        handle_key(&mut app, make_key(KeyCode::Left)).unwrap();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_viewing_scroll_zero_sessions() {
        let mut app = AppState::new(vec![]);
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::ConversationView;

        // Scrolling with no conversation should not panic
        handle_key(&mut app, make_key(KeyCode::Char('j'))).unwrap();
        handle_key(&mut app, make_key(KeyCode::Char('k'))).unwrap();
        handle_key(&mut app, make_key(KeyCode::Char('g'))).unwrap();
        handle_key(&mut app, make_key(KeyCode::Char('G'))).unwrap();
    }

    #[test]
    fn test_search_backspace_on_empty_query() {
        let mut app = AppState::new(make_sessions(3));
        app.enter_search();
        // Backspace on empty query should not panic
        handle_key(&mut app, make_key(KeyCode::Backspace)).unwrap();
        assert!(app.search_query.is_empty());
    }

    // --- Mouse Handler Tests ---

    fn make_mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn test_mouse_ignored_during_search_mode() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::FuzzySearch;
        app.panel_geometry.conversation_body = Some(ratatui::layout::Rect::new(40, 4, 60, 20));
        handle_mouse(
            &mut app,
            make_mouse(MouseEventKind::Down(MouseButton::Left), 50, 10),
        );
        assert!(app.text_selection.is_none());
    }

    #[test]
    fn test_mouse_ignored_during_help_mode() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Help;
        handle_mouse(
            &mut app,
            make_mouse(MouseEventKind::Down(MouseButton::Left), 50, 10),
        );
        assert!(app.text_selection.is_none());
    }

    #[test]
    fn test_mouse_click_conversation_starts_selection() {
        let mut app = AppState::new(make_sessions(3));
        app.panel_geometry.conversation_body = Some(ratatui::layout::Rect::new(40, 4, 60, 20));
        // Add some cached lines so position is valid.
        use ratatui::text::{Line as TLine, Span};
        app.conversation_lines_cache = vec![
            TLine::from(vec![Span::raw("│ "), Span::raw("  "), Span::raw("Hello")]),
            TLine::from(vec![Span::raw("│ "), Span::raw("  "), Span::raw("World")]),
        ];
        handle_mouse(
            &mut app,
            make_mouse(MouseEventKind::Down(MouseButton::Left), 50, 5),
        );
        assert!(app.text_selection.is_some());
        let sel = app.text_selection.as_ref().unwrap();
        assert!(sel.active);
        assert_eq!(sel.anchor, sel.cursor);
    }

    #[test]
    fn test_mouse_click_outside_panels_clears_selection() {
        let mut app = AppState::new(make_sessions(3));
        app.text_selection = Some(TextSelection {
            anchor: ContentPosition::new(0, 0),
            cursor: ContentPosition::new(1, 5),
            active: false,
        });
        // Click outside any panel (no geometry set).
        handle_mouse(
            &mut app,
            make_mouse(MouseEventKind::Down(MouseButton::Left), 0, 0),
        );
        assert!(app.text_selection.is_none());
    }

    #[test]
    fn test_mouse_drag_updates_cursor() {
        let mut app = AppState::new(make_sessions(3));
        app.panel_geometry.conversation_body = Some(ratatui::layout::Rect::new(40, 4, 60, 20));
        use ratatui::text::{Line as TLine, Span};
        app.conversation_lines_cache = vec![
            TLine::from(vec![Span::raw("│ "), Span::raw("  "), Span::raw("Hello")]),
            TLine::from(vec![Span::raw("│ "), Span::raw("  "), Span::raw("World")]),
        ];
        // Start selection.
        handle_mouse(
            &mut app,
            make_mouse(MouseEventKind::Down(MouseButton::Left), 50, 4),
        );
        // Drag to a different position.
        handle_mouse(
            &mut app,
            make_mouse(MouseEventKind::Drag(MouseButton::Left), 55, 5),
        );
        let sel = app.text_selection.as_ref().unwrap();
        assert!(sel.active);
        assert_ne!(sel.anchor, sel.cursor);
    }

    #[test]
    fn test_mouse_drag_clamps_to_conversation_bounds() {
        let mut app = AppState::new(make_sessions(3));
        let rect = ratatui::layout::Rect::new(40, 4, 60, 20);
        app.panel_geometry.conversation_body = Some(rect);
        use ratatui::text::{Line as TLine, Span};
        app.conversation_lines_cache = vec![TLine::from(vec![
            Span::raw("│ "),
            Span::raw("  "),
            Span::raw("Hello"),
        ])];
        // Start selection inside conversation.
        handle_mouse(
            &mut app,
            make_mouse(MouseEventKind::Down(MouseButton::Left), 50, 4),
        );
        // Drag far outside conversation bounds (into session list).
        handle_mouse(
            &mut app,
            make_mouse(MouseEventKind::Drag(MouseButton::Left), 5, 50),
        );
        let sel = app.text_selection.as_ref().unwrap();
        // Cursor should be clamped to the conversation body boundaries.
        assert!(sel.active);
        // The line should be clamped to max available (0 since only 1 line).
        assert_eq!(sel.cursor.line, 0);
    }

    #[test]
    fn test_scroll_in_conversation_panel() {
        let mut app = AppState::new(make_sessions(3));
        app.panel_geometry.conversation_body = Some(ratatui::layout::Rect::new(40, 4, 60, 20));
        app.conversation_scroll = 5;
        handle_mouse(&mut app, make_mouse(MouseEventKind::ScrollUp, 50, 10));
        assert_eq!(app.conversation_scroll, 4);
        handle_mouse(&mut app, make_mouse(MouseEventKind::ScrollDown, 50, 10));
        assert_eq!(app.conversation_scroll, 5);
    }

    #[test]
    fn test_scroll_in_session_list() {
        let mut app = AppState::new(make_sessions(5));
        app.panel_geometry.session_list = Some(ratatui::layout::Rect::new(0, 4, 40, 20));
        assert_eq!(app.selected_index, 0);
        handle_mouse(&mut app, make_mouse(MouseEventKind::ScrollDown, 10, 10));
        assert_eq!(app.selected_index, 1);
        handle_mouse(&mut app, make_mouse(MouseEventKind::ScrollUp, 10, 10));
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_is_in_rect() {
        let rect = ratatui::layout::Rect::new(10, 5, 20, 10);
        assert!(super::is_in_rect(10, 5, Some(rect)));
        assert!(super::is_in_rect(29, 14, Some(rect)));
        assert!(!super::is_in_rect(30, 5, Some(rect)));
        assert!(!super::is_in_rect(10, 15, Some(rect)));
        assert!(!super::is_in_rect(9, 5, Some(rect)));
        assert!(!super::is_in_rect(10, 4, Some(rect)));
        assert!(!super::is_in_rect(0, 0, None));
    }

    #[test]
    fn test_compute_content_position() {
        let mut app = AppState::new(make_sessions(1));
        use ratatui::text::{Line as TLine, Span};
        app.conversation_lines_cache = vec![
            TLine::from(vec![Span::raw("│ "), Span::raw("  "), Span::raw("Hello")]),
            TLine::from(vec![Span::raw("│ "), Span::raw("  "), Span::raw("World")]),
        ];
        app.conversation_scroll = 0;
        let rect = ratatui::layout::Rect::new(40, 4, 60, 20);
        // Click at col=48, row=4 -> visual_col=8, content_col=8-4=4, line=0
        let pos = super::compute_content_position(&app, 48, 4, rect).unwrap();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.col, 4);
        // Click at col=44, row=5 -> visual_col=4, content_col=0, line=1
        let pos = super::compute_content_position(&app, 44, 5, rect).unwrap();
        assert_eq!(pos.line, 1);
        assert_eq!(pos.col, 0);
    }
}
