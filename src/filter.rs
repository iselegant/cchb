use crate::session::SessionIndex;
use chrono::NaiveDate;

/// Build a searchable string from a session's metadata.
fn session_search_text(session: &SessionIndex) -> String {
    let mut text = String::new();
    text.push_str(&session.project_display);
    text.push(' ');
    text.push_str(&session.first_prompt);
    if let Some(ref branch) = session.git_branch {
        text.push(' ');
        text.push_str(branch);
    }
    if let Some(ref summary) = session.summary {
        text.push(' ');
        text.push_str(summary);
    }
    text.to_lowercase()
}

/// Check if all characters of the query appear in order within the target (fuzzy match).
fn fuzzy_match(target: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let mut chars = query.chars();
    let mut current = chars.next();
    for c in target.chars() {
        if let Some(q) = current {
            if c == q {
                current = chars.next();
            }
        } else {
            break;
        }
    }
    current.is_none()
}

/// Filter sessions by fuzzy matching against a query string.
/// Matches against project name, first prompt, git branch, and summary.
pub fn fuzzy_filter(sessions: &[SessionIndex], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..sessions.len()).collect();
    }
    let query_lower = query.to_lowercase();
    sessions
        .iter()
        .enumerate()
        .filter(|(_, session)| {
            let text = session_search_text(session);
            fuzzy_match(&text, &query_lower)
        })
        .map(|(i, _)| i)
        .collect()
}

/// Filter sessions by date range (inclusive).
/// Compares against the session's modified date.
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

/// Apply both fuzzy search and date range filters, returning matching indices.
/// The result is the intersection of both filters.
pub fn apply_filters(
    sessions: &[SessionIndex],
    query: &str,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Vec<usize> {
    let fuzzy_results = fuzzy_filter(sessions, query);
    let date_results = date_filter(sessions, from, to);

    // Intersect both results
    let date_set: std::collections::HashSet<usize> = date_results.into_iter().collect();
    fuzzy_results
        .into_iter()
        .filter(|i| date_set.contains(i))
        .collect()
}

/// Parse a date string in YYYY-MM-DD format.
pub fn parse_date_input(input: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(input.trim(), "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionIndex;
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
        }
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

    // --- fuzzy_match tests ---

    #[test]
    fn test_fuzzy_match_exact() {
        assert!(fuzzy_match("terraform", "terraform"));
    }

    #[test]
    fn test_fuzzy_match_subsequence() {
        assert!(fuzzy_match("terraform infra", "tfi"));
    }

    #[test]
    fn test_fuzzy_match_no_match() {
        assert!(!fuzzy_match("terraform", "xyz"));
    }

    #[test]
    fn test_fuzzy_match_empty_query() {
        assert!(fuzzy_match("anything", ""));
    }

    #[test]
    fn test_fuzzy_match_empty_target() {
        assert!(!fuzzy_match("", "abc"));
    }

    // --- fuzzy_filter tests ---

    #[test]
    fn test_fuzzy_filter_by_prompt() {
        let sessions = sample_sessions();
        let result = fuzzy_filter(&sessions, "terraform");
        assert_eq!(result, vec![0, 3]); // both terraform-infra sessions
    }

    #[test]
    fn test_fuzzy_filter_by_project() {
        let sessions = sample_sessions();
        let result = fuzzy_filter(&sessions, "web");
        assert_eq!(result, vec![1]);
    }

    #[test]
    fn test_fuzzy_filter_by_branch() {
        let sessions = sample_sessions();
        let result = fuzzy_filter(&sessions, "auth");
        assert_eq!(result, vec![1]);
    }

    #[test]
    fn test_fuzzy_filter_empty_query_returns_all() {
        let sessions = sample_sessions();
        let result = fuzzy_filter(&sessions, "");
        assert_eq!(result, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_fuzzy_filter_case_insensitive() {
        let sessions = sample_sessions();
        let result = fuzzy_filter(&sessions, "TERRAFORM");
        assert_eq!(result, vec![0, 3]);
    }

    #[test]
    fn test_fuzzy_filter_no_match() {
        let sessions = sample_sessions();
        let result = fuzzy_filter(&sessions, "zzzzz");
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
        let from = NaiveDate::from_ymd_opt(2026, 4, 1);
        let result = apply_filters(&sessions, "terraform", from, None);
        assert_eq!(result, vec![0]); // only s1 matches both terraform + after April 1
    }

    #[test]
    fn test_apply_filters_empty_query_with_date() {
        let sessions = sample_sessions();
        let from = NaiveDate::from_ymd_opt(2026, 4, 5);
        let result = apply_filters(&sessions, "", from, None);
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
}
