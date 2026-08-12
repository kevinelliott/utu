use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Mutex, MutexGuard},
    time::Duration,
};

use utu_codex::{
    ClientConfig, CodexClient, CodexError, NotificationPolicy, ThreadListOptions, ThreadPage,
};

const THREAD_PAGE_LIMIT: u32 = 100;
const MAX_THREAD_PAGES: usize = 5;
const MAX_THREAD_ID_BYTES: usize = 512;
const MAX_THREAD_CWD_BYTES: usize = 16 * 1024;
const MAX_THREAD_STATUS_BYTES: usize = 128;
const MAX_SERVER_VERSION_BYTES: usize = 256;

/// One persistent, bounded App Server process for the desktop lifetime. The
/// mutex serializes the synchronous protocol client without exposing payloads
/// or stderr to application logs.
pub struct CodexRuntime {
    lifecycle: Mutex<()>,
    state: Mutex<RuntimeState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SessionAuthorization {
    project_id: String,
    canonical_root: String,
    provider_thread_id: String,
}

#[derive(Default)]
struct RuntimeState {
    client: Option<CodexClient>,
    executable: Option<PathBuf>,
    authorized_sessions: HashMap<String, SessionAuthorization>,
}

impl Default for CodexRuntime {
    fn default() -> Self {
        Self {
            lifecycle: Mutex::new(()),
            state: Mutex::new(RuntimeState::default()),
        }
    }
}

impl CodexRuntime {
    pub fn lock_lifecycle(&self) -> MutexGuard<'_, ()> {
        self.lifecycle
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    pub fn connect_and_list(
        &self,
        codex_path: PathBuf,
        cwd: &str,
    ) -> Result<(String, Vec<utu_codex::ThreadSummary>), CodexError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let executable_changed = state.executable.as_ref() != Some(&codex_path);
        if executable_changed {
            state.client.take();
            state.executable = None;
            state.authorized_sessions.clear();
        }
        if state.client.as_ref().is_none_or(CodexClient::is_closed) {
            state.authorized_sessions.clear();
            state.client = Some(CodexClient::connect(client_config(codex_path.clone()))?);
            state.executable = Some(codex_path);
        }
        let result = {
            let client_ref = state.client.as_ref().ok_or(CodexError::Closed)?;
            (|| {
                let server_version = bounded_server_version(&client_ref.server_info().user_agent)?;
                let mut threads = list_bounded_threads(client_ref, cwd)?;
                drain_events(client_ref)?;
                if client_ref.take_dropped_event_count() > 0 {
                    // The first projection may have raced a bounded queue overflow.
                    // Re-read from provider authority, then require a clean interval.
                    threads = list_bounded_threads(client_ref, cwd)?;
                    drain_events(client_ref)?;
                    if client_ref.take_dropped_event_count() > 0 {
                        return Err(CodexError::Overloaded);
                    }
                }
                Ok((server_version, threads))
            })()
        };
        if result.is_err() && state.client.as_ref().is_some_and(CodexClient::is_closed) {
            state.client.take();
            state.executable = None;
            state.authorized_sessions.clear();
        }
        result
    }

    pub fn replace_authorized_sessions(
        &self,
        authorizations: impl IntoIterator<Item = (String, String, String, String)>,
    ) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        state.authorized_sessions.clear();
        for (session_id, project_id, canonical_root, provider_thread_id) in authorizations {
            state.authorized_sessions.insert(
                session_id,
                SessionAuthorization {
                    project_id,
                    canonical_root,
                    provider_thread_id,
                },
            );
        }
    }

    pub fn revoke_all(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        state.authorized_sessions.clear();
        state.client.take();
        state.executable = None;
    }

    pub fn with_authorized_client<T>(
        &self,
        session_id: &str,
        project_id: &str,
        canonical_root: &str,
        provider_thread_id: &str,
        operation: impl FnOnce(&CodexClient) -> Result<T, CodexError>,
    ) -> Result<T, CodexError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let authorized = state
            .authorized_sessions
            .get(session_id)
            .is_some_and(|entry| {
                entry.project_id == project_id
                    && entry.canonical_root == canonical_root
                    && entry.provider_thread_id == provider_thread_id
            });
        if !authorized {
            return Err(CodexError::InvalidInput(
                "session scope is not authorized for this Codex runtime",
            ));
        }
        let result = {
            let client_ref = state.client.as_ref().ok_or(CodexError::Closed)?;
            (|| {
                if client_ref.is_closed() {
                    return Err(CodexError::Closed);
                }
                drain_events(client_ref)?;
                if client_ref.take_dropped_event_count() > 0 {
                    return Err(CodexError::Overloaded);
                }
                let operation_result = operation(client_ref);
                let drain_result = drain_events(client_ref);
                let dropped = client_ref.take_dropped_event_count();
                drain_result?;
                if dropped > 0 {
                    return Err(CodexError::Overloaded);
                }
                operation_result
            })()
        };
        if result.is_err() && state.client.as_ref().is_some_and(CodexClient::is_closed) {
            state.client.take();
            state.executable = None;
            state.authorized_sessions.clear();
        }
        result
    }

    pub fn preflight(
        &self,
        session_id: &str,
        project_id: &str,
        canonical_root: &str,
        provider_thread_id: &str,
    ) -> Result<(), CodexError> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let authorized = state
            .authorized_sessions
            .get(session_id)
            .is_some_and(|entry| {
                entry.project_id == project_id
                    && entry.canonical_root == canonical_root
                    && entry.provider_thread_id == provider_thread_id
            });
        if !authorized {
            return Err(CodexError::InvalidInput(
                "session scope is not authorized for this Codex runtime",
            ));
        }
        if state.client.as_ref().is_none_or(CodexClient::is_closed) {
            return Err(CodexError::Closed);
        }
        Ok(())
    }

    pub fn is_session_authorized(
        &self,
        session_id: &str,
        project_id: &str,
        canonical_root: &str,
        provider_thread_id: &str,
    ) -> bool {
        self.preflight(session_id, project_id, canonical_root, provider_thread_id)
            .is_ok()
    }

    #[cfg(test)]
    pub fn is_connected(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .client
            .as_ref()
            .is_some_and(|client| !client.is_closed())
    }

    #[cfg(test)]
    pub fn authorized_session_count(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .authorized_sessions
            .len()
    }

    #[cfg(test)]
    fn has_authorization_entry(
        &self,
        session_id: &str,
        project_id: &str,
        canonical_root: &str,
        provider_thread_id: &str,
    ) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .authorized_sessions
            .get(session_id)
            .is_some_and(|entry| {
                entry.project_id == project_id
                    && entry.canonical_root == canonical_root
                    && entry.provider_thread_id == provider_thread_id
            })
    }
}

fn client_config(codex_path: PathBuf) -> ClientConfig {
    ClientConfig::default()
        .command(codex_path, ["app-server", "--stdio"])
        .initialize_timeout(Duration::from_secs(10))
        .request_timeout(Duration::from_secs(15))
        .shutdown_timeout(Duration::from_millis(750))
        .message_bounds(16 * 1024 * 1024, 512 * 1024)
        .max_stderr_bytes(64 * 1024)
        .queue_bounds(64, 8)
        .notification_policy(NotificationPolicy::MetadataOnly)
}

fn drain_events(client: &CodexClient) -> Result<(), CodexError> {
    while client.try_next_event()?.is_some() {}
    Ok(())
}

fn list_bounded_threads(
    client: &CodexClient,
    cwd: &str,
) -> Result<Vec<utu_codex::ThreadSummary>, CodexError> {
    let mut cursor = None;
    let mut threads = Vec::new();
    for _ in 0..MAX_THREAD_PAGES {
        let ThreadPage {
            mut data,
            next_cursor,
            ..
        } = client.list_threads(ThreadListOptions {
            cursor: cursor.clone(),
            limit: Some(THREAD_PAGE_LIMIT),
            archived: Some(false),
            cwd: Some(cwd.to_owned()),
            ..ThreadListOptions::default()
        })?;
        if data.len() > THREAD_PAGE_LIMIT as usize {
            return Err(CodexError::Protocol(
                "thread list page exceeded requested bound",
            ));
        }
        for thread in &mut data {
            validate_thread_metadata(thread)?;
            scrub_unused_thread_metadata(thread);
        }
        threads.extend(data);
        let Some(next_cursor) = next_cursor else {
            return Ok(threads);
        };
        if cursor.as_ref() == Some(&next_cursor) {
            return Err(CodexError::Protocol("thread cursor did not advance"));
        }
        cursor = Some(next_cursor);
    }
    Ok(threads)
}

fn bounded_server_version(value: &str) -> Result<String, CodexError> {
    if value.is_empty()
        || value.len() > MAX_SERVER_VERSION_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(CodexError::Protocol("invalid server user agent"));
    }
    Ok(value.to_owned())
}

fn validate_thread_metadata(thread: &utu_codex::ThreadSummary) -> Result<(), CodexError> {
    if thread.id.is_empty()
        || thread.id.len() > MAX_THREAD_ID_BYTES
        || thread.id.chars().any(char::is_control)
    {
        return Err(CodexError::Protocol("invalid thread id metadata"));
    }
    if thread.cwd.as_deref().is_some_and(|cwd| {
        cwd.is_empty() || cwd.len() > MAX_THREAD_CWD_BYTES || cwd.chars().any(char::is_control)
    }) {
        return Err(CodexError::Protocol("invalid thread cwd metadata"));
    }
    if thread.status.as_deref().is_some_and(|status| {
        status.len() > MAX_THREAD_STATUS_BYTES || status.chars().any(char::is_control)
    }) {
        return Err(CodexError::Protocol("invalid thread status metadata"));
    }
    Ok(())
}

fn scrub_unused_thread_metadata(thread: &mut utu_codex::ThreadSummary) {
    thread.session_id = None;
    thread.name = None;
    thread.preview = None;
    thread.model_provider = None;
    thread.source_kind = None;
    thread.updated_at = None;
    thread.ephemeral = None;
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, sync::mpsc, thread};

    use super::*;

    #[test]
    fn unused_or_body_adjacent_thread_metadata_is_scrubbed_immediately() {
        let mut thread = utu_codex::ThreadSummary {
            id: "thread".into(),
            session_id: Some("provider-session".into()),
            name: Some("private title".into()),
            preview: Some("private transcript preview".into()),
            cwd: Some("/tmp/project".into()),
            model_provider: Some("private provider".into()),
            source_kind: Some("private source".into()),
            status: Some("idle".into()),
            created_at: Some(1),
            updated_at: Some(2),
            ephemeral: Some(false),
        };
        scrub_unused_thread_metadata(&mut thread);
        assert_eq!(thread.id, "thread");
        assert_eq!(thread.cwd.as_deref(), Some("/tmp/project"));
        assert_eq!(thread.status.as_deref(), Some("idle"));
        assert_eq!(thread.created_at, Some(1));
        assert!(thread.session_id.is_none());
        assert!(thread.name.is_none());
        assert!(thread.preview.is_none());
        assert!(thread.model_provider.is_none());
        assert!(thread.source_kind.is_none());
        assert!(thread.updated_at.is_none());
        assert!(thread.ephemeral.is_none());
    }

    #[test]
    fn hostile_thread_metadata_and_server_identity_are_rejected() {
        let mut thread = utu_codex::ThreadSummary {
            id: "x".repeat(MAX_THREAD_ID_BYTES + 1),
            cwd: Some("/tmp/project".into()),
            ..utu_codex::ThreadSummary::default()
        };
        assert!(validate_thread_metadata(&thread).is_err());
        thread.id = "thread".into();
        thread.cwd = Some(format!("/{}", "x".repeat(MAX_THREAD_CWD_BYTES)));
        assert!(validate_thread_metadata(&thread).is_err());
        thread.cwd = Some("/tmp/project".into());
        thread.status = Some("x".repeat(MAX_THREAD_STATUS_BYTES + 1));
        assert!(validate_thread_metadata(&thread).is_err());
        assert!(bounded_server_version(&"x".repeat(MAX_SERVER_VERSION_BYTES + 1)).is_err());
        assert!(bounded_server_version("codex-test\nsecret").is_err());
        assert_eq!(bounded_server_version("codex-test").unwrap(), "codex-test");
    }

    #[test]
    fn restart_and_single_project_resync_leave_other_sessions_unauthorized() {
        let runtime = CodexRuntime::default();
        assert_eq!(runtime.authorized_session_count(), 0);

        runtime.replace_authorized_sessions([(
            "session-a".into(),
            "project-a".into(),
            "/canonical/a".into(),
            "thread-a".into(),
        )]);
        assert!(runtime.has_authorization_entry(
            "session-a",
            "project-a",
            "/canonical/a",
            "thread-a"
        ));
        assert!(!runtime.has_authorization_entry(
            "session-b",
            "project-b",
            "/canonical/b",
            "thread-b"
        ));

        // Every explicit sync starts by dropping the prior process and leases.
        runtime.revoke_all();
        runtime.replace_authorized_sessions([(
            "session-b".into(),
            "project-b".into(),
            "/canonical/b".into(),
            "thread-b".into(),
        )]);
        assert!(!runtime.has_authorization_entry(
            "session-a",
            "project-a",
            "/canonical/a",
            "thread-a"
        ));
        assert!(runtime.has_authorization_entry(
            "session-b",
            "project-b",
            "/canonical/b",
            "thread-b"
        ));
    }

    #[test]
    fn lifecycle_lock_serializes_sync_and_refresh_reconfiguration() {
        let runtime = Arc::new(CodexRuntime::default());
        let first = runtime.lock_lifecycle();
        let (entered_tx, entered_rx) = mpsc::channel();
        let contender = Arc::clone(&runtime);
        let worker = thread::spawn(move || {
            let _second = contender.lock_lifecycle();
            entered_tx.send(()).unwrap();
        });

        assert!(entered_rx.recv_timeout(Duration::from_millis(50)).is_err());
        drop(first);
        entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("second lifecycle operation enters after release");
        worker.join().unwrap();
    }
}
