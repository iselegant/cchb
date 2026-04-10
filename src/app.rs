use crate::filter;
use crate::session::{self, ConversationMessage, SessionIndex};
use chrono::{Days, Local};
use ratatui::widgets::ListState;
use std::sync::mpsc;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    Viewing,
    FuzzySearch,
    DateFilter,
    Help,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Panel {
    SessionList,
    ConversationView,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DateField {
    From,
    To,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SearchJumpDirection {
    First,
    Last,
}

/// Maximum total size of search content cache (1 GB).
const SEARCH_CACHE_MAX_BYTES: usize = 1_073_741_824;

pub struct AppState {
    pub mode: AppMode,
    pub active_panel: Panel,
    pub sessions: Vec<SessionIndex>,
    pub filtered_indices: Vec<usize>,
    pub selected_index: usize,
    pub list_state: ListState,
    pub conversation: Vec<ConversationMessage>,
    pub conversation_scroll: usize,
    pub search_query: String,
    pub date_from_input: String,
    pub date_to_input: String,
    pub date_field: DateField,
    pub should_quit: bool,
    /// Tracks which session_id is currently loaded in the conversation pane
    pub loaded_session_id: Option<String>,
    /// Set when user requests session resume; main loop will launch claude --resume
    pub resume_session_id: Option<String>,
    /// Project path for the session to resume (used as cwd for claude --resume)
    pub resume_project_path: Option<String>,
    /// Number of session items visible in the list panel (updated each render cycle)
    pub items_per_page: usize,
    /// Cached searchable text per session, built on search entry, cleared on exit.
    /// Each entry corresponds to sessions[i]. Empty string means not cached (e.g. over size limit).
    pub search_content_cache: Vec<String>,
    /// True while the background thread is building the search content cache.
    pub search_cache_loading: bool,
    /// Receiver for the background cache-building thread result.
    pub search_cache_receiver: Option<mpsc::Receiver<Vec<String>>>,
    /// All search match positions as (line_index, occurrence_index_within_line).
    /// Recomputed each render cycle by the UI layer.
    pub search_match_positions: Vec<(usize, usize)>,
    /// Current index into `search_match_positions` (None when no navigation has occurred).
    pub search_match_current: Option<usize>,
    /// Deferred jump direction after cross-session search navigation.
    pub pending_search_jump: Option<SearchJumpDirection>,
    /// The `selected_index` where cross-session navigation started, to detect full-cycle wrap.
    pub pending_search_jump_origin: Option<usize>,
    /// True while the "Reloaded" indicator should be shown.
    pub conversation_reloading: bool,
    /// When the reload indicator was triggered (used to auto-dismiss after a short duration).
    pub conversation_reload_at: Option<Instant>,
}

impl AppState {
    pub fn new(sessions: Vec<SessionIndex>) -> Self {
        let filtered_indices: Vec<usize> = (0..sessions.len()).collect();
        let mut list_state = ListState::default();
        if !sessions.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            mode: AppMode::Normal,
            active_panel: Panel::SessionList,
            sessions,
            filtered_indices,
            selected_index: 0,
            list_state,
            conversation: Vec::new(),
            conversation_scroll: 0,
            search_query: String::new(),
            date_from_input: String::new(),
            date_to_input: String::new(),
            date_field: DateField::From,
            should_quit: false,
            loaded_session_id: None,
            resume_session_id: None,
            resume_project_path: None,
            items_per_page: 5,
            search_content_cache: Vec::new(),
            search_cache_loading: false,
            search_cache_receiver: None,
            search_match_positions: Vec::new(),
            search_match_current: None,
            pending_search_jump: None,
            pending_search_jump_origin: None,
            conversation_reloading: false,
            conversation_reload_at: None,
        }
    }

    pub fn selected_session(&self) -> Option<&SessionIndex> {
        self.filtered_indices
            .get(self.selected_index)
            .and_then(|&i| self.sessions.get(i))
    }

    pub fn sync_list_state(&mut self) {
        if self.filtered_indices.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(self.selected_index));
        }
    }

    pub fn select_next(&mut self) {
        if !self.filtered_indices.is_empty()
            && self.selected_index < self.filtered_indices.len() - 1
        {
            self.selected_index += 1;
            self.sync_list_state();
        }
    }

    pub fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.sync_list_state();
        }
    }

    pub fn go_top(&mut self) {
        self.selected_index = 0;
        self.sync_list_state();
    }

    pub fn go_bottom(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected_index = self.filtered_indices.len() - 1;
            self.sync_list_state();
        }
    }

    pub fn half_page_down(&mut self, visible_height: usize) {
        let half = visible_height / 2;
        match self.mode {
            AppMode::Normal => {
                let max = self.filtered_indices.len().saturating_sub(1);
                self.selected_index = (self.selected_index + half).min(max);
                self.sync_list_state();
            }
            AppMode::Viewing => {
                self.conversation_scroll += half;
            }
            _ => {}
        }
    }

    pub fn half_page_up(&mut self, visible_height: usize) {
        let half = visible_height / 2;
        match self.mode {
            AppMode::Normal => {
                self.selected_index = self.selected_index.saturating_sub(half);
                self.sync_list_state();
            }
            AppMode::Viewing => {
                self.conversation_scroll = self.conversation_scroll.saturating_sub(half);
            }
            _ => {}
        }
    }

    pub fn page_down(&mut self) {
        if !self.filtered_indices.is_empty() {
            let max = self.filtered_indices.len() - 1;
            self.selected_index = (self.selected_index + self.items_per_page).min(max);
            self.sync_list_state();
        }
    }

    pub fn page_up(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(self.items_per_page);
        self.sync_list_state();
    }

    pub fn scroll_conversation_down(&mut self) {
        self.conversation_scroll += 1;
    }

    pub fn scroll_conversation_up(&mut self) {
        self.conversation_scroll = self.conversation_scroll.saturating_sub(1);
    }

    pub fn scroll_conversation_top(&mut self) {
        self.conversation_scroll = 0;
    }

    pub fn enter_viewing(&mut self) {
        if self.selected_session().is_some() {
            self.mode = AppMode::Viewing;
            self.active_panel = Panel::ConversationView;
            self.conversation_scroll = 0;
        }
    }

    pub fn exit_viewing(&mut self) {
        self.mode = AppMode::Normal;
        self.active_panel = Panel::SessionList;
    }

    pub fn enter_search(&mut self) {
        self.search_query.clear();
        self.search_content_cache.clear();
        self.search_cache_loading = true;
        self.mode = AppMode::FuzzySearch;

        // Collect file paths to move into the background thread.
        let paths: Vec<std::path::PathBuf> =
            self.sessions.iter().map(|s| s.file_path.clone()).collect();

        let (tx, rx) = mpsc::channel();
        self.search_cache_receiver = Some(rx);

        std::thread::spawn(move || {
            let mut cache = Vec::with_capacity(paths.len());
            let mut total_bytes: usize = 0;
            for path in &paths {
                if total_bytes >= SEARCH_CACHE_MAX_BYTES {
                    cache.push(String::new());
                    continue;
                }
                let text = session::extract_searchable_text(path);
                total_bytes = total_bytes.saturating_add(text.len());
                cache.push(text);
            }
            let _ = tx.send(cache);
        });
    }

    /// Poll for background cache completion. Call this from the main loop.
    pub fn poll_search_cache(&mut self) {
        if let Some(rx) = &self.search_cache_receiver
            && let Ok(cache) = rx.try_recv()
        {
            self.search_content_cache = cache;
            self.search_cache_loading = false;
            self.search_cache_receiver = None;

            // Re-apply live filter with current query now that cache is ready.
            if self.mode == AppMode::FuzzySearch {
                let indices = filter::fuzzy_filter(
                    &self.sessions,
                    &self.search_query,
                    &self.search_content_cache,
                );
                self.update_filtered_indices(indices);
            }
        }
    }

    pub fn cancel_search(&mut self) {
        self.search_query.clear();
        self.clear_search_content_cache();
        self.search_cache_loading = false;
        self.search_cache_receiver = None;
        self.mode = AppMode::Normal;
    }

    /// Clear the search content cache to free memory.
    pub fn clear_search_content_cache(&mut self) {
        self.search_content_cache = Vec::new();
    }

    pub fn enter_date_filter(&mut self) {
        let today = Local::now().date_naive();
        let week_ago = today.checked_sub_days(Days::new(7)).unwrap_or(today);
        self.date_from_input = week_ago.format("%Y-%m-%d").to_string();
        self.date_to_input = today.format("%Y-%m-%d").to_string();
        self.date_field = DateField::From;
        self.mode = AppMode::DateFilter;
    }

    /// Increment the active date field by 1 day.
    /// When incrementing From, it cannot exceed To.
    pub fn increment_date_field(&mut self) {
        match self.date_field {
            DateField::From => {
                if let Some(date) = filter::parse_date_input(&self.date_from_input)
                    && let Some(next) = date.checked_add_days(Days::new(1))
                {
                    let to = filter::parse_date_input(&self.date_to_input);
                    if to.is_none() || next <= to.unwrap() {
                        self.date_from_input = next.format("%Y-%m-%d").to_string();
                    }
                }
            }
            DateField::To => {
                let today = Local::now().date_naive();
                if let Some(date) = filter::parse_date_input(&self.date_to_input)
                    && let Some(next) = date.checked_add_days(Days::new(1))
                    && next <= today
                {
                    self.date_to_input = next.format("%Y-%m-%d").to_string();
                }
            }
        }
    }

    /// Decrement the active date field by 1 day.
    /// When decrementing To, it cannot go below From.
    pub fn decrement_date_field(&mut self) {
        match self.date_field {
            DateField::From => {
                if let Some(date) = filter::parse_date_input(&self.date_from_input)
                    && let Some(prev) = date.checked_sub_days(Days::new(1))
                {
                    self.date_from_input = prev.format("%Y-%m-%d").to_string();
                }
            }
            DateField::To => {
                if let Some(date) = filter::parse_date_input(&self.date_to_input)
                    && let Some(prev) = date.checked_sub_days(Days::new(1))
                {
                    let from = filter::parse_date_input(&self.date_from_input);
                    if from.is_none() || prev >= from.unwrap() {
                        self.date_to_input = prev.format("%Y-%m-%d").to_string();
                    }
                }
            }
        }
    }

    pub fn cancel_date_filter(&mut self) {
        self.mode = AppMode::Normal;
    }

    pub fn toggle_date_field(&mut self) {
        self.date_field = match self.date_field {
            DateField::From => DateField::To,
            DateField::To => DateField::From,
        };
    }

    pub fn toggle_help(&mut self) {
        self.mode = match self.mode {
            AppMode::Help => AppMode::Normal,
            _ => AppMode::Help,
        };
    }

    pub fn close_help(&mut self) {
        if self.mode == AppMode::Help {
            self.mode = AppMode::Normal;
        }
    }

    pub fn toggle_panel(&mut self) {
        self.active_panel = match self.active_panel {
            Panel::SessionList => Panel::ConversationView,
            Panel::ConversationView => Panel::SessionList,
        };
    }

    pub fn update_filtered_indices(&mut self, indices: Vec<usize>) {
        self.filtered_indices = indices;
        self.selected_index = 0;
        self.sync_list_state();
    }

    pub fn clear_filters(&mut self) {
        self.search_query.clear();
        self.date_from_input.clear();
        self.date_to_input.clear();
        self.filtered_indices = (0..self.sessions.len()).collect();
        self.selected_index = 0;
        self.sync_list_state();
    }

    pub fn next_session_in_viewing(&mut self) {
        if self.mode == AppMode::Viewing
            && self.selected_index < self.filtered_indices.len().saturating_sub(1)
        {
            self.selected_index += 1;
            self.conversation_scroll = 0;
            self.sync_list_state();
        }
    }

    pub fn prev_session_in_viewing(&mut self) {
        if self.mode == AppMode::Viewing && self.selected_index > 0 {
            self.selected_index -= 1;
            self.conversation_scroll = 0;
            self.sync_list_state();
        }
    }

    /// Number of lines to offset from the top when scrolling to a search match,
    /// so the matched line appears a few lines below the viewport top for readability.
    const SEARCH_SCROLL_MARGIN: usize = 5;

    /// Jump to the next search match line, wrapping around from last to first.
    pub fn jump_to_next_match(&mut self) {
        if self.search_match_positions.is_empty() {
            return;
        }
        let next = match self.search_match_current {
            Some(idx) => (idx + 1) % self.search_match_positions.len(),
            None => 0,
        };
        self.search_match_current = Some(next);
        self.conversation_scroll = self.search_match_positions[next]
            .0
            .saturating_sub(Self::SEARCH_SCROLL_MARGIN);
    }

    /// Jump to the previous search match occurrence, wrapping around from first to last.
    pub fn jump_to_prev_match(&mut self) {
        if self.search_match_positions.is_empty() {
            return;
        }
        let prev = match self.search_match_current {
            Some(0) => self.search_match_positions.len() - 1,
            Some(idx) => idx - 1,
            None => self.search_match_positions.len() - 1,
        };
        self.search_match_current = Some(prev);
        self.conversation_scroll = self.search_match_positions[prev]
            .0
            .saturating_sub(Self::SEARCH_SCROLL_MARGIN);
    }

    /// Jump to next match, crossing session boundaries when SessionList panel is active.
    /// Returns true if the session changed (caller should reload conversation).
    pub fn jump_to_next_match_cross_session(&mut self) -> bool {
        if self.search_query.is_empty() {
            return false;
        }
        // If there are matches and we're not at the last one (or haven't started), navigate within
        if !self.search_match_positions.is_empty() {
            if self.search_match_current.is_none() {
                self.jump_to_next_match();
                return false;
            }
            let at_last = self.search_match_current == Some(self.search_match_positions.len() - 1);
            if !at_last {
                self.jump_to_next_match();
                return false;
            }
        }
        // At last match or no matches: move to next session
        if self.filtered_indices.len() <= 1 {
            // Only one (or zero) session: wrap within conversation
            self.jump_to_next_match();
            return false;
        }
        let origin = self.selected_index;
        self.selected_index = (self.selected_index + 1) % self.filtered_indices.len();
        self.conversation_scroll = 0;
        self.search_match_current = None;
        self.search_match_positions.clear();
        self.sync_list_state();
        self.pending_search_jump = Some(SearchJumpDirection::First);
        if self.pending_search_jump_origin.is_none() {
            self.pending_search_jump_origin = Some(origin);
        }
        true
    }

    /// Jump to previous match, crossing session boundaries when SessionList panel is active.
    /// Returns true if the session changed (caller should reload conversation).
    pub fn jump_to_prev_match_cross_session(&mut self) -> bool {
        if self.search_query.is_empty() {
            return false;
        }
        if !self.search_match_positions.is_empty() {
            // If no current position, let normal prev_match handle it (goes to last)
            if self.search_match_current.is_none() {
                self.jump_to_prev_match();
                return false;
            }
            if self.search_match_current != Some(0) {
                self.jump_to_prev_match();
                return false;
            }
        }
        // At first match or no matches: move to previous session
        if self.filtered_indices.len() <= 1 {
            self.jump_to_prev_match();
            return false;
        }
        let origin = self.selected_index;
        self.selected_index = if self.selected_index == 0 {
            self.filtered_indices.len() - 1
        } else {
            self.selected_index - 1
        };
        self.conversation_scroll = 0;
        self.search_match_current = None;
        self.search_match_positions.clear();
        self.sync_list_state();
        self.pending_search_jump = Some(SearchJumpDirection::Last);
        if self.pending_search_jump_origin.is_none() {
            self.pending_search_jump_origin = Some(origin);
        }
        true
    }

    /// Resolve a pending cross-session search jump after render has populated match positions.
    /// Returns true if cascade is needed (no matches found, must advance to next session).
    pub fn resolve_pending_search_jump(&mut self) -> bool {
        let direction = match &self.pending_search_jump {
            Some(d) => d.clone(),
            None => return false,
        };
        if !self.search_match_positions.is_empty() {
            // Matches found: apply the jump
            match direction {
                SearchJumpDirection::First => {
                    self.search_match_current = Some(0);
                    self.conversation_scroll = self.search_match_positions[0]
                        .0
                        .saturating_sub(Self::SEARCH_SCROLL_MARGIN);
                }
                SearchJumpDirection::Last => {
                    let last = self.search_match_positions.len() - 1;
                    self.search_match_current = Some(last);
                    self.conversation_scroll = self.search_match_positions[last]
                        .0
                        .saturating_sub(Self::SEARCH_SCROLL_MARGIN);
                }
            }
            self.pending_search_jump = None;
            self.pending_search_jump_origin = None;
            return false;
        }
        // No matches in this session: check if we've completed a full cycle
        if self.pending_search_jump_origin == Some(self.selected_index) {
            self.pending_search_jump = None;
            self.pending_search_jump_origin = None;
            return false;
        }
        // Advance to next/prev session and signal cascade
        match direction {
            SearchJumpDirection::First => {
                self.selected_index = (self.selected_index + 1) % self.filtered_indices.len();
            }
            SearchJumpDirection::Last => {
                self.selected_index = if self.selected_index == 0 {
                    self.filtered_indices.len() - 1
                } else {
                    self.selected_index - 1
                };
            }
        }
        self.conversation_scroll = 0;
        self.search_match_current = None;
        self.sync_list_state();
        true
    }

    /// Request reload of the current conversation. Sets the reload indicator and clears
    /// loaded_session_id so the main loop will re-read the file.
    pub fn request_reload_conversation(&mut self) {
        self.loaded_session_id = None;
        self.conversation_reloading = true;
        self.conversation_reload_at = Some(Instant::now());
    }

    /// Clear the reload indicator after 500ms have elapsed.
    pub fn check_reload_expired(&mut self) {
        if let Some(at) = self.conversation_reload_at
            && at.elapsed() >= std::time::Duration::from_millis(500)
        {
            self.conversation_reloading = false;
            self.conversation_reload_at = None;
        }
    }

    /// Request resuming the currently selected session. Sets the session ID/project path and quits.
    pub fn request_resume(&mut self) {
        let info = self
            .selected_session()
            .map(|s| (s.session_id.clone(), s.project_path.clone()));
        if let Some((session_id, project_path)) = info {
            self.resume_session_id = Some(session_id);
            self.resume_project_path = Some(project_path);
            self.should_quit = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionIndex;
    use chrono::Utc;
    use std::path::PathBuf;

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
    fn test_new_initial_state() {
        let app = AppState::new(make_sessions(5));
        assert_eq!(app.mode, AppMode::Normal);
        assert_eq!(app.active_panel, Panel::SessionList);
        assert_eq!(app.selected_index, 0);
        assert_eq!(app.filtered_indices.len(), 5);
        assert!(!app.should_quit);
    }

    #[test]
    fn test_new_empty_sessions() {
        let app = AppState::new(vec![]);
        assert_eq!(app.filtered_indices.len(), 0);
        assert!(app.selected_session().is_none());
    }

    #[test]
    fn test_select_next() {
        let mut app = AppState::new(make_sessions(3));
        app.select_next();
        assert_eq!(app.selected_index, 1);
        app.select_next();
        assert_eq!(app.selected_index, 2);
        app.select_next(); // clamp
        assert_eq!(app.selected_index, 2);
    }

    #[test]
    fn test_select_prev() {
        let mut app = AppState::new(make_sessions(3));
        app.selected_index = 2;
        app.select_prev();
        assert_eq!(app.selected_index, 1);
        app.select_prev();
        assert_eq!(app.selected_index, 0);
        app.select_prev(); // clamp
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_go_top() {
        let mut app = AppState::new(make_sessions(5));
        app.selected_index = 4;
        app.go_top();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_go_bottom() {
        let mut app = AppState::new(make_sessions(5));
        app.go_bottom();
        assert_eq!(app.selected_index, 4);
    }

    #[test]
    fn test_go_bottom_empty() {
        let mut app = AppState::new(vec![]);
        app.go_bottom();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_half_page_down_normal() {
        let mut app = AppState::new(make_sessions(20));
        app.half_page_down(10); // half = 5
        assert_eq!(app.selected_index, 5);
        app.selected_index = 18;
        app.half_page_down(10); // clamp to 19
        assert_eq!(app.selected_index, 19);
    }

    #[test]
    fn test_half_page_up_normal() {
        let mut app = AppState::new(make_sessions(20));
        app.selected_index = 10;
        app.half_page_up(10); // half = 5
        assert_eq!(app.selected_index, 5);
        app.half_page_up(10);
        assert_eq!(app.selected_index, 0);
        app.half_page_up(10); // clamp at 0
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_half_page_down_viewing() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.half_page_down(10);
        assert_eq!(app.conversation_scroll, 5);
    }

    #[test]
    fn test_half_page_up_viewing() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.conversation_scroll = 10;
        app.half_page_up(10);
        assert_eq!(app.conversation_scroll, 5);
    }

    #[test]
    fn test_enter_viewing_mode() {
        let mut app = AppState::new(make_sessions(3));
        app.enter_viewing();
        assert_eq!(app.mode, AppMode::Viewing);
        assert_eq!(app.active_panel, Panel::ConversationView);
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn test_enter_viewing_empty_sessions() {
        let mut app = AppState::new(vec![]);
        app.enter_viewing();
        assert_eq!(app.mode, AppMode::Normal); // should not change
    }

    #[test]
    fn test_exit_viewing() {
        let mut app = AppState::new(make_sessions(3));
        app.enter_viewing();
        app.exit_viewing();
        assert_eq!(app.mode, AppMode::Normal);
        assert_eq!(app.active_panel, Panel::SessionList);
    }

    #[test]
    fn test_enter_search() {
        let mut app = AppState::new(make_sessions(3));
        app.search_query = "old query".into();
        app.enter_search();
        assert_eq!(app.mode, AppMode::FuzzySearch);
        assert_eq!(app.search_query, "");
    }

    #[test]
    fn test_cancel_search() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::FuzzySearch;
        app.search_query = "test".into();
        app.cancel_search();
        assert_eq!(app.mode, AppMode::Normal);
        assert_eq!(app.search_query, "");
    }

    #[test]
    fn test_enter_date_filter() {
        let mut app = AppState::new(make_sessions(3));
        app.enter_date_filter();
        assert_eq!(app.mode, AppMode::DateFilter);
        assert_eq!(app.date_field, DateField::From);
        // Verify preset values: from = today - 7 days, to = today
        let today = Local::now().date_naive();
        let week_ago = today.checked_sub_days(Days::new(7)).unwrap();
        assert_eq!(app.date_from_input, week_ago.format("%Y-%m-%d").to_string());
        assert_eq!(app.date_to_input, today.format("%Y-%m-%d").to_string());
    }

    #[test]
    fn test_increment_date_field_from() {
        let mut app = AppState::new(make_sessions(3));
        app.date_from_input = "2026-04-05".to_string();
        app.date_field = DateField::From;
        app.increment_date_field();
        assert_eq!(app.date_from_input, "2026-04-06");
    }

    #[test]
    fn test_decrement_date_field_from() {
        let mut app = AppState::new(make_sessions(3));
        app.date_from_input = "2026-04-05".to_string();
        app.date_field = DateField::From;
        app.decrement_date_field();
        assert_eq!(app.date_from_input, "2026-04-04");
    }

    #[test]
    fn test_increment_date_field_to() {
        let mut app = AppState::new(make_sessions(3));
        // Use a date far enough in the past so the today-clamp never blocks increment
        app.date_to_input = "2020-01-01".to_string();
        app.date_field = DateField::To;
        app.increment_date_field();
        assert_eq!(app.date_to_input, "2020-01-02");
    }

    #[test]
    fn test_decrement_date_field_to() {
        let mut app = AppState::new(make_sessions(3));
        app.date_to_input = "2026-04-08".to_string();
        app.date_field = DateField::To;
        app.decrement_date_field();
        assert_eq!(app.date_to_input, "2026-04-07");
    }

    #[test]
    fn test_increment_invalid_date_no_change() {
        let mut app = AppState::new(make_sessions(3));
        app.date_from_input = "invalid".to_string();
        app.date_field = DateField::From;
        app.increment_date_field();
        assert_eq!(app.date_from_input, "invalid");
    }

    #[test]
    fn test_decrement_month_boundary() {
        let mut app = AppState::new(make_sessions(3));
        app.date_from_input = "2026-05-01".to_string();
        app.date_field = DateField::From;
        app.decrement_date_field();
        assert_eq!(app.date_from_input, "2026-04-30");
    }

    #[test]
    fn test_increment_from_clamped_by_to() {
        let mut app = AppState::new(make_sessions(3));
        app.date_from_input = "2026-04-08".to_string();
        app.date_to_input = "2026-04-08".to_string();
        app.date_field = DateField::From;
        app.increment_date_field();
        // From should not exceed To
        assert_eq!(app.date_from_input, "2026-04-08");
    }

    #[test]
    fn test_decrement_to_clamped_by_from() {
        let mut app = AppState::new(make_sessions(3));
        app.date_from_input = "2026-04-05".to_string();
        app.date_to_input = "2026-04-05".to_string();
        app.date_field = DateField::To;
        app.decrement_date_field();
        // To should not go below From
        assert_eq!(app.date_to_input, "2026-04-05");
    }

    #[test]
    fn test_increment_from_allowed_when_below_to() {
        let mut app = AppState::new(make_sessions(3));
        app.date_from_input = "2026-04-05".to_string();
        app.date_to_input = "2026-04-08".to_string();
        app.date_field = DateField::From;
        app.increment_date_field();
        assert_eq!(app.date_from_input, "2026-04-06");
    }

    #[test]
    fn test_decrement_to_allowed_when_above_from() {
        let mut app = AppState::new(make_sessions(3));
        app.date_from_input = "2026-04-05".to_string();
        app.date_to_input = "2026-04-08".to_string();
        app.date_field = DateField::To;
        app.decrement_date_field();
        assert_eq!(app.date_to_input, "2026-04-07");
    }

    #[test]
    fn test_to_increment_clamped_by_today() {
        let mut app = AppState::new(make_sessions(3));
        let today = Local::now().date_naive();
        app.date_from_input = "2020-01-01".to_string();
        app.date_to_input = today.format("%Y-%m-%d").to_string();
        app.date_field = DateField::To;
        app.increment_date_field();
        // To should not exceed today
        assert_eq!(app.date_to_input, today.format("%Y-%m-%d").to_string());
    }

    #[test]
    fn test_to_increment_allowed_when_below_today() {
        let mut app = AppState::new(make_sessions(3));
        let today = Local::now().date_naive();
        let yesterday = today.checked_sub_days(Days::new(1)).unwrap();
        app.date_from_input = "2020-01-01".to_string();
        app.date_to_input = yesterday.format("%Y-%m-%d").to_string();
        app.date_field = DateField::To;
        app.increment_date_field();
        assert_eq!(app.date_to_input, today.format("%Y-%m-%d").to_string());
    }

    #[test]
    fn test_from_decrement_unconstrained() {
        let mut app = AppState::new(make_sessions(3));
        app.date_from_input = "2026-04-05".to_string();
        app.date_to_input = "2026-04-08".to_string();
        app.date_field = DateField::From;
        app.decrement_date_field();
        // From can always decrement freely
        assert_eq!(app.date_from_input, "2026-04-04");
    }

    #[test]
    fn test_toggle_date_field() {
        let mut app = AppState::new(make_sessions(3));
        app.enter_date_filter();
        assert_eq!(app.date_field, DateField::From);
        app.toggle_date_field();
        assert_eq!(app.date_field, DateField::To);
        app.toggle_date_field();
        assert_eq!(app.date_field, DateField::From);
    }

    #[test]
    fn test_toggle_help() {
        let mut app = AppState::new(make_sessions(3));
        app.toggle_help();
        assert_eq!(app.mode, AppMode::Help);
        app.toggle_help();
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn test_close_help() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Help;
        app.close_help();
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn test_toggle_panel() {
        let mut app = AppState::new(make_sessions(3));
        assert_eq!(app.active_panel, Panel::SessionList);
        app.toggle_panel();
        assert_eq!(app.active_panel, Panel::ConversationView);
        app.toggle_panel();
        assert_eq!(app.active_panel, Panel::SessionList);
    }

    #[test]
    fn test_update_filtered_indices() {
        let mut app = AppState::new(make_sessions(5));
        app.selected_index = 3;
        app.update_filtered_indices(vec![1, 3]);
        assert_eq!(app.filtered_indices, vec![1, 3]);
        assert_eq!(app.selected_index, 0); // reset
    }

    #[test]
    fn test_clear_filters() {
        let mut app = AppState::new(make_sessions(5));
        app.search_query = "query".into();
        app.filtered_indices = vec![0, 2];
        app.selected_index = 1;
        app.clear_filters();
        assert_eq!(app.search_query, "");
        assert_eq!(app.filtered_indices.len(), 5);
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_next_session_in_viewing() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Viewing;
        app.conversation_scroll = 10;
        app.next_session_in_viewing();
        assert_eq!(app.selected_index, 1);
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn test_prev_session_in_viewing() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Viewing;
        app.selected_index = 2;
        app.conversation_scroll = 10;
        app.prev_session_in_viewing();
        assert_eq!(app.selected_index, 1);
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn test_scroll_conversation() {
        let mut app = AppState::new(make_sessions(3));
        app.scroll_conversation_down();
        app.scroll_conversation_down();
        assert_eq!(app.conversation_scroll, 2);
        app.scroll_conversation_up();
        assert_eq!(app.conversation_scroll, 1);
        app.scroll_conversation_top();
        assert_eq!(app.conversation_scroll, 0);
        app.scroll_conversation_up(); // clamp at 0
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn test_selected_session() {
        let app = AppState::new(make_sessions(3));
        let session = app.selected_session().unwrap();
        assert_eq!(session.session_id, "sess-0");
    }

    #[test]
    fn test_request_resume() {
        let mut app = AppState::new(make_sessions(3));
        app.request_resume();
        assert!(app.should_quit);
        assert_eq!(app.resume_session_id.as_deref(), Some("sess-0"));
    }

    #[test]
    fn test_page_down() {
        let mut app = AppState::new(make_sessions(30));
        app.items_per_page = 5;
        app.page_down();
        assert_eq!(app.selected_index, 5);
        app.page_down();
        assert_eq!(app.selected_index, 10);
    }

    #[test]
    fn test_page_down_clamp() {
        let mut app = AppState::new(make_sessions(10));
        app.items_per_page = 5;
        app.selected_index = 7;
        app.page_down();
        assert_eq!(app.selected_index, 9); // clamped to last
    }

    #[test]
    fn test_page_down_empty() {
        let mut app = AppState::new(vec![]);
        app.items_per_page = 5;
        app.page_down();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_page_up() {
        let mut app = AppState::new(make_sessions(30));
        app.items_per_page = 5;
        app.selected_index = 15;
        app.page_up();
        assert_eq!(app.selected_index, 10);
        app.page_up();
        assert_eq!(app.selected_index, 5);
    }

    #[test]
    fn test_page_up_clamp() {
        let mut app = AppState::new(make_sessions(10));
        app.items_per_page = 5;
        app.selected_index = 3;
        app.page_up();
        assert_eq!(app.selected_index, 0); // clamped to 0
    }

    #[test]
    fn test_jump_to_next_match_empty() {
        let mut app = AppState::new(make_sessions(3));
        app.jump_to_next_match();
        assert_eq!(app.search_match_current, None);
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn test_jump_to_next_match_single() {
        let mut app = AppState::new(make_sessions(3));
        app.search_match_positions = vec![(10, 0)];
        app.jump_to_next_match();
        assert_eq!(app.search_match_current, Some(0));
        assert_eq!(app.conversation_scroll, 5); // 10 - 5 margin
        // Wrap around
        app.jump_to_next_match();
        assert_eq!(app.search_match_current, Some(0));
        assert_eq!(app.conversation_scroll, 5);
    }

    #[test]
    fn test_jump_to_next_match_multiple() {
        let mut app = AppState::new(make_sessions(3));
        app.search_match_positions = vec![(5, 0), (15, 0), (25, 0)];
        app.jump_to_next_match();
        assert_eq!(app.search_match_current, Some(0));
        assert_eq!(app.conversation_scroll, 0); // 5 - 5 = 0 (saturating)
        app.jump_to_next_match();
        assert_eq!(app.search_match_current, Some(1));
        assert_eq!(app.conversation_scroll, 10); // 15 - 5
        app.jump_to_next_match();
        assert_eq!(app.search_match_current, Some(2));
        assert_eq!(app.conversation_scroll, 20); // 25 - 5
        // Wrap around
        app.jump_to_next_match();
        assert_eq!(app.search_match_current, Some(0));
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn test_jump_to_prev_match_empty() {
        let mut app = AppState::new(make_sessions(3));
        app.jump_to_prev_match();
        assert_eq!(app.search_match_current, None);
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn test_jump_to_prev_match_multiple() {
        let mut app = AppState::new(make_sessions(3));
        app.search_match_positions = vec![(5, 0), (15, 0), (25, 0)];
        // First call without current goes to last
        app.jump_to_prev_match();
        assert_eq!(app.search_match_current, Some(2));
        assert_eq!(app.conversation_scroll, 20); // 25 - 5
        app.jump_to_prev_match();
        assert_eq!(app.search_match_current, Some(1));
        assert_eq!(app.conversation_scroll, 10); // 15 - 5
        app.jump_to_prev_match();
        assert_eq!(app.search_match_current, Some(0));
        assert_eq!(app.conversation_scroll, 0); // 5 - 5 = 0
        // Wrap around
        app.jump_to_prev_match();
        assert_eq!(app.search_match_current, Some(2));
        assert_eq!(app.conversation_scroll, 20);
    }

    #[test]
    fn test_request_resume_empty() {
        let mut app = AppState::new(vec![]);
        app.request_resume();
        assert!(!app.should_quit);
        assert!(app.resume_session_id.is_none());
    }

    // --- Cross-session search navigation tests ---

    #[test]
    fn test_jump_to_next_match_cross_session_no_search_query() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.active_panel = Panel::SessionList;
        let changed = app.jump_to_next_match_cross_session();
        assert!(!changed);
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_jump_to_next_match_cross_session_stays_when_not_at_last() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.search_query = "test".into();
        app.search_match_positions = vec![(5, 0), (15, 0), (25, 0)];
        app.search_match_current = Some(0);
        let changed = app.jump_to_next_match_cross_session();
        assert!(!changed);
        assert_eq!(app.search_match_current, Some(1));
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_jump_to_next_match_cross_session_stays_when_current_none() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.search_query = "test".into();
        app.search_match_positions = vec![(5, 0), (15, 0)];
        // search_match_current is None (first press)
        let changed = app.jump_to_next_match_cross_session();
        assert!(!changed);
        assert_eq!(app.search_match_current, Some(0)); // jumps to first
    }

    #[test]
    fn test_jump_to_next_match_cross_session_moves_session_at_last_match() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.search_query = "test".into();
        app.search_match_positions = vec![(5, 0), (15, 0)];
        app.search_match_current = Some(1); // at last match
        let changed = app.jump_to_next_match_cross_session();
        assert!(changed);
        assert_eq!(app.selected_index, 1);
        assert_eq!(app.pending_search_jump, Some(SearchJumpDirection::First));
        assert_eq!(app.pending_search_jump_origin, Some(0));
        assert_eq!(app.search_match_current, None);
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn test_jump_to_next_match_cross_session_no_matches() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.search_query = "test".into();
        // No match positions in current conversation
        let changed = app.jump_to_next_match_cross_session();
        assert!(changed);
        assert_eq!(app.selected_index, 1);
        assert_eq!(app.pending_search_jump, Some(SearchJumpDirection::First));
    }

    #[test]
    fn test_jump_to_next_match_cross_session_wraps_around() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Viewing;
        app.search_query = "test".into();
        app.selected_index = 2; // last session
        app.sync_list_state();
        let changed = app.jump_to_next_match_cross_session();
        assert!(changed);
        assert_eq!(app.selected_index, 0); // wrapped to first
    }

    #[test]
    fn test_jump_to_next_match_cross_session_single_session_wraps_within() {
        let mut app = AppState::new(make_sessions(1));
        app.mode = AppMode::Viewing;
        app.search_query = "test".into();
        app.search_match_positions = vec![(5, 0), (15, 0)];
        app.search_match_current = Some(1); // at last match
        let changed = app.jump_to_next_match_cross_session();
        // Only one session: wrap within conversation
        assert!(!changed);
        assert_eq!(app.search_match_current, Some(0));
    }

    #[test]
    fn test_jump_to_prev_match_cross_session_stays_when_not_at_first() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.search_query = "test".into();
        app.search_match_positions = vec![(5, 0), (15, 0), (25, 0)];
        app.search_match_current = Some(2);
        let changed = app.jump_to_prev_match_cross_session();
        assert!(!changed);
        assert_eq!(app.search_match_current, Some(1));
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_jump_to_prev_match_cross_session_stays_when_current_none() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.search_query = "test".into();
        app.search_match_positions = vec![(5, 0), (15, 0)];
        // search_match_current is None → jumps to last in current session
        let changed = app.jump_to_prev_match_cross_session();
        assert!(!changed);
        assert_eq!(app.search_match_current, Some(1)); // last match
    }

    #[test]
    fn test_jump_to_prev_match_cross_session_moves_session_at_first_match() {
        let mut app = AppState::new(make_sessions(5));
        app.mode = AppMode::Viewing;
        app.search_query = "test".into();
        app.selected_index = 2;
        app.sync_list_state();
        app.search_match_positions = vec![(5, 0), (15, 0)];
        app.search_match_current = Some(0); // at first match
        let changed = app.jump_to_prev_match_cross_session();
        assert!(changed);
        assert_eq!(app.selected_index, 1);
        assert_eq!(app.pending_search_jump, Some(SearchJumpDirection::Last));
        assert_eq!(app.pending_search_jump_origin, Some(2));
    }

    #[test]
    fn test_jump_to_prev_match_cross_session_wraps_around() {
        let mut app = AppState::new(make_sessions(3));
        app.mode = AppMode::Viewing;
        app.search_query = "test".into();
        app.selected_index = 0; // first session
        app.search_match_positions = vec![(5, 0)];
        app.search_match_current = Some(0);
        let changed = app.jump_to_prev_match_cross_session();
        assert!(changed);
        assert_eq!(app.selected_index, 2); // wrapped to last
    }

    #[test]
    fn test_resolve_pending_search_jump_none() {
        let mut app = AppState::new(make_sessions(3));
        let needs_more = app.resolve_pending_search_jump();
        assert!(!needs_more);
    }

    #[test]
    fn test_resolve_pending_search_jump_first() {
        let mut app = AppState::new(make_sessions(3));
        app.pending_search_jump = Some(SearchJumpDirection::First);
        app.pending_search_jump_origin = Some(0);
        app.search_match_positions = vec![(10, 0), (20, 0)];
        let needs_more = app.resolve_pending_search_jump();
        assert!(!needs_more);
        assert_eq!(app.search_match_current, Some(0));
        assert_eq!(app.conversation_scroll, 5); // 10 - 5
        assert_eq!(app.pending_search_jump, None);
        assert_eq!(app.pending_search_jump_origin, None);
    }

    #[test]
    fn test_resolve_pending_search_jump_last() {
        let mut app = AppState::new(make_sessions(3));
        app.pending_search_jump = Some(SearchJumpDirection::Last);
        app.pending_search_jump_origin = Some(2);
        app.search_match_positions = vec![(10, 0), (20, 0), (30, 0)];
        let needs_more = app.resolve_pending_search_jump();
        assert!(!needs_more);
        assert_eq!(app.search_match_current, Some(2)); // last
        assert_eq!(app.conversation_scroll, 25); // 30 - 5
        assert_eq!(app.pending_search_jump, None);
    }

    #[test]
    fn test_resolve_pending_search_jump_no_matches_advances_forward() {
        let mut app = AppState::new(make_sessions(5));
        app.selected_index = 1;
        app.sync_list_state();
        app.pending_search_jump = Some(SearchJumpDirection::First);
        app.pending_search_jump_origin = Some(0);
        // No matches in this session
        let needs_more = app.resolve_pending_search_jump();
        assert!(needs_more);
        assert_eq!(app.selected_index, 2); // advanced
        assert_eq!(app.pending_search_jump, Some(SearchJumpDirection::First));
    }

    #[test]
    fn test_resolve_pending_search_jump_no_matches_advances_backward() {
        let mut app = AppState::new(make_sessions(5));
        app.selected_index = 3;
        app.sync_list_state();
        app.pending_search_jump = Some(SearchJumpDirection::Last);
        app.pending_search_jump_origin = Some(4);
        // No matches in this session
        let needs_more = app.resolve_pending_search_jump();
        assert!(needs_more);
        assert_eq!(app.selected_index, 2); // retreated
    }

    #[test]
    fn test_resolve_pending_search_jump_full_cycle_stops() {
        let mut app = AppState::new(make_sessions(3));
        app.selected_index = 0;
        app.sync_list_state();
        app.pending_search_jump = Some(SearchJumpDirection::First);
        app.pending_search_jump_origin = Some(0); // started here
        // No matches, and we've returned to origin
        let needs_more = app.resolve_pending_search_jump();
        // Should detect we're back at origin and stop
        assert!(!needs_more);
        assert_eq!(app.pending_search_jump, None);
    }

    #[test]
    fn test_request_reload_conversation_sets_flag() {
        let mut app = AppState::new(make_sessions(3));
        assert!(!app.conversation_reloading);
        app.request_reload_conversation();
        assert!(app.conversation_reloading);
        assert!(app.conversation_reload_at.is_some());
    }

    #[test]
    fn test_request_reload_conversation_clears_loaded_session_id() {
        let mut app = AppState::new(make_sessions(3));
        app.loaded_session_id = Some("sess-0".into());
        app.request_reload_conversation();
        assert_eq!(app.loaded_session_id, None);
    }

    #[test]
    fn test_check_reload_expired_clears_flag() {
        let mut app = AppState::new(make_sessions(3));
        app.conversation_reloading = true;
        app.conversation_reload_at =
            Some(std::time::Instant::now() - std::time::Duration::from_millis(600));
        app.check_reload_expired();
        assert!(!app.conversation_reloading);
        assert!(app.conversation_reload_at.is_none());
    }

    #[test]
    fn test_check_reload_not_expired_keeps_flag() {
        let mut app = AppState::new(make_sessions(3));
        app.conversation_reloading = true;
        app.conversation_reload_at = Some(std::time::Instant::now());
        app.check_reload_expired();
        assert!(app.conversation_reloading);
    }
}
