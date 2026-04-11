use crate::session::SessionIndex;
use chrono::NaiveDate;

/// Filter sessions by substring matching against conversation content.
/// Only matches sessions where the query appears as a contiguous substring
/// in the displayed conversation text, ensuring the match is always visible
/// in the conversation view.
pub fn fuzzy_filter(
    sessions: &[SessionIndex],
    query: &str,
    content_cache: &[String],
) -> Vec<usize> {
    if query.is_empty() {
        return (0..sessions.len()).collect();
    }
    let query_lower = query.to_lowercase();
    sessions
        .iter()
        .enumerate()
        .filter(|(i, session)| {
            // Try content cache first (already lowercased at build time)
            if let Some(cached) = content_cache.get(*i)
                && !cached.is_empty()
            {
                return cached.contains(&query_lower);
            }
            // Fallback: match against session metadata
            session.first_prompt.to_lowercase().contains(&query_lower)
                || session
                    .project_display
                    .to_lowercase()
                    .contains(&query_lower)
                || session
                    .summary
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&query_lower)
                || session
                    .git_branch
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&query_lower)
        })
        .map(|(i, _)| i)
        .collect()
}

/// Filter sessions by date range (inclusive).
/// Compares against the session's modified date.
#[cfg(test)]
pub fn date_filter(
    sessions: &[SessionIndex],
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Vec<usize> {
    if from.is_none() && to.is_none() {
        return (0..sessions.len()).collect();
    }
    sessions
        .iter()
        .enumerate()
        .filter(|(_, session)| {
            let session_date = session.modified.date_naive();
            if let Some(from_date) = from
                && session_date < from_date
            {
                return false;
            }
            if let Some(to_date) = to
                && session_date > to_date
            {
                return false;
            }
            true
        })
        .map(|(i, _)| i)
        .collect()
}

/// Apply both fuzzy search and date range filters in a single pass.
pub fn apply_filters(
    sessions: &[SessionIndex],
    query: &str,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    content_cache: &[String],
) -> Vec<usize> {
    let no_query = query.is_empty();
    let no_date = from.is_none() && to.is_none();

    if no_query && no_date {
        return (0..sessions.len()).collect();
    }

    let query_lower = query.to_lowercase();

    sessions
        .iter()
        .enumerate()
        .filter(|(i, session)| {
            // Date filter
            if !no_date {
                let session_date = session.modified.date_naive();
                if let Some(from_date) = from
                    && session_date < from_date
                {
                    return false;
                }
                if let Some(to_date) = to
                    && session_date > to_date
                {
                    return false;
                }
            }

            // Fuzzy filter
            if !no_query {
                if let Some(cached) = content_cache.get(*i)
                    && !cached.is_empty()
                {
                    return cached.contains(query_lower.as_str());
                }
                return session.first_prompt.to_lowercase().contains(&query_lower)
                    || session
                        .project_display
                        .to_lowercase()
                        .contains(&query_lower)
                    || session
                        .summary
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query_lower)
                    || session
                        .git_branch
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query_lower);
            }

            true
        })
        .map(|(i, _)| i)
        .collect()
}

/// Count total occurrences of a search query across all cached session content.
/// Both the query and cache entries are compared case-insensitively
/// (cache is already pre-lowercased at build time).
pub fn count_total_search_matches(query: &str, cache: &[String]) -> usize {
    if query.is_empty() {
        return 0;
    }
    let query_lower = query.to_lowercase();
    cache
        .iter()
        .map(|entry| {
            let mut count = 0;
            let mut start = 0;
            while let Some(pos) = entry[start..].find(&query_lower) {
                count += 1;
                start += pos + query_lower.len();
            }
            count
        })
        .sum()
}

/// Parse a date string in YYYY-MM-DD format.
pub fn parse_date_input(input: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(input.trim(), "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{self, SessionIndex};
    use chrono::Utc;
    use std::path::PathBuf;

    fn make_session(
        id: &str,
        project: &str,
        prompt: &str,
        branch: Option<&str>,
        date: &str,
    ) -> SessionIndex {
        let dt = date
            .parse::<chrono::DateTime<Utc>>()
            .unwrap_or_else(|_| Utc::now());
        SessionIndex {
            session_id: id.into(),
            project_path: format!("/test/{project}"),
            project_display: project.into(),
            first_prompt: prompt.into(),
            summary: None,
            created: dt,
            modified: dt,
            git_branch: branch.map(Into::into),
            message_count: 10,
            file_path: PathBuf::from(format!("/tmp/{id}.jsonl")),
            date_display: String::new(),
            branch_display: String::new(),
            prompt_preview: String::new(),
        }
        .with_display_fields()
    }

    fn sample_sessions() -> Vec<SessionIndex> {
        vec![
            make_session(
                "s1",
                "terraform-infra",
                "Run terraform plan",
                Some("main"),
                "2026-04-01T10:00:00Z",
            ),
            make_session(
                "s2",
                "web-app",
                "Fix login bug",
                Some("feature/auth"),
                "2026-04-05T10:00:00Z",
            ),
            make_session(
                "s3",
                "api-server",
                "Add health check endpoint",
                Some("develop"),
                "2026-04-08T10:00:00Z",
            ),
            make_session(
                "s4",
                "terraform-infra",
                "Update VPC configuration",
                Some("feature/vpc"),
                "2026-03-20T10:00:00Z",
            ),
        ]
    }

    // --- fuzzy_filter tests (substring match against content cache) ---

    /// Returns pre-lowercased content cache (as built by the background thread).
    fn sample_caches() -> Vec<String> {
        vec![
            "run terraform plan for vpc setup".into(),        // s1
            "fix login bug in authentication flow".into(),    // s2
            "add health check endpoint to api".into(),        // s3
            "update vpc configuration with terraform".into(), // s4
        ]
    }

    #[test]
    fn test_fuzzy_filter_by_content() {
        let sessions = sample_sessions();
        let cache = sample_caches();
        let result = fuzzy_filter(&sessions, "terraform", &cache);
        assert_eq!(result, vec![0, 3]);
    }

    #[test]
    fn test_fuzzy_filter_by_content_partial() {
        let sessions = sample_sessions();
        let cache = sample_caches();
        let result = fuzzy_filter(&sessions, "login", &cache);
        assert_eq!(result, vec![1]);
    }

    #[test]
    fn test_fuzzy_filter_empty_query_returns_all() {
        let sessions = sample_sessions();
        let result = fuzzy_filter(&sessions, "", &[]);
        assert_eq!(result, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_fuzzy_filter_case_insensitive() {
        let sessions = sample_sessions();
        let cache = sample_caches();
        let result = fuzzy_filter(&sessions, "TERRAFORM", &cache);
        assert_eq!(result, vec![0, 3]);
    }

    #[test]
    fn test_fuzzy_filter_no_match() {
        let sessions = sample_sessions();
        let cache = sample_caches();
        let result = fuzzy_filter(&sessions, "zzzzz", &cache);
        assert!(result.is_empty());
    }

    #[test]
    fn test_fuzzy_filter_empty_cache_falls_back_to_metadata() {
        let sessions = sample_sessions();
        // Without cache, search falls back to metadata (first_prompt contains "terraform")
        // s1: "Run terraform plan" (first_prompt), s4: "terraform-infra" (project_display)
        let result = fuzzy_filter(&sessions, "terraform", &[]);
        assert_eq!(result, vec![0, 3]);
    }

    #[test]
    fn test_fuzzy_filter_metadata_matches_project_display() {
        let sessions = sample_sessions();
        // "web-app" matches s2's project_display
        let result = fuzzy_filter(&sessions, "web-app", &[]);
        assert_eq!(result, vec![1]);
    }

    #[test]
    fn test_fuzzy_filter_metadata_matches_git_branch() {
        let sessions = sample_sessions();
        // "feature/auth" matches s2's git_branch
        let result = fuzzy_filter(&sessions, "feature/auth", &[]);
        assert_eq!(result, vec![1]);
    }

    #[test]
    fn test_fuzzy_filter_metadata_matches_summary() {
        let sessions = sample_sessions();
        // Add a session with summary
        let mut sessions_with_summary = sessions;
        sessions_with_summary[2] = make_session(
            "s3",
            "api-server",
            "Add health check endpoint",
            Some("develop"),
            "2026-04-08T10:00:00Z",
        );
        // Manually set summary by creating a new session
        let mut s = make_session(
            "s3",
            "api-server",
            "unrelated prompt",
            Some("develop"),
            "2026-04-08T10:00:00Z",
        );
        s.summary = Some("health check implementation".into());
        sessions_with_summary[2] = s;
        let result = fuzzy_filter(&sessions_with_summary, "health check implementation", &[]);
        assert_eq!(result, vec![2]);
    }

    #[test]
    fn test_fuzzy_filter_content_cache_takes_precedence() {
        let sessions = sample_sessions();
        // Cache says "unrelated content" for s1, even though metadata has "terraform"
        let cache = vec![
            "unrelated content".into(),
            "unrelated content".into(),
            "unrelated content".into(),
            "unrelated content".into(),
        ];
        // "terraform" is in s1/s4 metadata but NOT in cache content
        let result = fuzzy_filter(&sessions, "terraform", &cache);
        assert!(result.is_empty());
    }

    // --- date_filter tests ---

    #[test]
    fn test_date_filter_from_only() {
        let sessions = sample_sessions();
        let from = NaiveDate::from_ymd_opt(2026, 4, 1);
        let result = date_filter(&sessions, from, None);
        assert_eq!(result, vec![0, 1, 2]); // excludes s4 (March 20)
    }

    #[test]
    fn test_date_filter_to_only() {
        let sessions = sample_sessions();
        let to = NaiveDate::from_ymd_opt(2026, 4, 5);
        let result = date_filter(&sessions, None, to);
        assert_eq!(result, vec![0, 1, 3]); // excludes s3 (April 8)
    }

    #[test]
    fn test_date_filter_range() {
        let sessions = sample_sessions();
        let from = NaiveDate::from_ymd_opt(2026, 4, 1);
        let to = NaiveDate::from_ymd_opt(2026, 4, 5);
        let result = date_filter(&sessions, from, to);
        assert_eq!(result, vec![0, 1]); // April 1 and April 5
    }

    #[test]
    fn test_date_filter_no_constraints() {
        let sessions = sample_sessions();
        let result = date_filter(&sessions, None, None);
        assert_eq!(result, vec![0, 1, 2, 3]);
    }

    // --- apply_filters tests ---

    #[test]
    fn test_apply_filters_combined() {
        let sessions = sample_sessions();
        let cache = sample_caches();
        let from = NaiveDate::from_ymd_opt(2026, 4, 1);
        let result = apply_filters(&sessions, "terraform", from, None, &cache);
        assert_eq!(result, vec![0]); // only s1 matches both terraform + after April 1
    }

    #[test]
    fn test_apply_filters_empty_query_with_date() {
        let sessions = sample_sessions();
        let from = NaiveDate::from_ymd_opt(2026, 4, 5);
        let result = apply_filters(&sessions, "", from, None, &[]);
        assert_eq!(result, vec![1, 2]); // April 5 and April 8
    }

    // --- parse_date_input tests ---

    #[test]
    fn test_parse_date_input_valid() {
        let date = parse_date_input("2026-04-08");
        assert_eq!(date, NaiveDate::from_ymd_opt(2026, 4, 8));
    }

    #[test]
    fn test_parse_date_input_invalid() {
        assert!(parse_date_input("not-a-date").is_none());
        assert!(parse_date_input("").is_none());
    }

    #[test]
    fn test_parse_date_input_trimmed() {
        let date = parse_date_input("  2026-04-08  ");
        assert_eq!(date, NaiveDate::from_ymd_opt(2026, 4, 8));
    }

    // --- conversation content search tests ---

    #[test]
    fn test_fuzzy_filter_matches_conversation_content() {
        use std::fs;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("content-search.jsonl");

        // Session metadata does NOT contain "実行日", but conversation content does
        let jsonl = r#"{"type":"user","uuid":"u1","parentUuid":null,"isSidechain":false,"message":{"role":"user","content":"設定を確認して"},"timestamp":"2026-04-08T10:00:00Z"}
{"type":"assistant","uuid":"a1","parentUuid":"u1","isSidechain":false,"message":{"role":"assistant","content":[{"type":"text","text":"実行日は2026-04-08です"}]},"timestamp":"2026-04-08T10:00:01Z"}"#;
        fs::write(&file_path, jsonl).unwrap();

        let sessions = vec![SessionIndex {
            session_id: "s1".into(),
            project_path: "/test/myproject".into(),
            project_display: "myproject".into(),
            first_prompt: "設定を確認して".into(),
            summary: None,
            created: Utc::now(),
            modified: Utc::now(),
            git_branch: Some("main".into()),
            message_count: 2,
            file_path,
            date_display: String::new(),
            branch_display: String::new(),
            prompt_preview: String::new(),
        }];

        // "実行日" is only in the assistant's response, not in metadata
        let cache = vec![session::extract_searchable_text(&sessions[0].file_path)];
        let result = fuzzy_filter(&sessions, "実行日", &cache);
        assert_eq!(result, vec![0]);
    }

    #[test]
    fn test_fuzzy_filter_without_cache_uses_metadata() {
        // Without cache, non-empty query falls back to metadata
        let sessions = sample_sessions();
        // "terraform" appears in first_prompt of s1 ("Run terraform plan")
        let result = fuzzy_filter(&sessions, "terraform", &[]);
        assert!(!result.is_empty());
        assert!(result.contains(&0)); // s1
    }

    #[test]
    fn test_fuzzy_filter_no_match_in_content() {
        use std::fs;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("no-match.jsonl");

        let jsonl = r#"{"type":"user","uuid":"u1","parentUuid":null,"isSidechain":false,"message":{"role":"user","content":"Hello"},"timestamp":"2026-04-08T10:00:00Z"}"#;
        fs::write(&file_path, jsonl).unwrap();

        let sessions = vec![SessionIndex {
            session_id: "s1".into(),
            project_path: "/test/proj".into(),
            project_display: "proj".into(),
            first_prompt: "Hello".into(),
            summary: None,
            created: Utc::now(),
            modified: Utc::now(),
            git_branch: None,
            message_count: 1,
            file_path,
            date_display: String::new(),
            branch_display: String::new(),
            prompt_preview: String::new(),
        }];

        let cache = vec![session::extract_searchable_text(&sessions[0].file_path)];
        let result = fuzzy_filter(&sessions, "zzzzz", &cache);
        assert!(result.is_empty());
    }

    #[test]
    fn test_fuzzy_filter_cache_is_pre_lowercased() {
        let sessions = sample_sessions();
        // Cache content is already lowercased (as built by the background thread)
        let cache = vec![
            "run terraform plan for vpc setup".into(),
            "fix login bug in authentication flow".into(),
            "add health check endpoint to api".into(),
            "update vpc configuration with terraform".into(),
        ];
        // Case-insensitive query should still match pre-lowercased cache
        let result = fuzzy_filter(&sessions, "TERRAFORM", &cache);
        assert_eq!(result, vec![0, 3]);
    }

    // --- count_total_search_matches tests ---

    #[test]
    fn test_count_total_search_matches_basic() {
        let cache = sample_caches();
        // "terraform" appears once in s1 and once in s4 → total 2
        let count = count_total_search_matches("terraform", &cache);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_count_total_search_matches_multiple_in_one_entry() {
        let cache = vec!["terraform plan terraform apply terraform destroy".into()];
        let count = count_total_search_matches("terraform", &cache);
        assert_eq!(count, 3);
    }

    #[test]
    fn test_count_total_search_matches_empty_query() {
        let cache = sample_caches();
        let count = count_total_search_matches("", &cache);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_count_total_search_matches_no_match() {
        let cache = sample_caches();
        let count = count_total_search_matches("zzzzz", &cache);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_count_total_search_matches_case_insensitive() {
        let cache = sample_caches();
        let count = count_total_search_matches("TERRAFORM", &cache);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_count_total_search_matches_empty_cache() {
        let count = count_total_search_matches("terraform", &[]);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_content_search_uses_substring_not_fuzzy() {
        use std::fs;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("substring-test.jsonl");

        // Contains 実, 行, 日 as scattered characters but NOT "実行日" as substring
        let jsonl = r#"{"type":"user","uuid":"u1","parentUuid":null,"isSidechain":false,"message":{"role":"user","content":"実装を行った日のログ"},"timestamp":"2026-04-08T10:00:00Z"}"#;
        fs::write(&file_path, jsonl).unwrap();

        let sessions = vec![SessionIndex {
            session_id: "s1".into(),
            project_path: "/test/proj".into(),
            project_display: "proj".into(),
            first_prompt: "テスト".into(),
            summary: None,
            created: Utc::now(),
            modified: Utc::now(),
            git_branch: None,
            message_count: 1,
            file_path,
            date_display: String::new(),
            branch_display: String::new(),
            prompt_preview: String::new(),
        }];

        // "実行日" should NOT match "実装を行った日のログ" via substring
        let cache = vec![session::extract_searchable_text(&sessions[0].file_path)];
        let result = fuzzy_filter(&sessions, "実行日", &cache);
        assert!(result.is_empty());
    }
}
