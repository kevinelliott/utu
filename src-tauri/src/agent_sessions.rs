//! Metadata-only session discovery from local agent files.
//!
//! These readers never open transcript bodies. Claude Code is observed from
//! `sessions-index.json` and jsonl filenames. Codex is observed from the first
//! `session_meta` line of a rollout file.

use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;
use utu_core::AgentState;

pub const CLAUDE_PROVIDER_ID: &str = "claude";
pub const CODEX_PROVIDER_ID: &str = "codex";
pub const CURSOR_PROVIDER_ID: &str = "cursor";
const MAX_SESSION_ID_BYTES: usize = 512;
const MAX_CWD_BYTES: usize = 16 * 1024;
const MAX_INDEX_BYTES: u64 = 2 * 1024 * 1024;
const MAX_META_LINE_BYTES: usize = 64 * 1024;
const MAX_TITLE_HINT_CHARS: usize = 80;
const MAX_TRANSCRIPT_FIRST_LINE_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionRoots {
    pub claude_projects: PathBuf,
    pub codex_sessions: PathBuf,
    pub codex_auth: PathBuf,
    pub cursor_projects: PathBuf,
}

impl SessionRoots {
    pub fn from_home(home: impl AsRef<Path>) -> Self {
        let home = home.as_ref();
        Self {
            claude_projects: home.join(".claude").join("projects"),
            codex_sessions: home.join(".codex").join("sessions"),
            codex_auth: home.join(".codex").join("auth.json"),
            cursor_projects: home.join(".cursor").join("projects"),
        }
    }

    pub fn from_env() -> Self {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/"));
        Self::from_home(home)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedSession {
    pub provider_id: &'static str,
    pub provider_session_id: String,
    pub canonical_cwd: String,
    pub started_at_unix_ms: u64,
    pub last_observed_at_unix_ms: u64,
    pub state: AgentState,
    pub title_hint: Option<String>,
}

pub fn claude_project_dirname(canonical_root: &str) -> String {
    canonical_root
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

pub fn claude_project_dir(roots: &SessionRoots, canonical_root: &str) -> PathBuf {
    roots
        .claude_projects
        .join(claude_project_dirname(canonical_root))
}

pub fn list_claude_sessions(
    roots: &SessionRoots,
    canonical_root: &str,
) -> Result<Vec<ObservedSession>, String> {
    let project_dir = claude_project_dir(roots, canonical_root);
    if !project_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut sessions = Vec::new();
    let index_path = project_dir.join("sessions-index.json");
    if index_path.is_file() {
        sessions.extend(parse_claude_index(&index_path, canonical_root)?);
    }
    let mut seen: std::collections::HashSet<String> = sessions
        .iter()
        .map(|session| session.provider_session_id.clone())
        .collect();
    for entry in read_dir_files(&project_dir)? {
        let Some(stem) = entry.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if entry.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        if !valid_provider_id(stem) || !seen.insert(stem.to_owned()) {
            continue;
        }
        let observed_at = file_mtime_ms(&entry).unwrap_or_else(unix_ms);
        sessions.push(ObservedSession {
            provider_id: CLAUDE_PROVIDER_ID,
            provider_session_id: stem.to_owned(),
            canonical_cwd: canonical_root.to_owned(),
            started_at_unix_ms: observed_at,
            last_observed_at_unix_ms: observed_at,
            state: claude_state_from_mtime(observed_at),
            title_hint: None,
        });
    }
    Ok(sessions)
}

pub fn list_codex_file_sessions(
    roots: &SessionRoots,
    canonical_root: &str,
) -> Result<Vec<ObservedSession>, String> {
    Ok(list_all_codex_file_sessions(roots)?
        .remove(canonical_root)
        .unwrap_or_default())
}

pub fn list_all_claude_sessions(
    roots: &SessionRoots,
) -> Result<HashMap<String, Vec<ObservedSession>>, String> {
    let mut by_root: HashMap<String, Vec<ObservedSession>> = HashMap::new();
    if !roots.claude_projects.is_dir() {
        return Ok(by_root);
    }
    for entry in read_dir_dirs(&roots.claude_projects)? {
        let encoded = entry
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if encoded.is_empty() {
            continue;
        }
        let resolved = resolve_claude_encoded_dirname(encoded);
        let index_path = entry.join("sessions-index.json");
        let mut seen = HashSet::new();
        if index_path.is_file() {
            for session in parse_claude_index_entries(&index_path)? {
                seen.insert(session.provider_session_id.clone());
                let cwd = session
                    .project_path
                    .as_deref()
                    .and_then(canonical_path)
                    .or_else(|| resolved.clone());
                let Some(cwd) = cwd else {
                    continue;
                };
                by_root
                    .entry(cwd.clone())
                    .or_default()
                    .push(ObservedSession {
                        provider_id: CLAUDE_PROVIDER_ID,
                        provider_session_id: session.provider_session_id,
                        canonical_cwd: cwd,
                        started_at_unix_ms: session.started_at_unix_ms,
                        last_observed_at_unix_ms: session.last_observed_at_unix_ms,
                        state: claude_state_from_mtime(session.last_observed_at_unix_ms),
                        title_hint: None,
                    });
            }
        }
        let Some(cwd) = resolved else {
            continue;
        };
        for file in read_dir_files(&entry)? {
            let Some(stem) = file.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            if file.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                continue;
            }
            if !valid_provider_id(stem) || !seen.insert(stem.to_owned()) {
                continue;
            }
            let observed_at = file_mtime_ms(&file).unwrap_or_else(unix_ms);
            by_root
                .entry(cwd.clone())
                .or_default()
                .push(ObservedSession {
                    provider_id: CLAUDE_PROVIDER_ID,
                    provider_session_id: stem.to_owned(),
                    canonical_cwd: cwd.clone(),
                    started_at_unix_ms: observed_at,
                    last_observed_at_unix_ms: observed_at,
                    state: claude_state_from_mtime(observed_at),
                    title_hint: None,
                });
        }
    }
    Ok(by_root)
}

pub fn list_all_codex_file_sessions(
    roots: &SessionRoots,
) -> Result<HashMap<String, Vec<ObservedSession>>, String> {
    let mut by_root: HashMap<String, Vec<ObservedSession>> = HashMap::new();
    if !roots.codex_sessions.is_dir() {
        return Ok(by_root);
    }
    collect_codex_rollouts(&roots.codex_sessions, &mut by_root)?;
    Ok(by_root)
}

pub fn is_importable_project_root(canonical: &str) -> bool {
    let path = Path::new(canonical);
    if !path.is_absolute() || !path.is_dir() || path.parent().is_none() {
        return false;
    }
    !matches!(
        canonical,
        "/tmp" | "/private/tmp" | "/var/tmp" | "/private/var/tmp"
    )
}

pub fn watched_claude_paths(roots: &SessionRoots, canonical_roots: &[String]) -> Vec<PathBuf> {
    let mut paths = vec![roots.claude_projects.clone()];
    for root in canonical_roots {
        paths.push(claude_project_dir(roots, root));
    }
    paths
}

pub fn watched_codex_paths(roots: &SessionRoots) -> Vec<PathBuf> {
    vec![roots.codex_sessions.clone(), roots.codex_auth.clone()]
}

/// Cursor encodes project paths by replacing all non-alphanumeric characters
/// with `-`, then stripping leading hyphens. The leading `/` in an absolute
/// path becomes a leading `-` under Claude's scheme, but Cursor omits it.
/// For example `/Users/kevin/Projects/utu` → `Users-kevin-Projects-utu`.
pub fn cursor_project_dirname(canonical_root: &str) -> String {
    canonical_root
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_start_matches('-')
        .to_owned()
}

pub fn cursor_project_dir(roots: &SessionRoots, canonical_root: &str) -> PathBuf {
    roots
        .cursor_projects
        .join(cursor_project_dirname(canonical_root))
}

pub fn list_all_cursor_sessions(
    roots: &SessionRoots,
) -> Result<HashMap<String, Vec<ObservedSession>>, String> {
    let mut by_root: HashMap<String, Vec<ObservedSession>> = HashMap::new();
    if !roots.cursor_projects.is_dir() {
        return Ok(by_root);
    }
    for project_dir in read_dir_dirs(&roots.cursor_projects)? {
        let encoded = project_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if encoded.is_empty() {
            continue;
        }
        let transcripts_dir = project_dir.join("agent-transcripts");
        if !transcripts_dir.is_dir() {
            continue;
        }
        let cwd = resolve_cursor_encoded_dirname(encoded);
        for uuid_dir in read_dir_dirs(&transcripts_dir)? {
            let uuid_name = uuid_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_owned();
            if uuid_name.is_empty() || !valid_provider_id(&uuid_name) {
                continue;
            }
            let jsonl_path = uuid_dir.join(format!("{uuid_name}.jsonl"));
            if !jsonl_path.is_file() {
                continue;
            }
            let file_mtime = file_mtime_ms(&jsonl_path).unwrap_or_else(unix_ms);
            let title_hint = extract_cursor_title_hint(&jsonl_path);
            let session_cwd = cwd.clone().unwrap_or_else(|| encoded.to_owned());
            by_root
                .entry(session_cwd.clone())
                .or_default()
                .push(ObservedSession {
                    provider_id: CURSOR_PROVIDER_ID,
                    provider_session_id: uuid_name,
                    canonical_cwd: session_cwd,
                    started_at_unix_ms: file_mtime,
                    last_observed_at_unix_ms: file_mtime,
                    state: cursor_state_from_mtime(file_mtime),
                    title_hint,
                });
        }
    }
    Ok(by_root)
}

/// Extract a human-readable title from the first user message in a Cursor
/// `.jsonl` transcript. Cursor transcripts are newline-delimited JSON; the
/// first line that contains a user message is a good title source.
fn extract_cursor_title_hint(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    for _ in 0..20 {
        let mut line_bytes = Vec::new();
        let read = reader.read_until(b'\n', &mut line_bytes).ok()?;
        if read == 0 || line_bytes.len() > MAX_TRANSCRIPT_FIRST_LINE_BYTES {
            break;
        }
        let line = String::from_utf8(line_bytes).ok()?;
        if let Some(text) = extract_user_text_from_cursor_line(&line) {
            return Some(truncate_title(&text));
        }
    }
    None
}

fn extract_user_text_from_cursor_line(line: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let role = v.get("role").and_then(|r| r.as_str());
    if role == Some("user") {
        // Cursor stores content under message.content (nested) or top-level content.
        let content = v
            .get("message")
            .and_then(|m| m.get("content"))
            .or_else(|| v.get("content"));
        if let Some(content) = content {
            let raw = if let Some(s) = content.as_str() {
                s.to_owned()
            } else if let Some(arr) = content.as_array() {
                arr.iter()
                    .filter_map(|part| {
                        if part.get("type").and_then(|t| t.as_str()) == Some("text") {
                            part.get("text").and_then(|t| t.as_str()).map(|s| s.to_owned())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                String::new()
            };
            if !raw.trim().is_empty() {
                let text = extract_user_query_tag(&raw).unwrap_or_else(|| strip_xml_tags(&raw));
                if !text.trim().is_empty() {
                    return Some(text);
                }
            }
        }
    }
    // Also try a top-level `body` field containing a `<user_query>` tag.
    if let Some(body) = v.get("body").and_then(|b| b.as_str()) {
        if let Some(text) = extract_user_query_tag(body) {
            return Some(text);
        }
    }
    None
}

/// Extracts the content of the first `<user_query>` tag from a string.
fn extract_user_query_tag(text: &str) -> Option<String> {
    let start = text.find("<user_query>")?;
    let after = &text[start + "<user_query>".len()..];
    let end = after.find("</user_query>").unwrap_or(after.len());
    let extracted = after[..end].trim().to_owned();
    if extracted.is_empty() {
        None
    } else {
        Some(extracted)
    }
}

/// Strips XML/HTML-like tags from text, returning only the text content.
fn strip_xml_tags(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => result.push(c),
            _ => {}
        }
    }
    result.trim().to_owned()
}

fn truncate_title(text: &str) -> String {
    let trimmed = text.trim();
    let collected: String = trimmed.chars().take(MAX_TITLE_HINT_CHARS).collect();
    if collected.len() < trimmed.chars().count() {
        format!("{}…", collected.trim_end())
    } else {
        collected
    }
}

fn cursor_state_from_mtime(last_observed_at_unix_ms: u64) -> AgentState {
    if unix_ms().saturating_sub(last_observed_at_unix_ms) <= 30_000 {
        AgentState::Running
    } else {
        AgentState::Idle
    }
}

pub fn watched_cursor_paths(roots: &SessionRoots) -> Vec<PathBuf> {
    vec![roots.cursor_projects.clone()]
}

fn parse_claude_index(path: &Path, canonical_root: &str) -> Result<Vec<ObservedSession>, String> {
    let mut sessions = Vec::new();
    for entry in parse_claude_index_entries(path)? {
        if let Some(project_path) = entry.project_path.as_deref()
            && canonical_path(project_path).as_deref() != Some(canonical_root)
        {
            continue;
        }
        sessions.push(ObservedSession {
            provider_id: CLAUDE_PROVIDER_ID,
            provider_session_id: entry.provider_session_id,
            canonical_cwd: canonical_root.to_owned(),
            started_at_unix_ms: entry.started_at_unix_ms,
            last_observed_at_unix_ms: entry.last_observed_at_unix_ms,
            state: claude_state_from_mtime(entry.last_observed_at_unix_ms),
            title_hint: None,
        });
    }
    Ok(sessions)
}

struct ClaudeIndexSession {
    provider_session_id: String,
    project_path: Option<String>,
    started_at_unix_ms: u64,
    last_observed_at_unix_ms: u64,
}

fn parse_claude_index_entries(path: &Path) -> Result<Vec<ClaudeIndexSession>, String> {
    let metadata = fs::metadata(path).map_err(|error| {
        format!(
            "could not read Claude session index {}: {error}",
            path.display()
        )
    })?;
    if metadata.len() > MAX_INDEX_BYTES {
        return Err("Claude session index exceeded the metadata bound".into());
    }
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "could not read Claude session index {}: {error}",
            path.display()
        )
    })?;
    let index: ClaudeSessionsIndex = serde_json::from_slice(&bytes)
        .map_err(|_| "Claude session index was not valid metadata JSON".to_owned())?;
    let mut sessions = Vec::new();
    for entry in index.entries {
        if !valid_provider_id(&entry.session_id) {
            continue;
        }
        let last_observed = entry
            .file_mtime
            .or_else(|| parse_rfc3339_utc_millis(entry.modified.as_deref().unwrap_or_default()))
            .unwrap_or_else(unix_ms);
        let started = parse_rfc3339_utc_millis(entry.created.as_deref().unwrap_or_default())
            .unwrap_or(last_observed);
        sessions.push(ClaudeIndexSession {
            provider_session_id: entry.session_id,
            project_path: entry.project_path,
            started_at_unix_ms: started.min(last_observed),
            last_observed_at_unix_ms: last_observed,
        });
    }
    Ok(sessions)
}

fn collect_codex_rollouts(
    dir: &Path,
    sessions: &mut HashMap<String, Vec<ObservedSession>>,
) -> Result<(), String> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "could not read Codex session directory {}: {error}",
                dir.display()
            ));
        }
    };
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "could not read Codex session directory {}: {error}",
                dir.display()
            )
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "could not inspect Codex session path {}: {error}",
                path.display()
            )
        })?;
        if file_type.is_dir() {
            collect_codex_rollouts(&path, sessions)?;
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        if let Some(session) = parse_codex_rollout_meta(&path)? {
            sessions
                .entry(session.canonical_cwd.clone())
                .or_default()
                .push(session);
        }
    }
    Ok(())
}

fn parse_codex_rollout_meta(path: &Path) -> Result<Option<ObservedSession>, String> {
    let line = match read_first_line_bounded(path, MAX_META_LINE_BYTES) {
        Ok(line) => line,
        Err(_) => return Ok(None),
    };
    let parsed: CodexLogLine = match serde_json::from_str(&line) {
        Ok(parsed) => parsed,
        Err(_) => return Ok(None),
    };
    if parsed.kind != "session_meta" {
        return Ok(None);
    }
    let Some(payload) = parsed.payload else {
        return Ok(None);
    };
    let Some(id) = payload.id.filter(|id| valid_provider_id(id)) else {
        return Ok(None);
    };
    let Some(cwd) = payload.cwd.filter(|cwd| valid_cwd(cwd)) else {
        return Ok(None);
    };
    let Some(canonical_cwd) = canonical_path(&cwd) else {
        return Ok(None);
    };
    let file_mtime = file_mtime_ms(path).unwrap_or_else(unix_ms);
    let started = parse_rfc3339_utc_millis(payload.timestamp.as_deref().unwrap_or_default())
        .or_else(|| parse_rfc3339_utc_millis(parsed.timestamp.as_deref().unwrap_or_default()))
        .unwrap_or(file_mtime);
    Ok(Some(ObservedSession {
        provider_id: CODEX_PROVIDER_ID,
        provider_session_id: id,
        canonical_cwd,
        started_at_unix_ms: started.min(file_mtime),
        last_observed_at_unix_ms: file_mtime,
        state: claude_state_from_mtime(file_mtime),
        title_hint: None,
    }))
}

fn claude_state_from_mtime(last_observed_at_unix_ms: u64) -> AgentState {
    if unix_ms().saturating_sub(last_observed_at_unix_ms) <= 15_000 {
        AgentState::Running
    } else {
        AgentState::Idle
    }
}

fn read_dir_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let entries = fs::read_dir(dir).map_err(|error| {
        format!(
            "could not read Claude project sessions {}: {error}",
            dir.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "could not read Claude project sessions {}: {error}",
                dir.display()
            )
        })?;
        let path = entry.path();
        if path.is_file() {
            files.push(path);
        }
    }
    Ok(files)
}

fn read_dir_dirs(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut dirs = Vec::new();
    let entries = fs::read_dir(dir)
        .map_err(|error| format!("could not read Claude projects {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!("could not read Claude projects {}: {error}", dir.display())
        })?;
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }
    Ok(dirs)
}

/// Resolves a Cursor-encoded project directory name back to a canonical path.
/// Cursor encodes paths without a leading separator, so `Users-kevin-Projects-utu`
/// corresponds to `/Users/kevin/Projects/utu`. We prepend `-` to recover the
/// Claude-style encoding before delegating to `resolve_claude_encoded_dirname`.
fn resolve_cursor_encoded_dirname(encoded: &str) -> Option<String> {
    if encoded.is_empty() {
        return None;
    }
    // Skip purely numeric or UUID-style names that Cursor uses for non-project windows.
    if encoded.chars().next().map_or(false, |c| c.is_ascii_digit()) {
        return None;
    }
    resolve_claude_encoded_dirname(&format!("-{encoded}"))
}

fn resolve_claude_encoded_dirname(encoded: &str) -> Option<String> {
    if encoded.is_empty() {
        return None;
    }
    let mut current = PathBuf::from("/");
    for _ in 0..64 {
        let current_encoded = claude_project_dirname(&current.to_string_lossy());
        if current_encoded == encoded {
            return canonical_path(&current.to_string_lossy());
        }
        if !encoded.starts_with(&current_encoded) {
            return None;
        }
        let mut best: Option<(usize, PathBuf)> = None;
        let Ok(entries) = fs::read_dir(&current) else {
            return None;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let child_encoded = claude_project_dirname(&path.to_string_lossy());
            if child_encoded == encoded || encoded.starts_with(&format!("{child_encoded}-")) {
                let score = child_encoded.len();
                if best
                    .as_ref()
                    .is_none_or(|(best_score, _)| score > *best_score)
                {
                    best = Some((score, path));
                }
            }
        }
        current = best?.1;
    }
    None
}

fn read_first_line_bounded(path: &Path, max_bytes: usize) -> Result<String, String> {
    let file = File::open(path).map_err(|error| {
        format!(
            "could not read session metadata {}: {error}",
            path.display()
        )
    })?;
    let mut reader = BufReader::new(file);
    let mut line = Vec::new();
    let read = reader.read_until(b'\n', &mut line).map_err(|error| {
        format!(
            "could not read session metadata {}: {error}",
            path.display()
        )
    })?;
    if read == 0 || line.len() > max_bytes {
        return Err("session metadata line was empty or too large".into());
    }
    if line.last() == Some(&b'\n') {
        line.pop();
        if line.last() == Some(&b'\r') {
            line.pop();
        }
    }
    String::from_utf8(line).map_err(|_| "session metadata line was not UTF-8".to_owned())
}

fn canonical_path(path: &str) -> Option<String> {
    let canonical = Path::new(path).canonicalize().ok()?;
    canonical
        .is_dir()
        .then(|| canonical.to_string_lossy().into_owned())
}

fn valid_provider_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_SESSION_ID_BYTES && !value.chars().any(char::is_control)
}

fn valid_cwd(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_CWD_BYTES && !value.chars().any(char::is_control)
}

fn file_mtime_ms(path: &Path) -> Option<u64> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    Some(
        modified
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX),
    )
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

pub fn parse_rfc3339_utc_millis(value: &str) -> Option<u64> {
    let value = value.trim();
    let value = value
        .strip_suffix('Z')
        .or_else(|| value.split('+').next())?;
    let (date, time) = value.split_once('T')?;
    let mut date_parts = date.split('-');
    let year: i32 = date_parts.next()?.parse().ok()?;
    let month: u32 = date_parts.next()?.parse().ok()?;
    let day: u32 = date_parts.next()?.parse().ok()?;
    let (hms, frac) = match time.split_once('.') {
        Some((hms, frac)) => (hms, frac),
        None => (time, "0"),
    };
    let mut time_parts = hms.split(':');
    let hour: u32 = time_parts.next()?.parse().ok()?;
    let minute: u32 = time_parts.next()?.parse().ok()?;
    let second: u32 = time_parts.next()?.parse().ok()?;
    let mut millis: u32 = 0;
    for (index, digit) in frac.chars().take(3).enumerate() {
        let value = digit.to_digit(10)?;
        millis += value * 10u32.pow(2 - index as u32);
    }
    utc_civil_to_unix_ms(year, month, day, hour, minute, second, millis)
}

fn utc_civil_to_unix_ms(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    millis: u32,
) -> Option<u64> {
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
        || millis > 999
    {
        return None;
    }
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400) as u32;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = i64::from(era) * 146_097 + i64::from(doe) - 719_468;
    let seconds = days
        .checked_mul(86_400)?
        .checked_add(i64::from(hour) * 3_600)?
        .checked_add(i64::from(minute) * 60)?
        .checked_add(i64::from(second))?;
    let millis = seconds.checked_mul(1_000)?.checked_add(i64::from(millis))?;
    u64::try_from(millis).ok()
}

#[derive(Deserialize)]
struct ClaudeSessionsIndex {
    #[serde(default)]
    entries: Vec<ClaudeIndexEntry>,
}

#[derive(Deserialize)]
struct ClaudeIndexEntry {
    #[serde(rename = "sessionId")]
    session_id: String,
    #[serde(rename = "projectPath")]
    project_path: Option<String>,
    created: Option<String>,
    modified: Option<String>,
    #[serde(rename = "fileMtime")]
    file_mtime: Option<u64>,
}

#[derive(Deserialize)]
struct CodexLogLine {
    #[serde(rename = "type")]
    kind: String,
    timestamp: Option<String>,
    payload: Option<CodexSessionMeta>,
}

#[derive(Deserialize)]
struct CodexSessionMeta {
    id: Option<String>,
    cwd: Option<String>,
    timestamp: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("utu-agent-sessions-{}-{nonce}", std::process::id()));
            fs::create_dir_all(&path).expect("fixture");
            Self(path)
        }

        fn roots(&self) -> SessionRoots {
            SessionRoots::from_home(&self.0)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn claude_index_imports_matching_project_metadata_without_prompts() {
        let fixture = Fixture::new();
        let project = fixture.0.join("repo");
        fs::create_dir_all(&project).unwrap();
        let canonical = project.canonicalize().unwrap();
        let canonical_root = canonical.to_string_lossy().into_owned();
        let roots = fixture.roots();
        let session_dir = claude_project_dir(&roots, &canonical_root);
        fs::create_dir_all(&session_dir).unwrap();
        fs::write(
            session_dir.join("sessions-index.json"),
            format!(
                r#"{{
                    "version": 1,
                    "entries": [
                        {{
                            "sessionId": "11111111-1111-1111-1111-111111111111",
                            "firstPrompt": "secret transcript preview",
                            "projectPath": "{canonical_root}",
                            "created": "2026-01-18T09:27:33.722Z",
                            "modified": "2026-01-18T09:28:45.741Z",
                            "fileMtime": 1768728525741,
                            "isSidechain": false
                        }},
                        {{
                            "sessionId": "22222222-2222-2222-2222-222222222222",
                            "projectPath": "/tmp/other-project",
                            "fileMtime": 1
                        }}
                    ]
                }}"#
            ),
        )
        .unwrap();
        fs::write(
            session_dir.join("33333333-3333-3333-3333-333333333333.jsonl"),
            "{\"type\":\"user\",\"message\":\"not imported\"}\n",
        )
        .unwrap();

        let sessions = list_claude_sessions(&roots, &canonical_root).unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(
            sessions.iter().any(
                |session| session.provider_session_id == "11111111-1111-1111-1111-111111111111"
            )
        );
        assert!(
            sessions.iter().any(
                |session| session.provider_session_id == "33333333-3333-3333-3333-333333333333"
            )
        );
        assert!(
            sessions
                .iter()
                .all(|session| session.canonical_cwd == canonical_root
                    && session.provider_id == CLAUDE_PROVIDER_ID)
        );
        let index_bytes = fs::read(session_dir.join("sessions-index.json")).unwrap();
        assert!(String::from_utf8(index_bytes).unwrap().contains("secret"));
    }

    #[test]
    fn codex_rollout_meta_matches_canonical_cwd_and_ignores_bodies() {
        let fixture = Fixture::new();
        let project = fixture.0.join("repo");
        fs::create_dir_all(&project).unwrap();
        let canonical_root = project
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let roots = fixture.roots();
        let day = roots.codex_sessions.join("2026").join("08").join("12");
        fs::create_dir_all(&day).unwrap();
        fs::write(
            day.join("rollout-matching.jsonl"),
            format!(
                "{}\n{}\n",
                serde_json::json!({
                    "timestamp": "2026-08-12T01:02:03.400Z",
                    "type": "session_meta",
                    "payload": {
                        "id": "0199eb7b-8b92-7d71-a281-02f23639a2ae",
                        "cwd": canonical_root,
                        "timestamp": "2026-08-12T01:02:03.000Z"
                    }
                }),
                serde_json::json!({
                    "type": "response_item",
                    "payload": {"type": "message", "role": "user", "content": "secret"}
                })
            ),
        )
        .unwrap();
        fs::write(
            day.join("rollout-other.jsonl"),
            serde_json::json!({
                "type": "session_meta",
                "payload": {
                    "id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                    "cwd": "/tmp/not-this-project"
                }
            })
            .to_string(),
        )
        .unwrap();

        let sessions = list_codex_file_sessions(&roots, &canonical_root).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(
            sessions[0].provider_session_id,
            "0199eb7b-8b92-7d71-a281-02f23639a2ae"
        );
        assert_eq!(sessions[0].canonical_cwd, canonical_root);
    }

    #[test]
    fn claude_dirname_encodes_non_alphanumeric_path_characters() {
        assert_eq!(
            claude_project_dirname("/Users/kevin/Projects/Hello, World"),
            "-Users-kevin-Projects-Hello--World"
        );
        assert_eq!(
            claude_project_dirname("/Users/kevin/Projects/utu"),
            "-Users-kevin-Projects-utu"
        );
    }

    #[test]
    fn cursor_dirname_strips_leading_separator_unlike_claude() {
        // Cursor drops the leading "/" separator; Claude keeps it as "-".
        assert_eq!(
            cursor_project_dirname("/Users/kevin/Projects/utu"),
            "Users-kevin-Projects-utu"
        );
        assert_eq!(
            cursor_project_dirname("/Users/kevin/Projects/Hello, World"),
            "Users-kevin-Projects-Hello--World"
        );
    }

    #[test]
    fn cursor_sessions_discovered_for_project_with_transcripts() {
        let fixture = Fixture::new();
        // Create the project directory and canonicalize it to get the real path
        // (macOS /var is a symlink to /private/var; canonicalize resolves it).
        let project = fixture.0.join("repo");
        fs::create_dir_all(&project).unwrap();
        let canonical = project.canonicalize().unwrap();
        let canonical_root = canonical.to_string_lossy().into_owned();

        // Build roots anchored to the CANONICAL fixture root so that paths
        // used for directory creation and discovery are consistent.
        let canonical_home = fixture.0.canonicalize().unwrap();
        let roots = SessionRoots::from_home(&canonical_home);

        // Create Cursor-style project dir (no leading dash) using the
        // canonical project path that the scanner will also compute.
        let encoded = cursor_project_dirname(&canonical_root);
        let uuid = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
        let transcript_dir = roots
            .cursor_projects
            .join(&encoded)
            .join("agent-transcripts")
            .join(uuid);
        fs::create_dir_all(&transcript_dir).unwrap();

        // Write a Cursor-format transcript with nested message.content.
        fs::write(
            transcript_dir.join(format!("{uuid}.jsonl")),
            serde_json::json!({
                "role": "user",
                "message": {
                    "content": [{"type": "text", "text": "<user_query>\nFix the bug\n</user_query>"}]
                }
            })
            .to_string()
                + "\n",
        )
        .unwrap();

        let sessions = list_all_cursor_sessions(&roots).unwrap();
        let project_sessions = sessions.get(&canonical_root).expect("sessions for project");
        assert_eq!(project_sessions.len(), 1);
        assert_eq!(project_sessions[0].provider_session_id, uuid);
        assert_eq!(project_sessions[0].provider_id, CURSOR_PROVIDER_ID);
        assert_eq!(project_sessions[0].canonical_cwd, canonical_root);
        assert_eq!(
            project_sessions[0].title_hint.as_deref(),
            Some("Fix the bug")
        );
    }

    #[test]
    fn cursor_title_hint_extracted_from_user_query_tag() {
        let line = serde_json::json!({
            "role": "user",
            "message": {
                "content": [{"type": "text", "text": "<timestamp>now</timestamp>\n<user_query>\nRefactor the session sync\n</user_query>"}]
            }
        })
        .to_string();
        let result = extract_user_text_from_cursor_line(&line);
        assert_eq!(result.as_deref(), Some("Refactor the session sync"));
    }

    #[test]
    fn filesystem_root_is_not_an_importable_project() {
        assert!(!is_importable_project_root("/"));
        assert!(!is_importable_project_root("/tmp"));
        assert!(!is_importable_project_root("/private/tmp"));
    }

    #[test]
    fn rfc3339_utc_parser_accepts_claude_index_timestamps() {
        assert_eq!(
            parse_rfc3339_utc_millis("2026-01-18T09:27:33.722Z"),
            Some(1_768_728_453_722)
        );
    }
}
