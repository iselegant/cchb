use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rayon::prelude::*;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Metadata for a session, used in the session list panel.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SessionIndex {
    pub session_id: String,
    pub project_path: String,
    pub project_display: String,
    pub first_prompt: String,
    pub summary: Option<String>,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    pub git_branch: Option<String>,
    pub message_count: usize,
    pub file_path: PathBuf,
    /// Pre-formatted date string for display ("YYYY-MM-DD HH:MM").
    pub date_display: String,
    /// Pre-formatted branch string for display ("(branch)"), or empty.
    pub branch_display: String,
    /// Truncated first prompt for display (max 60 chars).
    pub prompt_preview: String,
}

impl SessionIndex {
    /// Compute the display-only fields from the core fields.
    pub fn with_display_fields(mut self) -> Self {
        self.date_display = self.modified.format("%Y-%m-%d %H:%M").to_string();
        self.branch_display = self
            .git_branch
            .as_ref()
            .map(|b| format!("({b})"))
            .unwrap_or_default();
        self.prompt_preview = if self.first_prompt.is_empty() {
            "(no prompt)".to_string()
        } else {
            self.first_prompt.chars().take(60).collect()
        };
        self
    }
}

/// The type of a content block within an assistant message.
#[derive(Debug, Clone, PartialEq)]
pub enum ContentBlock {
    Text(String),
    Thinking(String),
    ToolUse { name: String },
    ToolResult(String),
}

/// A single conversation message extracted from a session JSONL file.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ConversationMessage {
    pub uuid: String,
    pub parent_uuid: Option<String>,
    pub role: String,
    pub content_blocks: Vec<ContentBlock>,
    pub timestamp: Option<DateTime<Utc>>,
    pub is_sidechain: bool,
}

// --- Raw JSON deserialization types ---

#[derive(Deserialize)]
struct RawMessage {
    #[serde(rename = "type")]
    msg_type: Option<String>,
    uuid: Option<String>,
    #[serde(rename = "parentUuid")]
    parent_uuid: Option<String>,
    #[serde(rename = "isSidechain")]
    is_sidechain: Option<bool>,
    message: Option<RawInnerMessage>,
    timestamp: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct RawInnerMessage {
    role: Option<String>,
    content: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct RawContentBlock {
    #[serde(rename = "type")]
    block_type: Option<String>,
    text: Option<String>,
    thinking: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct RawSessionsIndex {
    #[serde(rename = "originalPath")]
    #[allow(dead_code)]
    original_path: Option<String>,
    entries: Option<Vec<RawSessionEntry>>,
}

#[derive(Deserialize)]
struct RawSessionEntry {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    #[serde(rename = "fullPath")]
    full_path: Option<String>,
    #[serde(rename = "firstPrompt")]
    first_prompt: Option<String>,
    summary: Option<String>,
    #[serde(rename = "messageCount")]
    message_count: Option<usize>,
    created: Option<String>,
    modified: Option<String>,
    #[serde(rename = "gitBranch")]
    git_branch: Option<String>,
    #[serde(rename = "projectPath")]
    project_path: Option<String>,
    #[serde(rename = "isSidechain")]
    is_sidechain: Option<bool>,
}

/// Decode a dash-encoded project path back to the original path.
///
/// Example: `-Users-foo-Documents-project` → `/Users/foo/Documents/project`
pub fn decode_project_path(encoded: &str) -> String {
    if encoded.is_empty() {
        return String::new();
    }
    encoded.replacen('-', "/", 1).replace('-', "/")
}

/// Extract the short display name from a project path.
///
/// Example: `/Users/foo/Documents/my-project` → `my-project`
pub fn project_display_name(project_path: &str) -> String {
    project_path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(project_path)
        .to_string()
}

/// Parse a single JSONL line into a ConversationMessage, if it represents a user or assistant message.
fn parse_message_line(line: &str) -> Option<ConversationMessage> {
    let raw: RawMessage = serde_json::from_str(line).ok()?;

    let msg_type = raw.msg_type.as_deref()?;
    if msg_type != "user" && msg_type != "assistant" {
        return None;
    }

    let inner = raw.message?;
    let role = inner.role.unwrap_or_default();
    if role != "user" && role != "assistant" {
        return None;
    }

    let content_blocks = parse_content_blocks(&inner.content)?;

    let timestamp = parse_timestamp(&raw.timestamp);

    Some(ConversationMessage {
        uuid: raw.uuid.unwrap_or_default(),
        parent_uuid: raw.parent_uuid,
        role,
        content_blocks,
        timestamp,
        is_sidechain: raw.is_sidechain.unwrap_or(false),
    })
}

fn parse_content_blocks(content: &Option<serde_json::Value>) -> Option<Vec<ContentBlock>> {
    let content = content.as_ref()?;

    match content {
        serde_json::Value::String(s) => Some(vec![ContentBlock::Text(s.clone())]),
        serde_json::Value::Array(arr) => {
            let blocks: Vec<ContentBlock> = arr
                .iter()
                .filter_map(|v| {
                    let block: RawContentBlock = serde_json::from_value(v.clone()).ok()?;
                    match block.block_type.as_deref()? {
                        "text" => Some(ContentBlock::Text(block.text.unwrap_or_default())),
                        "thinking" => {
                            Some(ContentBlock::Thinking(block.thinking.unwrap_or_default()))
                        }
                        "tool_use" => Some(ContentBlock::ToolUse {
                            name: block.name.unwrap_or_default(),
                        }),
                        "tool_result" => {
                            Some(ContentBlock::ToolResult(block.text.unwrap_or_default()))
                        }
                        _ => None,
                    }
                })
                .collect();
            if blocks.is_empty() {
                None
            } else {
                Some(blocks)
            }
        }
        _ => None,
    }
}

/// Check if a first_prompt represents a meaningful user conversation.
///
/// Returns false for empty prompts, placeholder text ("No prompt"),
/// and error-only prompts (e.g., hook stderr output).
fn has_meaningful_prompt(first_prompt: &str) -> bool {
    let trimmed = first_prompt.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed == "No prompt" {
        return false;
    }
    if trimmed.starts_with("<local-command-stderr>") {
        return false;
    }
    true
}

fn parse_timestamp(value: &Option<serde_json::Value>) -> Option<DateTime<Utc>> {
    let v = value.as_ref()?;
    match v {
        serde_json::Value::String(s) => s.parse::<DateTime<Utc>>().ok(),
        serde_json::Value::Number(n) => {
            let millis = n.as_i64()?;
            DateTime::from_timestamp_millis(millis)
        }
        _ => None,
    }
}

/// Discover all sessions from the Claude Code data directory.
///
/// Scans `claude_dir/projects/` for project directories. Uses `sessions-index.json`
/// when available (fast path), falls back to scanning JSONL files directly.
pub fn discover_sessions(claude_dir: &Path) -> Result<Vec<SessionIndex>> {
    let projects_dir = claude_dir.join("projects");
    if !projects_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&projects_dir).with_context(|| {
        format!(
            "Failed to read projects directory: {}",
            projects_dir.display()
        )
    })?;

    // Collect directory entries first, then process in parallel.
    let dirs: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();

    let mut sessions: Vec<SessionIndex> = dirs
        .par_iter()
        .flat_map(|path| {
            let dir_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => return Vec::new(),
            };

            let project_path = decode_project_path(&dir_name);
            let project_display = project_display_name(&project_path);

            let mut dir_sessions = Vec::new();
            let mut seen_ids: HashSet<String> = HashSet::new();

            // Try sessions-index.json fast path
            let index_path = path.join("sessions-index.json");
            if index_path.exists()
                && let Ok(index_sessions) =
                    load_sessions_from_index(&index_path, &project_path, &project_display)
            {
                for s in &index_sessions {
                    seen_ids.insert(s.session_id.clone());
                }
                dir_sessions.extend(index_sessions);
            }

            // Always scan JSONL files to catch sessions not in the index
            if let Ok(scanned_sessions) =
                load_sessions_from_jsonl_scan(path, &project_path, &project_display)
            {
                for s in scanned_sessions {
                    if !seen_ids.contains(&s.session_id) {
                        dir_sessions.push(s);
                    }
                }
            }

            dir_sessions
        })
        .collect();

    sessions.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(sessions)
}

fn load_sessions_from_index(
    index_path: &Path,
    project_path: &str,
    project_display: &str,
) -> Result<Vec<SessionIndex>> {
    let content = fs::read_to_string(index_path)?;
    let index: RawSessionsIndex = serde_json::from_str(&content)?;

    let mut sessions = Vec::new();
    for entry in index.entries.unwrap_or_default() {
        if entry.is_sidechain.unwrap_or(false) {
            continue;
        }

        let session_id = match entry.session_id {
            Some(id) => id,
            None => continue,
        };

        let file_path = match entry.full_path {
            Some(p) => {
                let path = PathBuf::from(p);
                if !path.exists() {
                    continue;
                }
                path
            }
            None => continue,
        };

        let created = entry
            .created
            .as_deref()
            .and_then(|s| s.parse::<DateTime<Utc>>().ok())
            .unwrap_or_else(Utc::now);

        let modified = entry
            .modified
            .as_deref()
            .and_then(|s| s.parse::<DateTime<Utc>>().ok())
            .unwrap_or(created);

        let effective_project_path = entry
            .project_path
            .unwrap_or_else(|| project_path.to_string());

        let message_count = entry.message_count.unwrap_or(0);
        if message_count == 0 {
            continue;
        }

        let first_prompt = entry.first_prompt.unwrap_or_default();
        if !has_meaningful_prompt(&first_prompt) {
            continue;
        }

        sessions.push(
            SessionIndex {
                session_id,
                project_path: effective_project_path,
                project_display: project_display.to_string(),
                first_prompt,
                summary: entry.summary,
                created,
                modified,
                git_branch: entry.git_branch,
                message_count,
                file_path,
                date_display: String::new(),
                branch_display: String::new(),
                prompt_preview: String::new(),
            }
            .with_display_fields(),
        );
    }

    Ok(sessions)
}

fn load_sessions_from_jsonl_scan(
    project_dir: &Path,
    project_path: &str,
    _project_display: &str,
) -> Result<Vec<SessionIndex>> {
    let mut sessions = Vec::new();

    let entries = fs::read_dir(project_dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }

        let session_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut first_prompt = String::new();
        let mut first_timestamp: Option<DateTime<Utc>> = None;
        let mut last_timestamp: Option<DateTime<Utc>> = None;
        let mut git_branch: Option<String> = None;
        let mut cwd: Option<String> = None;
        let mut message_count = 0usize;

        for line in content.lines().take(50) {
            if let Ok(raw) = serde_json::from_str::<serde_json::Value>(line) {
                let msg_type = raw.get("type").and_then(|v| v.as_str());

                if msg_type == Some("user") || msg_type == Some("assistant") {
                    message_count += 1;
                }

                if msg_type == Some("user")
                    && first_prompt.is_empty()
                    && let Some(msg) = raw.get("message")
                    && let Some(content) = msg.get("content")
                {
                    if let Some(s) = content.as_str() {
                        first_prompt = s.chars().take(200).collect();
                    } else if let Some(arr) = content.as_array() {
                        for block in arr {
                            if block.get("type").and_then(|v| v.as_str()) == Some("text")
                                && let Some(text) = block.get("text").and_then(|v| v.as_str())
                            {
                                first_prompt = text.chars().take(200).collect();
                                break;
                            }
                        }
                    }
                }

                if let Some(ts) = raw.get("timestamp") {
                    let parsed = parse_timestamp(&Some(ts.clone()));
                    if first_timestamp.is_none() {
                        first_timestamp = parsed;
                    }
                    if parsed.is_some() {
                        last_timestamp = parsed;
                    }
                }

                if git_branch.is_none()
                    && let Some(branch) = raw.get("gitBranch").and_then(|v| v.as_str())
                {
                    git_branch = Some(branch.to_string());
                }

                if cwd.is_none()
                    && let Some(c) = raw.get("cwd").and_then(|v| v.as_str())
                {
                    cwd = Some(c.to_string());
                }
            }
        }

        let created = first_timestamp.unwrap_or_else(Utc::now);
        let modified = last_timestamp.unwrap_or(created);

        let effective_project_path = cwd.unwrap_or_else(|| project_path.to_string());

        if message_count == 0 {
            continue;
        }

        if !has_meaningful_prompt(&first_prompt) {
            continue;
        }

        sessions.push(
            SessionIndex {
                session_id,
                project_display: project_display_name(&effective_project_path),
                project_path: effective_project_path,
                first_prompt,
                summary: None,
                created,
                modified,
                git_branch,
                message_count,
                file_path: path,
                date_display: String::new(),
                branch_display: String::new(),
                prompt_preview: String::new(),
            }
            .with_display_fields(),
        );
    }

    Ok(sessions)
}

/// Extract all text content from a session JSONL file as a single searchable string.
/// This is a lightweight alternative to `load_conversation()` that avoids building
/// full `ConversationMessage` structs — it only extracts text for search purposes.
pub fn extract_searchable_text(path: &Path) -> String {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };

    let mut texts = Vec::new();
    for line in content.lines() {
        if let Ok(raw) = serde_json::from_str::<serde_json::Value>(line) {
            let msg_type = raw.get("type").and_then(|v| v.as_str());
            if msg_type != Some("user") && msg_type != Some("assistant") {
                continue;
            }
            // Skip sidechains — they are not shown in conversation view
            if raw
                .get("isSidechain")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                continue;
            }
            if let Some(msg) = raw.get("message")
                && let Some(content) = msg.get("content")
            {
                if let Some(s) = content.as_str() {
                    texts.push(s.to_string());
                } else if let Some(arr) = content.as_array() {
                    for block in arr {
                        if block.get("type").and_then(|v| v.as_str()) == Some("text")
                            && let Some(text) = block.get("text").and_then(|v| v.as_str())
                        {
                            texts.push(text.to_string());
                        }
                    }
                }
            }
        }
    }
    texts.join(" ")
}

/// Load and parse a conversation from a session JSONL file.
pub fn load_conversation(path: &Path) -> Result<Vec<ConversationMessage>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read session file: {}", path.display()))?;

    let messages: Vec<ConversationMessage> =
        content.lines().filter_map(parse_message_line).collect();

    Ok(messages)
}

/// Filter conversation messages to only those suitable for display.
/// Returns user text and assistant text blocks, excluding sidechains.
/// For assistant messages, only keeps Text content blocks.
pub fn display_messages(messages: Vec<ConversationMessage>) -> Vec<ConversationMessage> {
    messages
        .into_iter()
        .filter(|msg| {
            if msg.is_sidechain {
                return false;
            }
            match msg.role.as_str() {
                "user" | "assistant" => msg
                    .content_blocks
                    .iter()
                    .any(|b| matches!(b, ContentBlock::Text(_))),
                _ => false,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // --- decode_project_path tests ---

    #[test]
    fn test_decode_project_path_basic() {
        assert_eq!(
            decode_project_path("-Users-foo-Documents-project"),
            "/Users/foo/Documents/project"
        );
    }

    #[test]
    fn test_decode_project_path_empty() {
        assert_eq!(decode_project_path(""), "");
    }

    #[test]
    fn test_decode_project_path_single_component() {
        assert_eq!(decode_project_path("-home"), "/home");
    }

    // --- project_display_name tests ---

    #[test]
    fn test_project_display_name_basic() {
        assert_eq!(
            project_display_name("/Users/foo/Documents/my-project"),
            "my-project"
        );
    }

    #[test]
    fn test_project_display_name_trailing_slash() {
        assert_eq!(
            project_display_name("/Users/foo/Documents/my-project/"),
            "my-project"
        );
    }

    #[test]
    fn test_project_display_name_root() {
        assert_eq!(project_display_name("/"), "");
    }

    // --- parse_message_line tests ---

    #[test]
    fn test_parse_user_message_string_content() {
        let json = r#"{"type":"user","uuid":"abc-123","parentUuid":null,"isSidechain":false,"message":{"role":"user","content":"Hello world"},"timestamp":"2026-04-08T10:00:00Z"}"#;
        let msg = parse_message_line(json).unwrap();
        assert_eq!(msg.role, "user");
        assert_eq!(msg.uuid, "abc-123");
        assert!(msg.parent_uuid.is_none());
        assert!(!msg.is_sidechain);
        assert_eq!(
            msg.content_blocks,
            vec![ContentBlock::Text("Hello world".into())]
        );
        assert!(msg.timestamp.is_some());
    }

    #[test]
    fn test_parse_user_message_array_content() {
        let json = r#"{"type":"user","uuid":"abc-456","parentUuid":"abc-123","isSidechain":false,"message":{"role":"user","content":[{"type":"text","text":"Run terraform plan"}]},"timestamp":"2026-04-08T10:01:00Z"}"#;
        let msg = parse_message_line(json).unwrap();
        assert_eq!(msg.role, "user");
        assert_eq!(
            msg.content_blocks,
            vec![ContentBlock::Text("Run terraform plan".into())]
        );
    }

    #[test]
    fn test_parse_assistant_message_with_text() {
        let json = r#"{"type":"assistant","uuid":"def-789","parentUuid":"abc-123","isSidechain":false,"message":{"role":"assistant","content":[{"type":"text","text":"Here are the results"}]},"timestamp":"2026-04-08T10:02:00Z"}"#;
        let msg = parse_message_line(json).unwrap();
        assert_eq!(msg.role, "assistant");
        assert_eq!(
            msg.content_blocks,
            vec![ContentBlock::Text("Here are the results".into())]
        );
    }

    #[test]
    fn test_parse_assistant_message_with_thinking_and_tool_use() {
        let json = r#"{"type":"assistant","uuid":"def-012","parentUuid":"abc-456","isSidechain":false,"message":{"role":"assistant","content":[{"type":"thinking","thinking":"Let me think..."},{"type":"text","text":"I will run the command"},{"type":"tool_use","id":"toolu_01","name":"Bash","input":{}}]},"timestamp":"2026-04-08T10:03:00Z"}"#;
        let msg = parse_message_line(json).unwrap();
        assert_eq!(msg.content_blocks.len(), 3);
        assert_eq!(
            msg.content_blocks[0],
            ContentBlock::Thinking("Let me think...".into())
        );
        assert_eq!(
            msg.content_blocks[1],
            ContentBlock::Text("I will run the command".into())
        );
        assert_eq!(
            msg.content_blocks[2],
            ContentBlock::ToolUse {
                name: "Bash".into()
            }
        );
    }

    #[test]
    fn test_parse_file_history_snapshot_returns_none() {
        let json = r#"{"type":"file-history-snapshot","messageId":"snap-001","snapshot":{}}"#;
        assert!(parse_message_line(json).is_none());
    }

    #[test]
    fn test_parse_system_message_returns_none() {
        let json = r#"{"type":"system","uuid":"sys-001","message":{"role":"system","content":"System message"}}"#;
        assert!(parse_message_line(json).is_none());
    }

    #[test]
    fn test_parse_malformed_json_returns_none() {
        assert!(parse_message_line("not valid json {{{").is_none());
    }

    #[test]
    fn test_parse_timestamp_iso8601() {
        let json = r#"{"type":"user","uuid":"ts-001","message":{"role":"user","content":"test"},"timestamp":"2026-04-08T10:00:00Z"}"#;
        let msg = parse_message_line(json).unwrap();
        let ts = msg.timestamp.unwrap();
        assert_eq!(chrono::Datelike::year(&ts), 2026);
    }

    #[test]
    fn test_parse_timestamp_epoch_millis() {
        let json = r#"{"type":"user","uuid":"ts-002","message":{"role":"user","content":"test"},"timestamp":1759226506420}"#;
        let msg = parse_message_line(json).unwrap();
        assert!(msg.timestamp.is_some());
    }

    // --- display_messages tests ---

    #[test]
    fn test_display_messages_filters_correctly() {
        let messages = vec![
            ConversationMessage {
                uuid: "1".into(),
                parent_uuid: None,
                role: "user".into(),
                content_blocks: vec![ContentBlock::Text("Hello".into())],
                timestamp: None,
                is_sidechain: false,
            },
            ConversationMessage {
                uuid: "2".into(),
                parent_uuid: Some("1".into()),
                role: "assistant".into(),
                content_blocks: vec![
                    ContentBlock::Thinking("hmm".into()),
                    ContentBlock::Text("Hi there".into()),
                    ContentBlock::ToolUse {
                        name: "Bash".into(),
                    },
                ],
                timestamp: None,
                is_sidechain: false,
            },
            ConversationMessage {
                uuid: "3".into(),
                parent_uuid: Some("2".into()),
                role: "user".into(),
                content_blocks: vec![ContentBlock::Text("Sidechain msg".into())],
                timestamp: None,
                is_sidechain: true,
            },
        ];

        let displayed = display_messages(messages);
        assert_eq!(displayed.len(), 2);
        assert_eq!(displayed[0].uuid, "1");
        assert_eq!(displayed[1].uuid, "2");
    }

    #[test]
    fn test_display_messages_excludes_thinking_only() {
        let messages = vec![ConversationMessage {
            uuid: "1".into(),
            parent_uuid: None,
            role: "assistant".into(),
            content_blocks: vec![ContentBlock::Thinking("only thinking".into())],
            timestamp: None,
            is_sidechain: false,
        }];

        let displayed = display_messages(messages);
        assert!(displayed.is_empty());
    }

    // --- load_conversation tests ---

    #[test]
    fn test_load_conversation_from_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test-session.jsonl");

        let content = r#"{"type":"user","uuid":"u1","parentUuid":null,"isSidechain":false,"message":{"role":"user","content":"Hello"},"timestamp":"2026-04-08T10:00:00Z"}
{"type":"assistant","uuid":"a1","parentUuid":"u1","isSidechain":false,"message":{"role":"assistant","content":[{"type":"thinking","thinking":"..."},{"type":"text","text":"Hi!"}]},"timestamp":"2026-04-08T10:00:01Z"}
{"type":"file-history-snapshot","messageId":"snap1","snapshot":{}}
{"type":"user","uuid":"u2","parentUuid":"a1","isSidechain":false,"message":{"role":"user","content":"Thanks"},"timestamp":"2026-04-08T10:00:02Z"}"#;

        fs::write(&file_path, content).unwrap();

        let messages = load_conversation(&file_path).unwrap();
        assert_eq!(messages.len(), 3); // 2 user + 1 assistant, snapshot skipped
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[2].role, "user");
    }

    #[test]
    fn test_load_conversation_skips_malformed_lines() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("bad-session.jsonl");

        let content = r#"not json at all
{"type":"user","uuid":"u1","message":{"role":"user","content":"Valid"},"timestamp":"2026-04-08T10:00:00Z"}
{"type":"broken"}"#;

        fs::write(&file_path, content).unwrap();

        let messages = load_conversation(&file_path).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].uuid, "u1");
    }

    // --- discover_sessions tests ---

    #[test]
    fn test_discover_sessions_with_index() {
        let dir = TempDir::new().unwrap();
        let claude_dir = dir.path();
        let project_dir = claude_dir
            .join("projects")
            .join("-Users-foo-Documents-myproject");
        fs::create_dir_all(&project_dir).unwrap();

        let index = serde_json::json!({
            "version": "1",
            "originalPath": "/Users/foo/Documents/myproject",
            "entries": [
                {
                    "sessionId": "sess-001",
                    "fullPath": project_dir.join("sess-001.jsonl").to_str().unwrap(),
                    "firstPrompt": "Hello world",
                    "summary": "Test session",
                    "messageCount": 5,
                    "created": "2026-04-08T10:00:00Z",
                    "modified": "2026-04-08T12:00:00Z",
                    "gitBranch": "main",
                    "projectPath": "/Users/foo/Documents/myproject",
                    "isSidechain": false
                }
            ]
        });

        fs::write(
            project_dir.join("sessions-index.json"),
            serde_json::to_string(&index).unwrap(),
        )
        .unwrap();

        // Create the JSONL file too (though index is used)
        fs::write(project_dir.join("sess-001.jsonl"), "").unwrap();

        let sessions = discover_sessions(claude_dir).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "sess-001");
        assert_eq!(sessions[0].first_prompt, "Hello world");
        assert_eq!(sessions[0].project_display, "myproject");
        assert_eq!(sessions[0].git_branch.as_deref(), Some("main"));
        assert_eq!(sessions[0].message_count, 5);
    }

    #[test]
    fn test_discover_sessions_jsonl_fallback() {
        let dir = TempDir::new().unwrap();
        let claude_dir = dir.path();
        let project_dir = claude_dir.join("projects").join("-Users-bar-code-app");
        fs::create_dir_all(&project_dir).unwrap();

        let jsonl = r#"{"type":"user","uuid":"u1","parentUuid":null,"isSidechain":false,"message":{"role":"user","content":"Build the app"},"timestamp":"2026-04-07T09:00:00Z","gitBranch":"feature/x"}
{"type":"assistant","uuid":"a1","parentUuid":"u1","isSidechain":false,"message":{"role":"assistant","content":[{"type":"text","text":"Sure!"}]},"timestamp":"2026-04-07T09:01:00Z"}"#;

        fs::write(project_dir.join("sess-002.jsonl"), jsonl).unwrap();

        let sessions = discover_sessions(claude_dir).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "sess-002");
        assert_eq!(sessions[0].first_prompt, "Build the app");
        assert_eq!(sessions[0].project_display, "app");
        assert_eq!(sessions[0].git_branch.as_deref(), Some("feature/x"));
        assert_eq!(sessions[0].message_count, 2);
    }

    #[test]
    fn test_discover_sessions_empty_projects_dir() {
        let dir = TempDir::new().unwrap();
        let claude_dir = dir.path();
        fs::create_dir_all(claude_dir.join("projects")).unwrap();

        let sessions = discover_sessions(claude_dir).unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_discover_sessions_excludes_empty_sessions_from_index() {
        let dir = TempDir::new().unwrap();
        let claude_dir = dir.path();
        let project_dir = claude_dir
            .join("projects")
            .join("-Users-foo-Documents-proj");
        fs::create_dir_all(&project_dir).unwrap();

        let index = serde_json::json!({
            "version": "1",
            "entries": [
                {
                    "sessionId": "has-messages",
                    "fullPath": project_dir.join("has-messages.jsonl").to_str().unwrap(),
                    "firstPrompt": "Hello",
                    "messageCount": 3,
                    "created": "2026-04-08T10:00:00Z",
                    "modified": "2026-04-08T12:00:00Z",
                    "isSidechain": false
                },
                {
                    "sessionId": "no-messages",
                    "fullPath": project_dir.join("no-messages.jsonl").to_str().unwrap(),
                    "firstPrompt": "",
                    "messageCount": 0,
                    "created": "2026-04-08T10:00:00Z",
                    "modified": "2026-04-08T11:00:00Z",
                    "isSidechain": false
                },
                {
                    "sessionId": "null-messages",
                    "fullPath": project_dir.join("null-messages.jsonl").to_str().unwrap(),
                    "firstPrompt": "",
                    "created": "2026-04-08T10:00:00Z",
                    "modified": "2026-04-08T11:00:00Z",
                    "isSidechain": false
                }
            ]
        });

        fs::write(
            project_dir.join("sessions-index.json"),
            serde_json::to_string(&index).unwrap(),
        )
        .unwrap();

        // Create JSONL file for the session that should pass filtering
        fs::write(project_dir.join("has-messages.jsonl"), "").unwrap();

        let sessions = discover_sessions(claude_dir).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "has-messages");
    }

    #[test]
    fn test_discover_sessions_excludes_missing_files_from_index() {
        let dir = TempDir::new().unwrap();
        let claude_dir = dir.path();
        let project_dir = claude_dir
            .join("projects")
            .join("-Users-foo-Documents-proj");
        fs::create_dir_all(&project_dir).unwrap();

        let index = serde_json::json!({
            "version": "1",
            "entries": [
                {
                    "sessionId": "exists",
                    "fullPath": project_dir.join("exists.jsonl").to_str().unwrap(),
                    "firstPrompt": "Hello",
                    "messageCount": 3,
                    "created": "2026-04-08T10:00:00Z",
                    "modified": "2026-04-08T12:00:00Z",
                    "isSidechain": false
                },
                {
                    "sessionId": "missing",
                    "fullPath": project_dir.join("missing.jsonl").to_str().unwrap(),
                    "firstPrompt": "World",
                    "messageCount": 5,
                    "created": "2026-04-08T10:00:00Z",
                    "modified": "2026-04-08T11:00:00Z",
                    "isSidechain": false
                }
            ]
        });

        fs::write(
            project_dir.join("sessions-index.json"),
            serde_json::to_string(&index).unwrap(),
        )
        .unwrap();

        // Only create the file for the "exists" session
        fs::write(project_dir.join("exists.jsonl"), "").unwrap();

        let sessions = discover_sessions(claude_dir).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "exists");
    }

    #[test]
    fn test_discover_sessions_excludes_empty_sessions_from_jsonl_scan() {
        let dir = TempDir::new().unwrap();
        let claude_dir = dir.path();
        let project_dir = claude_dir.join("projects").join("-Users-bar-code-app");
        fs::create_dir_all(&project_dir).unwrap();

        // Session with messages
        let jsonl_with_messages = r#"{"type":"user","uuid":"u1","parentUuid":null,"isSidechain":false,"message":{"role":"user","content":"Hello"},"timestamp":"2026-04-07T09:00:00Z"}
{"type":"assistant","uuid":"a1","parentUuid":"u1","isSidechain":false,"message":{"role":"assistant","content":[{"type":"text","text":"Hi!"}]},"timestamp":"2026-04-07T09:01:00Z"}"#;
        fs::write(
            project_dir.join("sess-with-msgs.jsonl"),
            jsonl_with_messages,
        )
        .unwrap();

        // Session without messages (only system/snapshot lines)
        let jsonl_no_messages = r#"{"type":"system","uuid":"s1","message":{"role":"system","content":"System init"}}
{"type":"file-history-snapshot","messageId":"snap1","snapshot":{}}"#;
        fs::write(project_dir.join("sess-no-msgs.jsonl"), jsonl_no_messages).unwrap();

        // Empty session file
        fs::write(project_dir.join("sess-empty.jsonl"), "").unwrap();

        let sessions = discover_sessions(claude_dir).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "sess-with-msgs");
    }

    #[test]
    fn test_has_meaningful_prompt_filters_empty() {
        assert!(!has_meaningful_prompt(""));
        assert!(!has_meaningful_prompt("   "));
    }

    #[test]
    fn test_has_meaningful_prompt_filters_no_prompt() {
        assert!(!has_meaningful_prompt("No prompt"));
    }

    #[test]
    fn test_has_meaningful_prompt_filters_stderr_errors() {
        assert!(!has_meaningful_prompt(
            "<local-command-stderr>Error: Bash command permission check failed"
        ));
        assert!(!has_meaningful_prompt(
            "<local-command-stderr>Error: Bash command failed for pattern"
        ));
    }

    #[test]
    fn test_has_meaningful_prompt_accepts_real_prompts() {
        assert!(has_meaningful_prompt("Hello world"));
        assert!(has_meaningful_prompt("Run terraform plan"));
        assert!(has_meaningful_prompt("日本語のプロンプト"));
    }

    #[test]
    fn test_discover_sessions_excludes_no_prompt_from_index() {
        let dir = TempDir::new().unwrap();
        let claude_dir = dir.path();
        let project_dir = claude_dir
            .join("projects")
            .join("-Users-foo-Documents-proj2");
        fs::create_dir_all(&project_dir).unwrap();

        let index = serde_json::json!({
            "version": "1",
            "entries": [
                {
                    "sessionId": "real-session",
                    "fullPath": project_dir.join("real-session.jsonl").to_str().unwrap(),
                    "firstPrompt": "Hello world",
                    "messageCount": 4,
                    "created": "2026-04-08T10:00:00Z",
                    "modified": "2026-04-08T12:00:00Z",
                    "isSidechain": false
                },
                {
                    "sessionId": "no-prompt-session",
                    "fullPath": project_dir.join("no-prompt-session.jsonl").to_str().unwrap(),
                    "firstPrompt": "No prompt",
                    "messageCount": 2,
                    "created": "2026-04-08T10:00:00Z",
                    "modified": "2026-04-08T11:00:00Z",
                    "isSidechain": false
                },
                {
                    "sessionId": "stderr-session",
                    "fullPath": project_dir.join("stderr-session.jsonl").to_str().unwrap(),
                    "firstPrompt": "<local-command-stderr>Error: Bash command permission check failed",
                    "messageCount": 2,
                    "created": "2026-04-08T10:00:00Z",
                    "modified": "2026-04-08T11:00:00Z",
                    "isSidechain": false
                }
            ]
        });

        fs::write(
            project_dir.join("sessions-index.json"),
            serde_json::to_string(&index).unwrap(),
        )
        .unwrap();

        // Create JSONL files so they pass the existence check
        fs::write(project_dir.join("real-session.jsonl"), "").unwrap();
        fs::write(project_dir.join("no-prompt-session.jsonl"), "").unwrap();
        fs::write(project_dir.join("stderr-session.jsonl"), "").unwrap();

        let sessions = discover_sessions(claude_dir).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "real-session");
    }

    #[test]
    fn test_discover_sessions_excludes_no_prompt_from_jsonl_scan() {
        let dir = TempDir::new().unwrap();
        let claude_dir = dir.path();
        let project_dir = claude_dir.join("projects").join("-Users-bar-code-app2");
        fs::create_dir_all(&project_dir).unwrap();

        // Session with real conversation
        let jsonl_real = r#"{"type":"user","uuid":"u1","parentUuid":null,"isSidechain":false,"message":{"role":"user","content":"Hello"},"timestamp":"2026-04-07T09:00:00Z"}
{"type":"assistant","uuid":"a1","parentUuid":"u1","isSidechain":false,"message":{"role":"assistant","content":[{"type":"text","text":"Hi!"}]},"timestamp":"2026-04-07T09:01:00Z"}"#;
        fs::write(project_dir.join("sess-real.jsonl"), jsonl_real).unwrap();

        // Session with only stderr error content
        let jsonl_stderr = r#"{"type":"user","uuid":"u1","parentUuid":null,"isSidechain":false,"message":{"role":"user","content":"<local-command-stderr>Error: Bash command permission check failed"},"timestamp":"2026-04-07T09:00:00Z"}
{"type":"assistant","uuid":"a1","parentUuid":"u1","isSidechain":false,"message":{"role":"assistant","content":[{"type":"text","text":"Error"}]},"timestamp":"2026-04-07T09:01:00Z"}"#;
        fs::write(project_dir.join("sess-stderr.jsonl"), jsonl_stderr).unwrap();

        let sessions = discover_sessions(claude_dir).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "sess-real");
    }

    #[test]
    fn test_discover_sessions_no_projects_dir() {
        let dir = TempDir::new().unwrap();
        let sessions = discover_sessions(dir.path()).unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_discover_sessions_skips_sidechain_entries() {
        let dir = TempDir::new().unwrap();
        let claude_dir = dir.path();
        let project_dir = claude_dir.join("projects").join("-Users-test-proj");
        fs::create_dir_all(&project_dir).unwrap();

        let index = serde_json::json!({
            "version": "1",
            "entries": [
                {
                    "sessionId": "main-sess",
                    "fullPath": project_dir.join("main-sess.jsonl").to_str().unwrap(),
                    "firstPrompt": "Main session",
                    "messageCount": 5,
                    "created": "2026-04-08T10:00:00Z",
                    "modified": "2026-04-08T12:00:00Z",
                    "isSidechain": false
                },
                {
                    "sessionId": "side-sess",
                    "fullPath": project_dir.join("side-sess.jsonl").to_str().unwrap(),
                    "firstPrompt": "Sidechain",
                    "messageCount": 3,
                    "created": "2026-04-08T10:00:00Z",
                    "modified": "2026-04-08T12:00:00Z",
                    "isSidechain": true
                }
            ]
        });

        fs::write(
            project_dir.join("sessions-index.json"),
            serde_json::to_string(&index).unwrap(),
        )
        .unwrap();

        // Create JSONL files
        fs::write(project_dir.join("main-sess.jsonl"), "").unwrap();
        fs::write(project_dir.join("side-sess.jsonl"), "").unwrap();

        let sessions = discover_sessions(claude_dir).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "main-sess");
    }

    #[test]
    fn test_discover_sessions_merges_index_and_jsonl_scan() {
        let dir = TempDir::new().unwrap();
        let claude_dir = dir.path();
        let project_dir = claude_dir
            .join("projects")
            .join("-Users-foo-Documents-merge");
        fs::create_dir_all(&project_dir).unwrap();

        // Index contains only one session
        let index = serde_json::json!({
            "version": "1",
            "entries": [
                {
                    "sessionId": "indexed-sess",
                    "fullPath": project_dir.join("indexed-sess.jsonl").to_str().unwrap(),
                    "firstPrompt": "From index",
                    "messageCount": 4,
                    "created": "2026-02-01T10:00:00Z",
                    "modified": "2026-02-01T12:00:00Z",
                    "isSidechain": false
                }
            ]
        });

        fs::write(
            project_dir.join("sessions-index.json"),
            serde_json::to_string(&index).unwrap(),
        )
        .unwrap();

        // Create indexed JSONL file
        fs::write(project_dir.join("indexed-sess.jsonl"), "").unwrap();

        // Create a NEW JSONL file not in the index (added after index was last updated)
        let new_jsonl = r#"{"type":"user","uuid":"u1","parentUuid":null,"isSidechain":false,"message":{"role":"user","content":"New session"},"timestamp":"2026-04-01T09:00:00Z","gitBranch":"main"}
{"type":"assistant","uuid":"a1","parentUuid":"u1","isSidechain":false,"message":{"role":"assistant","content":[{"type":"text","text":"Hello!"}]},"timestamp":"2026-04-01T09:01:00Z"}"#;
        fs::write(project_dir.join("new-sess.jsonl"), new_jsonl).unwrap();

        let sessions = discover_sessions(claude_dir).unwrap();
        assert_eq!(sessions.len(), 2);

        let ids: Vec<&str> = sessions.iter().map(|s| s.session_id.as_str()).collect();
        assert!(ids.contains(&"indexed-sess"));
        assert!(ids.contains(&"new-sess"));
    }

    #[test]
    fn test_discover_sessions_index_takes_priority_over_jsonl() {
        let dir = TempDir::new().unwrap();
        let claude_dir = dir.path();
        let project_dir = claude_dir
            .join("projects")
            .join("-Users-foo-Documents-dedup");
        fs::create_dir_all(&project_dir).unwrap();

        // Index has a session with specific metadata
        let index = serde_json::json!({
            "version": "1",
            "entries": [
                {
                    "sessionId": "same-sess",
                    "fullPath": project_dir.join("same-sess.jsonl").to_str().unwrap(),
                    "firstPrompt": "Index version",
                    "summary": "From the index",
                    "messageCount": 10,
                    "created": "2026-02-01T10:00:00Z",
                    "modified": "2026-02-01T12:00:00Z",
                    "isSidechain": false
                }
            ]
        });

        fs::write(
            project_dir.join("sessions-index.json"),
            serde_json::to_string(&index).unwrap(),
        )
        .unwrap();

        // JSONL file also exists (same session ID) with different first_prompt
        let jsonl = r#"{"type":"user","uuid":"u1","parentUuid":null,"isSidechain":false,"message":{"role":"user","content":"JSONL version"},"timestamp":"2026-02-01T10:00:00Z"}
{"type":"assistant","uuid":"a1","parentUuid":"u1","isSidechain":false,"message":{"role":"assistant","content":[{"type":"text","text":"Hi"}]},"timestamp":"2026-02-01T10:01:00Z"}"#;
        fs::write(project_dir.join("same-sess.jsonl"), jsonl).unwrap();

        let sessions = discover_sessions(claude_dir).unwrap();
        // Should have exactly 1 session (no duplicate)
        assert_eq!(sessions.len(), 1);
        // Index version should take priority
        assert_eq!(sessions[0].first_prompt, "Index version");
        assert_eq!(sessions[0].message_count, 10);
    }

    // --- extract_searchable_text tests ---

    #[test]
    fn test_extract_searchable_text_basic() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("search-test.jsonl");

        let content = r#"{"type":"user","uuid":"u1","parentUuid":null,"isSidechain":false,"message":{"role":"user","content":"実行日を設定してください"},"timestamp":"2026-04-08T10:00:00Z"}
{"type":"assistant","uuid":"a1","parentUuid":"u1","isSidechain":false,"message":{"role":"assistant","content":[{"type":"text","text":"実行日の設定を行います"}]},"timestamp":"2026-04-08T10:00:01Z"}"#;

        fs::write(&file_path, content).unwrap();

        let text = extract_searchable_text(&file_path);
        assert!(text.contains("実行日を設定してください"));
        assert!(text.contains("実行日の設定を行います"));
    }

    #[test]
    fn test_extract_searchable_text_skips_non_message_types() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("skip-test.jsonl");

        let content = r#"{"type":"file-history-snapshot","messageId":"snap1","snapshot":{}}
{"type":"user","uuid":"u1","message":{"role":"user","content":"Hello"},"timestamp":"2026-04-08T10:00:00Z"}
{"type":"system","content":"System message"}"#;

        fs::write(&file_path, content).unwrap();

        let text = extract_searchable_text(&file_path);
        assert!(text.contains("Hello"));
        assert!(!text.contains("System message"));
        assert!(!text.contains("snapshot"));
    }

    #[test]
    fn test_extract_searchable_text_extracts_only_text_blocks() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("blocks-test.jsonl");

        let content = r#"{"type":"assistant","uuid":"a1","parentUuid":null,"isSidechain":false,"message":{"role":"assistant","content":[{"type":"thinking","thinking":"internal thought"},{"type":"text","text":"Visible response"},{"type":"tool_use","id":"t1","name":"Bash","input":{}}]},"timestamp":"2026-04-08T10:00:00Z"}"#;

        fs::write(&file_path, content).unwrap();

        let text = extract_searchable_text(&file_path);
        assert!(text.contains("Visible response"));
        assert!(!text.contains("internal thought"));
    }

    #[test]
    fn test_extract_searchable_text_skips_sidechains() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("sidechain-test.jsonl");

        let content = r#"{"type":"user","uuid":"u1","parentUuid":null,"isSidechain":false,"message":{"role":"user","content":"Main conversation"},"timestamp":"2026-04-08T10:00:00Z"}
{"type":"user","uuid":"u2","parentUuid":"u1","isSidechain":true,"message":{"role":"user","content":"Sidechain hidden text"},"timestamp":"2026-04-08T10:00:01Z"}
{"type":"assistant","uuid":"a1","parentUuid":"u2","isSidechain":true,"message":{"role":"assistant","content":[{"type":"text","text":"Sidechain response"}]},"timestamp":"2026-04-08T10:00:02Z"}"#;

        fs::write(&file_path, content).unwrap();

        let text = extract_searchable_text(&file_path);
        assert!(text.contains("Main conversation"));
        assert!(!text.contains("Sidechain hidden text"));
        assert!(!text.contains("Sidechain response"));
    }

    #[test]
    fn test_extract_searchable_text_missing_file() {
        let text = extract_searchable_text(Path::new("/nonexistent/path.jsonl"));
        assert!(text.is_empty());
    }
}
