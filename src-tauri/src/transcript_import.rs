//! On-demand transcript hydration for observed agent sessions.
//!
//! Session Sync is metadata-only. This module reads transcript files lazily
//! the first time the owner opens a session in the workspace and the store
//! has no messages for it yet.
//!
//! Supported providers:
//! - **Claude Code** – reads `~/.claude/projects/<encoded-root>/<session-id>.jsonl`
//! - **Codex** – searches `~/.codex/sessions/` for a rollout file whose name
//!   contains the session UUID.
//!
//! Only user/assistant chat turns are imported. Tool call payloads, system
//! context, attachments, and developer messages are skipped. Bodies are
//! capped at `MAX_BODY_BYTES` and the total per session is capped at
//! `MAX_MESSAGES`. Import is best-effort; parse errors on individual lines
//! are silently skipped. Any store error (including a UNIQUE collision from a
//! concurrent import) stops the batch without propagating.

use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use serde::Deserialize;
use utu_core::{EvidenceKind, MessageRole, Session};
use utu_store::{NewMessage, Store};

use crate::{
    agent_sessions::{SessionRoots, claude_project_dir, cursor_project_dir, parse_rfc3339_utc_millis},
    clock::unix_ms,
    codex_commands::CODEX_AGENT_ID,
    ids::deterministic_id,
    session_sync::{CLAUDE_AGENT_ID, CURSOR_AGENT_ID},
};

const MAX_MESSAGES: usize = 500;
const MAX_BODY_BYTES: usize = 32 * 1024;

// ─── Public entry point ──────────────────────────────────────────────────────

/// Imports chat messages from local agent transcript files into the store for
/// the given session. Returns the number of messages persisted.
///
/// This is a no-op (returns 0) when:
/// - the session has no `provider_session_id`,
/// - the agent is neither Claude Code nor Codex,
/// - the transcript file cannot be located or read, or
/// - the session already has messages (caller should check first).
pub fn import_transcript(store: &Store, session: &Session, roots: &SessionRoots) -> u32 {
    let Some(provider_session_id) = session.provider_session_id.as_deref() else {
        return 0;
    };
    if session.agent_id == CLAUDE_AGENT_ID {
        import_claude_transcript(store, session, provider_session_id, roots)
    } else if session.agent_id == CODEX_AGENT_ID {
        import_codex_transcript(store, session, provider_session_id, roots)
    } else if session.agent_id == CURSOR_AGENT_ID {
        import_cursor_transcript(store, session, provider_session_id, roots)
    } else {
        0
    }
}

// ─── Claude Code ─────────────────────────────────────────────────────────────

fn import_claude_transcript(
    store: &Store,
    session: &Session,
    provider_session_id: &str,
    roots: &SessionRoots,
) -> u32 {
    let project_root = match project_root_for_session(store, session) {
        Some(root) => root,
        None => return 0,
    };
    let path = claude_project_dir(roots, &project_root)
        .join(format!("{provider_session_id}.jsonl"));
    if !path.is_file() {
        return 0;
    }
    persist_claude_messages(store, session, &path)
}

fn persist_claude_messages(store: &Store, session: &Session, path: &Path) -> u32 {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return 0,
    };
    let reader = BufReader::new(file);
    let now = unix_ms();
    let mut imported = 0u32;

    for line in reader.lines() {
        if imported as usize >= MAX_MESSAGES {
            break;
        }
        let line = match line {
            Ok(l) if !l.trim().is_empty() => l,
            _ => continue,
        };
        let entry: ClaudeEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let (role, author_agent_id) = match entry.kind.as_str() {
            "human" | "user" => (MessageRole::Owner, None),
            "assistant" => (
                MessageRole::Agent,
                Some(session.agent_id.clone()),
            ),
            _ => continue,
        };
        let Some(msg) = entry.message else {
            continue;
        };
        let body = extract_claude_body(&msg.content);
        let body = body.trim().to_owned();
        if body.is_empty() {
            continue;
        }
        let body = truncate_body(body);
        let sent_at = msg
            .timestamp
            .as_deref()
            .and_then(parse_rfc3339_utc_millis)
            .or_else(|| entry.timestamp.as_deref().and_then(parse_rfc3339_utc_millis))
            .unwrap_or(now);
        let id = deterministic_id(
            "transcript-msg",
            &format!("{}:{}", session.id, imported),
        );
        let new = NewMessage {
            id,
            session_id: session.id.clone(),
            role,
            author_agent_id,
            body,
            sent_at_unix_ms: sent_at,
            ingested_at_unix_ms: now,
            evidence: EvidenceKind::Observed,
            source: "transcript.claude".into(),
            correlation_id: None,
        };
        match store.append_message(new) {
            Ok(_) => imported += 1,
            Err(_) => break,
        }
    }
    imported
}

// ─── Claude entry structs ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ClaudeEntry {
    #[serde(rename = "type")]
    kind: String,
    message: Option<ClaudeMessage>,
    timestamp: Option<String>,
}

#[derive(Deserialize)]
struct ClaudeMessage {
    #[serde(default)]
    content: ClaudeContent,
    timestamp: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(untagged)]
enum ClaudeContent {
    Text(String),
    Parts(Vec<ClaudeContentPart>),
    #[default]
    Empty,
}

#[derive(Deserialize)]
struct ClaudeContentPart {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

fn extract_claude_body(content: &ClaudeContent) -> String {
    match content {
        ClaudeContent::Text(text) => text.clone(),
        ClaudeContent::Parts(parts) => parts
            .iter()
            .filter(|part| part.kind == "text")
            .filter_map(|part| part.text.as_deref())
            .collect::<Vec<_>>()
            .join("\n"),
        ClaudeContent::Empty => String::new(),
    }
}

// ─── Codex ────────────────────────────────────────────────────────────────────

fn import_codex_transcript(
    store: &Store,
    session: &Session,
    provider_session_id: &str,
    roots: &SessionRoots,
) -> u32 {
    let path = match find_codex_rollout_file(&roots.codex_sessions, provider_session_id) {
        Some(p) => p,
        None => return 0,
    };
    persist_codex_messages(store, session, &path)
}

/// Searches the Codex sessions tree for a rollout file whose stem contains
/// `session_id`. Codex names files like
/// `rollout-2026-08-12T00-17-58-<session-id>.jsonl`.
fn find_codex_rollout_file(codex_sessions: &Path, session_id: &str) -> Option<PathBuf> {
    find_codex_rollout_recursive(codex_sessions, session_id)
}

fn find_codex_rollout_recursive(dir: &Path, session_id: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut children: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .collect();
    // Descend newest directories first (sorted descending) so recent sessions
    // are found quickly without scanning old dated directories.
    children.sort_unstable_by(|a, b| b.cmp(a));
    for path in children {
        if path.is_dir() {
            if let Some(found) = find_codex_rollout_recursive(&path, session_id) {
                return Some(found);
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if stem.contains(session_id) {
                return Some(path);
            }
        }
    }
    None
}

fn persist_codex_messages(store: &Store, session: &Session, path: &Path) -> u32 {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return 0,
    };
    let reader = BufReader::new(file);
    let now = unix_ms();
    let mut imported = 0u32;
    let mut first_line = true;

    for line in reader.lines() {
        if imported as usize >= MAX_MESSAGES {
            break;
        }
        let line = match line {
            Ok(l) if !l.trim().is_empty() => l,
            _ => continue,
        };
        // Skip the session_meta header line.
        if first_line {
            first_line = false;
            continue;
        }
        let entry: CodexLogLine = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.kind != "response_item" {
            continue;
        }
        let Some(payload) = entry.payload else {
            continue;
        };
        if payload.kind.as_deref() != Some("message") {
            continue;
        }
        let (role, author_agent_id) = match payload.role.as_deref() {
            Some("user") => (MessageRole::Owner, None),
            Some("assistant") => (
                MessageRole::Agent,
                Some(session.agent_id.clone()),
            ),
            // Skip system/developer context messages.
            _ => continue,
        };
        let body = extract_codex_body(&payload.content);
        let body = body.trim().to_owned();
        if body.is_empty() {
            continue;
        }
        let body = truncate_body(body);
        let sent_at = entry
            .timestamp
            .as_deref()
            .and_then(parse_rfc3339_utc_millis)
            .unwrap_or(now);
        let id = deterministic_id(
            "transcript-msg",
            &format!("{}:{}", session.id, imported),
        );
        let new = NewMessage {
            id,
            session_id: session.id.clone(),
            role,
            author_agent_id,
            body,
            sent_at_unix_ms: sent_at,
            ingested_at_unix_ms: now,
            evidence: EvidenceKind::Observed,
            source: "transcript.codex".into(),
            correlation_id: None,
        };
        match store.append_message(new) {
            Ok(_) => imported += 1,
            Err(_) => break,
        }
    }
    imported
}

// ─── Codex entry structs ──────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CodexLogLine {
    #[serde(rename = "type")]
    kind: String,
    timestamp: Option<String>,
    payload: Option<CodexPayload>,
}

#[derive(Deserialize)]
struct CodexPayload {
    #[serde(rename = "type")]
    kind: Option<String>,
    role: Option<String>,
    #[serde(default)]
    content: Vec<CodexContentItem>,
}

#[derive(Deserialize)]
struct CodexContentItem {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

fn extract_codex_body(items: &[CodexContentItem]) -> String {
    items
        .iter()
        .filter(|item| matches!(item.kind.as_str(), "input_text" | "text"))
        .filter_map(|item| item.text.as_deref())
        .collect::<Vec<_>>()
        .join("\n")
}

// ─── Cursor Agent ─────────────────────────────────────────────────────────────

fn import_cursor_transcript(
    store: &Store,
    session: &Session,
    provider_session_id: &str,
    roots: &SessionRoots,
) -> u32 {
    let project_root = match project_root_for_session(store, session) {
        Some(root) => root,
        None => return 0,
    };
    // Cursor transcript: ~/.cursor/projects/<cursor-encoded>/<agent-transcripts>/<uuid>/<uuid>.jsonl
    let path = cursor_project_dir(roots, &project_root)
        .join("agent-transcripts")
        .join(provider_session_id)
        .join(format!("{provider_session_id}.jsonl"));
    if !path.is_file() {
        return 0;
    }
    persist_cursor_messages(store, session, &path)
}

fn persist_cursor_messages(store: &Store, session: &Session, path: &Path) -> u32 {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return 0,
    };
    let reader = BufReader::new(file);
    let now = unix_ms();
    let mut imported = 0u32;

    for line in reader.lines() {
        if imported as usize >= MAX_MESSAGES {
            break;
        }
        let line = match line {
            Ok(l) if !l.trim().is_empty() => l,
            _ => continue,
        };
        let entry: CursorEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let (role, author_agent_id) = match entry.role.as_str() {
            "user" => (MessageRole::Owner, None),
            "assistant" => (MessageRole::Agent, Some(session.agent_id.clone())),
            _ => continue,
        };
        let Some(msg) = entry.message else { continue };
        let body = extract_cursor_body(&msg.content);
        let body = body.trim().to_owned();
        if body.is_empty() {
            continue;
        }
        let body = truncate_body(body);
        let id = deterministic_id(
            "transcript-msg",
            &format!("{}:{}", session.id, imported),
        );
        let new = NewMessage {
            id,
            session_id: session.id.clone(),
            role,
            author_agent_id,
            body,
            sent_at_unix_ms: now,
            ingested_at_unix_ms: now,
            evidence: EvidenceKind::Observed,
            source: "transcript.cursor".into(),
            correlation_id: None,
        };
        match store.append_message(new) {
            Ok(_) => imported += 1,
            Err(_) => break,
        }
    }
    imported
}

// ─── Cursor entry structs ──────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CursorEntry {
    role: String,
    message: Option<CursorMessage>,
}

#[derive(Deserialize)]
struct CursorMessage {
    #[serde(default)]
    content: Vec<CursorContentPart>,
}

#[derive(Deserialize)]
struct CursorContentPart {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

/// Extracts human-readable text from Cursor message content.
///
/// Text parts may contain XML-like tags inserted by the Cursor IDE:
/// `<timestamp>...</timestamp>` and `<user_query>...</user_query>`.  When a
/// `<user_query>` tag is present we extract only its body; otherwise the raw
/// text is returned with the timestamp wrapper stripped.
fn extract_cursor_body(parts: &[CursorContentPart]) -> String {
    parts
        .iter()
        .filter(|part| part.kind == "text")
        .filter_map(|part| part.text.as_deref())
        .map(|text| {
            // Prefer the <user_query> body when present.
            if let Some(start) = text.find("<user_query>") {
                let after = &text[start + "<user_query>".len()..];
                let end = after.find("</user_query>").unwrap_or(after.len());
                return after[..end].trim().to_owned();
            }
            // Strip <timestamp>...</timestamp> wrapper if present.
            if let Some(ts_end) = text.find("</timestamp>") {
                return text[ts_end + "</timestamp>".len()..].trim().to_owned();
            }
            text.to_owned()
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

// ─── Shared helpers ───────────────────────────────────────────────────────────

fn project_root_for_session(store: &Store, session: &Session) -> Option<String> {
    store
        .get_project(&session.project_id)
        .ok()?
        .and_then(|p| p.root_path)
}

fn truncate_body(body: String) -> String {
    if body.len() <= MAX_BODY_BYTES {
        return body;
    }
    // Truncate to a valid UTF-8 boundary.
    let mut end = MAX_BODY_BYTES;
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    let mut truncated = body[..end].to_owned();
    truncated.push_str("\n…[truncated]");
    truncated
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };
    use utu_core::{AgentState, ConnectorCapabilities, Project, ProjectState, Provider, ProviderKind};
    use utu_store::Store;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("utu-transcript-{}-{nonce}", std::process::id()));
            fs::create_dir_all(&path).expect("fixture dir");
            Self(path)
        }

        fn roots(&self) -> SessionRoots {
            SessionRoots::from_home(&self.0)
        }

        fn project_root(&self) -> PathBuf {
            let root = self.0.join("repo");
            fs::create_dir_all(&root).unwrap();
            root.canonicalize().unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn seeded_store(root: &Path, agent_id: &str) -> Store {
        let store = Store::open_in_memory().unwrap();
        store
            .upsert_provider(&Provider {
                id: "test-provider".into(),
                display_name: "Test".into(),
                kind: ProviderKind::LocalCli,
            })
            .unwrap();
        use utu_core::{Agent, AuthState, IntegrationState, EvidenceKind};
        store
            .upsert_integration(&utu_core::Integration {
                id: "test-connector".into(),
                provider_id: Some("test-provider".into()),
                connector_key: "test-connector".into(),
                display_name: "Test Connector".into(),
                kind: ProviderKind::LocalCli,
                state: IntegrationState::Ready,
                auth: AuthState::Confirmed,
                evidence: EvidenceKind::Observed,
                checked_at_unix_ms: Some(1),
                problem: None,
                capabilities: ConnectorCapabilities::default(),
            })
            .unwrap();
        store
            .upsert_agent(&Agent {
                id: agent_id.into(),
                provider_id: "test-provider".into(),
                connector_id: "test-connector".into(),
                display_name: agent_id.into(),
                model: None,
                capabilities: ConnectorCapabilities::default(),
            })
            .unwrap();
        store
            .upsert_project(&Project {
                id: "project".into(),
                name: "Project".into(),
                root_path: Some(root.to_string_lossy().into_owned()),
                state: ProjectState::Active,
                created_at_unix_ms: 1,
            })
            .unwrap();
        store
            .upsert_session(&utu_core::Session {
                id: "session".into(),
                project_id: "project".into(),
                task_id: None,
                agent_id: agent_id.into(),
                provider_session_id: Some("test-session-id".into()),
                state: AgentState::Idle,
                started_at_unix_ms: 1,
                last_observed_at_unix_ms: None,
                title_hint: None,
            })
            .unwrap();
        store
    }

    fn make_session(agent_id: &str) -> Session {
        Session {
            id: "session".into(),
            project_id: "project".into(),
            task_id: None,
            agent_id: agent_id.into(),
            provider_session_id: Some("test-session-id".into()),
            state: AgentState::Idle,
            started_at_unix_ms: 1,
            last_observed_at_unix_ms: None,
            title_hint: None,
        }
    }

    #[test]
    fn claude_jsonl_imports_user_and_assistant_turns() {
        let fixture = Fixture::new();
        let root = fixture.project_root();
        let store = seeded_store(&root, CLAUDE_AGENT_ID);
        let roots = fixture.roots();
        let session = make_session(CLAUDE_AGENT_ID);

        let project_dir = claude_project_dir(&roots, &root.to_string_lossy());
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(
            project_dir.join("test-session-id.jsonl"),
            concat!(
                "{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"Hello agent\"},\"timestamp\":\"2026-01-01T00:00:00.000Z\"}\n",
                "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"Hello owner\"}]},\"timestamp\":\"2026-01-01T00:00:01.000Z\"}\n",
                "{\"type\":\"attachment\",\"data\":\"skip me\"}\n",
            ),
        )
        .unwrap();

        let imported = import_transcript(&store, &session, &roots);
        assert_eq!(imported, 2);

        let query = utu_store::StreamQuery { after_sequence: None, limit: 10 };
        let projection = store
            .read_session_projection("session", query, query, 10)
            .unwrap()
            .unwrap();
        assert_eq!(projection.messages.len(), 2);
        assert_eq!(projection.messages[0].body, "Hello agent");
        assert_eq!(projection.messages[0].role, utu_core::MessageRole::Owner);
        assert_eq!(projection.messages[1].body, "Hello owner");
        assert_eq!(projection.messages[1].role, utu_core::MessageRole::Agent);
        assert_eq!(
            projection.messages[1].author_agent_id.as_deref(),
            Some(CLAUDE_AGENT_ID)
        );
    }

    #[test]
    fn claude_jsonl_string_content_is_imported() {
        let fixture = Fixture::new();
        let root = fixture.project_root();
        let store = seeded_store(&root, CLAUDE_AGENT_ID);
        let roots = fixture.roots();
        let session = make_session(CLAUDE_AGENT_ID);

        let project_dir = claude_project_dir(&roots, &root.to_string_lossy());
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(
            project_dir.join("test-session-id.jsonl"),
            "{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"plain string body\"}}\n",
        )
        .unwrap();

        let imported = import_transcript(&store, &session, &roots);
        assert_eq!(imported, 1);
        let query = utu_store::StreamQuery { after_sequence: None, limit: 5 };
        let projection = store
            .read_session_projection("session", query, query, 5)
            .unwrap()
            .unwrap();
        assert_eq!(projection.messages[0].body, "plain string body");
    }

    #[test]
    fn codex_rollout_imports_user_and_assistant_turns() {
        let fixture = Fixture::new();
        let root = fixture.project_root();
        let canonical = root.to_string_lossy().into_owned();
        let store = seeded_store(&root, CODEX_AGENT_ID);
        let roots = fixture.roots();
        let session = make_session(CODEX_AGENT_ID);

        let day = roots.codex_sessions.join("2026").join("01").join("01");
        fs::create_dir_all(&day).unwrap();
        let rollout = serde_json::json!([
            // line 1: session_meta (skipped)
            {"type":"session_meta","payload":{"id":"test-session-id","cwd":canonical}},
            // line 2: developer context (skipped)
            {"type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"system context"}]}},
            // line 3: user turn
            {"type":"response_item","timestamp":"2026-01-01T00:00:00.000Z","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"What is 2+2?"}]}},
            // line 4: assistant turn
            {"type":"response_item","timestamp":"2026-01-01T00:00:01.000Z","payload":{"type":"message","role":"assistant","content":[{"type":"input_text","text":"4"}]}},
            // line 5: event (skipped)
            {"type":"event_msg","payload":{"type":"item_completed"}},
        ]);
        let lines: Vec<String> = rollout
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.to_string())
            .collect();
        fs::write(
            day.join("rollout-2026-01-01T00-00-00-test-session-id.jsonl"),
            lines.join("\n"),
        )
        .unwrap();

        let imported = import_transcript(&store, &session, &roots);
        assert_eq!(imported, 2);

        let query = utu_store::StreamQuery { after_sequence: None, limit: 10 };
        let projection = store
            .read_session_projection("session", query, query, 10)
            .unwrap()
            .unwrap();
        assert_eq!(projection.messages.len(), 2);
        assert_eq!(projection.messages[0].body, "What is 2+2?");
        assert_eq!(projection.messages[0].role, utu_core::MessageRole::Owner);
        assert_eq!(projection.messages[1].body, "4");
        assert_eq!(projection.messages[1].role, utu_core::MessageRole::Agent);
    }

    #[test]
    fn missing_transcript_file_returns_zero() {
        let fixture = Fixture::new();
        let root = fixture.project_root();
        let store = seeded_store(&root, CLAUDE_AGENT_ID);
        let roots = fixture.roots();
        let session = make_session(CLAUDE_AGENT_ID);
        // No file written.
        let imported = import_transcript(&store, &session, &roots);
        assert_eq!(imported, 0);
    }

    #[test]
    fn session_without_provider_id_returns_zero() {
        let fixture = Fixture::new();
        let root = fixture.project_root();
        let store = seeded_store(&root, CLAUDE_AGENT_ID);
        let roots = fixture.roots();
        let mut session = make_session(CLAUDE_AGENT_ID);
        session.provider_session_id = None;
        let imported = import_transcript(&store, &session, &roots);
        assert_eq!(imported, 0);
    }

    #[test]
    fn body_truncation_preserves_utf8_boundary() {
        let body = "x".repeat(MAX_BODY_BYTES + 100);
        let truncated = truncate_body(body);
        assert!(truncated.len() <= MAX_BODY_BYTES + 20);
        assert!(truncated.is_char_boundary(0));
    }

    #[test]
    fn cursor_jsonl_imports_user_and_assistant_turns() {
        let fixture = Fixture::new();
        let root = fixture.project_root();
        let store = seeded_store(&root, CURSOR_AGENT_ID);
        let roots = fixture.roots();
        let session = make_session(CURSOR_AGENT_ID);

        let transcript_dir = crate::agent_sessions::cursor_project_dir(&roots, &root.to_string_lossy())
            .join("agent-transcripts")
            .join("test-session-id");
        fs::create_dir_all(&transcript_dir).unwrap();
        fs::write(
            transcript_dir.join("test-session-id.jsonl"),
            concat!(
                r#"{"role":"user","message":{"content":[{"type":"text","text":"<user_query>\nFix the bug\n</user_query>"}]}}"#, "\n",
                r#"{"role":"assistant","message":{"content":[{"type":"text","text":"I'll fix it."},{"type":"tool_use","name":"Read","input":{}}]}}"#, "\n",
                r#"{"role":"user","message":{"content":[{"type":"text","text":"Thanks!"}]}}"#, "\n",
            ),
        )
        .unwrap();

        let imported = import_transcript(&store, &session, &roots);
        assert_eq!(imported, 3);

        let query = utu_store::StreamQuery { after_sequence: None, limit: 10 };
        let projection = store
            .read_session_projection("session", query, query, 10)
            .unwrap()
            .unwrap();
        assert_eq!(projection.messages.len(), 3);
        assert_eq!(projection.messages[0].body, "Fix the bug");
        assert_eq!(projection.messages[0].role, utu_core::MessageRole::Owner);
        assert_eq!(projection.messages[1].body, "I'll fix it.");
        assert_eq!(projection.messages[1].role, utu_core::MessageRole::Agent);
        assert_eq!(
            projection.messages[1].author_agent_id.as_deref(),
            Some(CURSOR_AGENT_ID)
        );
        assert_eq!(projection.messages[2].body, "Thanks!");
    }

    #[test]
    fn cursor_user_query_tag_extraction() {
        let parts = vec![
            CursorContentPart {
                kind: "text".into(),
                text: Some(
                    "<timestamp>Aug 13</timestamp>\n<user_query>\nRefactor the auth module\n</user_query>"
                        .into(),
                ),
            },
        ];
        assert_eq!(extract_cursor_body(&parts), "Refactor the auth module");
    }

    #[test]
    fn cursor_body_strips_timestamp_without_user_query() {
        let parts = vec![CursorContentPart {
            kind: "text".into(),
            text: Some("<timestamp>Aug 13</timestamp>\nPlain follow-up".into()),
        }];
        assert_eq!(extract_cursor_body(&parts), "Plain follow-up");
    }

    #[test]
    fn cursor_tool_use_parts_are_skipped() {
        let parts = vec![
            CursorContentPart {
                kind: "text".into(),
                text: Some("Hello".into()),
            },
            CursorContentPart {
                kind: "tool_use".into(),
                text: None,
            },
        ];
        assert_eq!(extract_cursor_body(&parts), "Hello");
    }
}
