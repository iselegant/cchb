use cchb::app::AppState;
use cchb::filter;
use cchb::session::{self, SessionIndex};
use chrono::{NaiveDate, Utc};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

/// Create a minimal Claude Code directory structure with sessions.
fn create_test_claude_dir(sessions: &[(&str, &str, &str)]) -> TempDir {
    let dir = TempDir::new().unwrap();
    let projects_dir = dir.path().join("projects");
    fs::create_dir_all(&projects_dir).unwrap();

    let project_dir = projects_dir.join("-Users-test-myproject");
    fs::create_dir_all(&project_dir).unwrap();

    for (session_id, prompt, response) in sessions {
        let jsonl_path = project_dir.join(format!("{session_id}.jsonl"));
        let mut f = fs::File::create(&jsonl_path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"{prompt}"}},"uuid":"u-{session_id}","timestamp":"2025-01-15T10:00:00Z"}}"#,
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"{response}"}}]}},"uuid":"a-{session_id}","timestamp":"2025-01-15T10:01:00Z"}}"#,
        )
        .unwrap();
    }

    dir
}

#[test]
fn test_discover_and_load_conversation() {
    let dir = create_test_claude_dir(&[("sess-001", "Hello Claude", "Hi there!")]);

    let sessions = session::discover_sessions(dir.path()).unwrap();
    assert!(!sessions.is_empty());

    let sess = &sessions[0];
    assert!(sess.file_path.exists());

    let messages = session::load_conversation(&sess.file_path).unwrap();
    let display = session::display_messages(messages);
    assert_eq!(display.len(), 2);
    assert_eq!(display[0].role, "user");
    assert_eq!(display[1].role, "assistant");
}

#[test]
fn test_discover_multiple_sessions() {
    let dir = create_test_claude_dir(&[
        ("sess-001", "First session", "Response 1"),
        ("sess-002", "Second session", "Response 2"),
    ]);

    let sessions = session::discover_sessions(dir.path()).unwrap();
    assert_eq!(sessions.len(), 2);

    let ids: Vec<&str> = sessions.iter().map(|s| s.session_id.as_str()).collect();
    assert!(ids.contains(&"sess-001"));
    assert!(ids.contains(&"sess-002"));
}

#[test]
fn test_search_filter_workflow() {
    let dir = create_test_claude_dir(&[
        ("sess-001", "Rust programming help", "Sure, Rust is great"),
        ("sess-002", "Python scripting", "Python is versatile"),
        ("sess-003", "Rust async patterns", "Use tokio for async"),
    ]);

    let sessions = session::discover_sessions(dir.path()).unwrap();
    assert_eq!(sessions.len(), 3);

    // Build a content cache as Vec<String> indexed by position
    let mut cache: Vec<String> = Vec::new();
    for sess in &sessions {
        let text = session::extract_searchable_text(&sess.file_path);
        cache.push(text.to_lowercase());
    }

    // Search for "Rust" should return 2 sessions
    let filtered = filter::fuzzy_filter(&sessions, "Rust", &cache);
    assert_eq!(filtered.len(), 2);

    // Search for "Python" should return 1 session
    let filtered = filter::fuzzy_filter(&sessions, "Python", &cache);
    assert_eq!(filtered.len(), 1);

    // Search for "nonexistent" should return 0 sessions
    let filtered = filter::fuzzy_filter(&sessions, "nonexistent", &cache);
    assert!(filtered.is_empty());
}

#[test]
fn test_apply_filters_with_date_and_search() {
    let now = Utc::now();
    let sessions = vec![
        SessionIndex {
            session_id: "s1".into(),
            project_path: "/p".into(),
            project_display: "p".into(),
            first_prompt: "Rust help".into(),
            summary: None,
            created: now,
            modified: now,
            git_branch: None,
            message_count: 1,
            file_path: PathBuf::from("/tmp/s1.jsonl"),
            date_display: String::new(),
            branch_display: String::new(),
            prompt_preview: String::new(),
        }
        .with_display_fields(),
        SessionIndex {
            session_id: "s2".into(),
            project_path: "/p".into(),
            project_display: "p".into(),
            first_prompt: "Python help".into(),
            summary: None,
            created: now,
            modified: now,
            git_branch: None,
            message_count: 1,
            file_path: PathBuf::from("/tmp/s2.jsonl"),
            date_display: String::new(),
            branch_display: String::new(),
            prompt_preview: String::new(),
        }
        .with_display_fields(),
    ];

    let today = now.date_naive();
    let cache: Vec<String> = vec![String::new(); sessions.len()];

    // Combined: search "Rust" + today's date → 1 result (metadata fallback)
    let filtered = filter::apply_filters(&sessions, "Rust", Some(today), Some(today), &cache);
    assert_eq!(filtered.len(), 1);

    // Combined: search "help" + today's date → 2 results
    let filtered = filter::apply_filters(&sessions, "help", Some(today), Some(today), &cache);
    assert_eq!(filtered.len(), 2);

    // Combined: search "Rust" + past date → 0 results
    let past_from = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
    let past_to = NaiveDate::from_ymd_opt(2020, 12, 31).unwrap();
    let filtered = filter::apply_filters(&sessions, "Rust", Some(past_from), Some(past_to), &cache);
    assert!(filtered.is_empty());
}

#[test]
fn test_app_state_discovery_to_view_workflow() {
    let dir =
        create_test_claude_dir(&[("sess-001", "Hello", "Hi"), ("sess-002", "World", "Earth")]);

    let sessions = session::discover_sessions(dir.path()).unwrap();
    let mut app = AppState::new(sessions);

    // Initial state
    assert_eq!(app.selected_index, 0);
    assert_eq!(app.filtered_indices.len(), 2);

    // Load conversation for first session
    let changed = app.maybe_load_focused_conversation();
    assert!(changed);
    assert!(!app.conversation.is_empty());

    // Navigate to next session
    app.select_next();
    let changed = app.maybe_load_focused_conversation();
    assert!(changed);
    assert!(!app.conversation.is_empty());

    // Going back triggers reload (different session)
    app.select_prev();
    let changed = app.maybe_load_focused_conversation();
    assert!(changed);
}

#[test]
fn test_empty_claude_dir() {
    let dir = TempDir::new().unwrap();
    let projects_dir = dir.path().join("projects");
    fs::create_dir_all(&projects_dir).unwrap();

    let sessions = session::discover_sessions(dir.path()).unwrap();
    assert!(sessions.is_empty());
}

#[test]
fn test_search_filter_empty_query_returns_all() {
    let dir =
        create_test_claude_dir(&[("sess-001", "Hello", "Hi"), ("sess-002", "World", "Earth")]);

    let sessions = session::discover_sessions(dir.path()).unwrap();
    let cache: Vec<String> = vec![String::new(); sessions.len()];
    let filtered = filter::fuzzy_filter(&sessions, "", &cache);
    assert_eq!(filtered.len(), sessions.len());
}
