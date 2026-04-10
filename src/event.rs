use crate::app::{AppMode, AppState, DateField, Panel};
use crate::filter;
use crate::session;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
        (KeyCode::Enter, _) | (KeyCode::Char('l'), _) => {
            if app.active_panel == Panel::ConversationView {
                app.toggle_panel();
            } else if let Some(session) = app.selected_session() {
                let path = session.file_path.clone();
                app.enter_viewing();
                if let Ok(messages) = session::load_conversation(&path) {
                    let display = session::display_messages(&messages);
                    app.conversation = display.into_iter().cloned().collect();
                }
            }
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
                enter_viewing_with_search_jump(app, crate::app::SearchJumpDirection::First);
            }
        }
        (KeyCode::Char('N'), _) => {
            if app.active_panel == Panel::ConversationView {
                app.jump_to_prev_match();
            } else if !app.search_query.is_empty() {
                enter_viewing_with_search_jump(app, crate::app::SearchJumpDirection::Last);
            }
        }
        (KeyCode::Char('h'), _) | (KeyCode::Char('?'), _) => {
            app.toggle_help();
        }
        (KeyCode::Char('r'), _) => {
            app.request_resume();
        }
        (KeyCode::Char('R'), _) => {
            // Reload is handled by main loop since it needs claude_dir path
        }
        (KeyCode::Tab, _) => {
            app.toggle_panel();
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
        (KeyCode::Char('r'), _) => {
            app.request_resume();
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
        (KeyCode::Tab, _) | (KeyCode::Enter, _) => {
            app.toggle_panel();
        }
        (KeyCode::Char('n'), _) => {
            if app.jump_to_next_match_cross_session() {
                reload_conversation(app);
            }
        }
        (KeyCode::Char('N'), _) => {
            if app.jump_to_prev_match_cross_session() {
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
        KeyCode::Enter => {
            // Apply search and return to normal
            if !app.search_cache_loading {
                let query = app.search_query.clone();
                let cache = &app.search_content_cache;
                let indices = filter::fuzzy_filter(&app.sessions, &query, cache);
                app.update_filtered_indices(indices);
            }
            app.clear_search_content_cache();
            app.search_cache_loading = false;
            app.search_cache_receiver = None;
            app.mode = AppMode::Normal;
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            // Live filter (skip while cache is loading)
            if !app.search_cache_loading {
                let cache = &app.search_content_cache;
                let indices = filter::fuzzy_filter(&app.sessions, &app.search_query, cache);
                app.update_filtered_indices(indices);
            }
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            // Live filter (skip while cache is loading)
            if !app.search_cache_loading {
                let cache = &app.search_content_cache;
                let indices = filter::fuzzy_filter(&app.sessions, &app.search_query, cache);
                app.update_filtered_indices(indices);
            }
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
fn enter_viewing_with_search_jump(app: &mut AppState, direction: crate::app::SearchJumpDirection) {
    if app.selected_session().is_some() {
        let path = app.selected_session().unwrap().file_path.clone();
        app.enter_viewing();
        if let Ok(messages) = session::load_conversation(&path) {
            let display = session::display_messages(&messages);
            app.conversation = display.into_iter().cloned().collect();
        }
        app.pending_search_jump = Some(direction);
    }
}

fn reload_conversation(app: &mut AppState) {
    if let Some(session) = app.selected_session() {
        let path = session.file_path.clone();
        if let Ok(messages) = session::load_conversation(&path) {
            let display = session::display_messages(&messages);
            app.conversation = display.into_iter().cloned().collect();
        }
    }
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
            .map(|i| SessionIndex {
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
    fn test_search_enter_preserves_list_while_cache_loading() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::FuzzySearch;
        app.search_cache_loading = true;
        app.search_query = "test".into();
        handle_key(&mut app, make_key(KeyCode::Enter)).unwrap();
        // Should return to Normal without clearing the list
        assert_eq!(app.mode, AppMode::Normal);
        assert_eq!(app.filtered_indices.len(), 5);
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
    fn test_enter_toggles_panel_in_normal_mode_conversation_panel() {
        let mut app = AppState::new(make_sessions(3));
        app.active_panel = Panel::ConversationView;
        handle_key(&mut app, make_key(KeyCode::Enter)).unwrap();
        assert_eq!(app.active_panel, Panel::SessionList);
        // Should NOT enter viewing mode — just toggle panel
        assert_eq!(app.mode, AppMode::Normal);
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
    fn test_enter_toggles_panel_in_viewing_mode() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Viewing;
        assert_eq!(app.active_panel, crate::app::Panel::SessionList);
        handle_key(&mut app, make_key(KeyCode::Enter)).unwrap();
        assert_eq!(app.active_panel, crate::app::Panel::ConversationView);
        handle_key(&mut app, make_key(KeyCode::Enter)).unwrap();
        assert_eq!(app.active_panel, crate::app::Panel::SessionList);
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
    fn test_viewing_r_requests_resume() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Viewing;
        app.selected_index = 1;
        handle_key(&mut app, make_key(KeyCode::Char('r'))).unwrap();
        assert!(app.should_quit);
        assert_eq!(app.resume_session_id.as_deref(), Some("sess-1"));
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
    fn test_r_requests_resume() {
        let mut app = AppState::new(make_sessions(3));
        handle_key(&mut app, make_key(KeyCode::Char('r'))).unwrap();
        assert!(app.should_quit);
        assert_eq!(app.resume_session_id.as_deref(), Some("sess-0"));
    }

    #[test]
    fn test_r_resume_empty_sessions() {
        let mut app = AppState::new(vec![]);
        handle_key(&mut app, make_key(KeyCode::Char('r'))).unwrap();
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
    fn test_viewing_n_navigates_within_conversation_then_cross_session() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::ConversationView;
        app.search_query = "test".into();
        app.search_match_positions = vec![(5, 0), (15, 0)];
        // First n: navigate within session (not at last match yet)
        handle_key(&mut app, make_key(KeyCode::Char('n'))).unwrap();
        assert_eq!(app.search_match_current, Some(0));
        assert_eq!(app.selected_index, 0); // still same session
        handle_key(&mut app, make_key(KeyCode::Char('n'))).unwrap();
        assert_eq!(app.search_match_current, Some(1)); // now at last match
        // Next n at last match: should advance to next session
        handle_key(&mut app, make_key(KeyCode::Char('n'))).unwrap();
        assert_eq!(app.selected_index, 1); // advanced to next session
        assert_eq!(
            app.pending_search_jump,
            Some(crate::app::SearchJumpDirection::First)
        );
    }

    #[test]
    fn test_viewing_shift_n_navigates_within_conversation_then_cross_session() {
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
        // Next N at first match: should go to previous session
        handle_key(&mut app, make_key(KeyCode::Char('N'))).unwrap();
        assert_eq!(app.selected_index, 4); // wrapped to last session
        assert_eq!(
            app.pending_search_jump,
            Some(crate::app::SearchJumpDirection::Last)
        );
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
    fn test_normal_n_enters_viewing_from_session_list_with_search() {
        let mut app = AppState::new(make_sessions(5));
        app.search_query = "test".into();
        // active_panel defaults to SessionList
        assert_eq!(app.active_panel, Panel::SessionList);
        handle_key(&mut app, make_key(KeyCode::Char('n'))).unwrap();
        // Should enter Viewing mode and set pending jump
        assert_eq!(app.mode, AppMode::Viewing);
        assert_eq!(app.active_panel, Panel::ConversationView);
        assert_eq!(
            app.pending_search_jump,
            Some(crate::app::SearchJumpDirection::First)
        );
    }

    #[test]
    fn test_normal_shift_n_enters_viewing_from_session_list_with_search() {
        let mut app = AppState::new(make_sessions(5));
        app.search_query = "test".into();
        handle_key(&mut app, make_key(KeyCode::Char('N'))).unwrap();
        assert_eq!(app.mode, AppMode::Viewing);
        assert_eq!(app.active_panel, Panel::ConversationView);
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
}
