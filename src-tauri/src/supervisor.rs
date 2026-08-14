use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        mpsc::{self, RecvTimeoutError},
    },
    thread,
    time::Duration,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use utu_connectors::{DiagnosticReport, diagnose_known_connectors};
use utu_store::Store;

use crate::{
    agent_sessions::{SessionRoots, watched_claude_paths, watched_codex_paths, watched_cursor_paths},
    clock::unix_ms,
    codex_commands::canonical_stored_project_root,
    codex_runtime::CodexRuntime,
    commands,
    session_sync::{SyncProjectSessionsSummary, import_ready_agent_sessions},
};

pub const WORKSPACE_CHANGED_EVENT: &str = "utu-workspace-changed";
const DEBOUNCE: Duration = Duration::from_millis(250);
const DIAGNOSTIC_HEARTBEAT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceChanged {
    pub reason: String,
    pub generated_at_unix_ms: u64,
}

pub struct SessionSupervisor {
    store: Arc<Store>,
    codex: Arc<CodexRuntime>,
    roots: SessionRoots,
    app: Mutex<Option<AppHandle>>,
    last_diagnostics: Mutex<Option<Arc<DiagnosticReport>>>,
    hydrate_lock: Mutex<()>,
}

impl SessionSupervisor {
    pub fn new(store: Arc<Store>, codex: Arc<CodexRuntime>, roots: SessionRoots) -> Arc<Self> {
        Arc::new(Self {
            store,
            codex,
            roots,
            app: Mutex::new(None),
            last_diagnostics: Mutex::new(None),
            hydrate_lock: Mutex::new(()),
        })
    }

    pub fn roots(&self) -> SessionRoots {
        self.roots.clone()
    }

    pub fn last_diagnostics(&self) -> Option<Arc<DiagnosticReport>> {
        self.last_diagnostics
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }

    pub fn attach_and_start(self: &Arc<Self>, app: AppHandle) {
        *self.app.lock().unwrap_or_else(|poison| poison.into_inner()) = Some(app);
        let supervisor = Arc::clone(self);
        thread::Builder::new()
            .name("utu-session-watch".into())
            .spawn(move || supervisor.run())
            .expect("session watch thread");
    }

    pub fn hydrate_all(&self) -> Result<SyncProjectSessionsSummary, String> {
        self.hydrate_projects(&self.projects_with_roots()?, true, true, true)
    }

    pub fn hydrate_project(&self, project_id: &str) -> Result<SyncProjectSessionsSummary, String> {
        let project = self
            .store
            .get_project(project_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("project `{project_id}` was not found"))?;
        self.hydrate_projects(&[project], true, true, false)
    }

    pub fn import_with_report(
        &self,
        report: &DiagnosticReport,
    ) -> Result<SyncProjectSessionsSummary, String> {
        self.cache_diagnostics(report.clone());
        let projects = self
            .store
            .list_projects()
            .map_err(|error| error.to_string())?
            .into_iter()
            .filter(|project| project.root_path.is_some())
            .collect::<Vec<_>>();
        let _lifecycle = self.codex.lock_lifecycle();
        let _guard = self
            .hydrate_lock
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let summary = import_ready_agent_sessions(
            &self.store,
            &self.codex,
            &self.roots,
            &projects,
            report,
            true,
            true,
        )?;
        self.emit("sessions");
        Ok(summary)
    }

    fn hydrate_projects(
        &self,
        projects: &[utu_core::Project],
        diagnose: bool,
        attach_app_server: bool,
        discover_roots: bool,
    ) -> Result<SyncProjectSessionsSummary, String> {
        let _lifecycle = self.codex.lock_lifecycle();
        let _guard = self
            .hydrate_lock
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let report = if diagnose {
            let report = diagnose_known_connectors();
            commands::persist_diagnostics(&self.store, &report)?;
            self.cache_diagnostics(report.clone());
            self.emit("diagnostics");
            report
        } else if let Some(report) = self.last_diagnostics() {
            (*report).clone()
        } else {
            let report = diagnose_known_connectors();
            commands::persist_diagnostics(&self.store, &report)?;
            self.cache_diagnostics(report.clone());
            report
        };
        let summary = import_ready_agent_sessions(
            &self.store,
            &self.codex,
            &self.roots,
            projects,
            &report,
            attach_app_server,
            discover_roots,
        )?;
        self.emit("sessions");
        Ok(summary)
    }

    fn run(self: Arc<Self>) {
        let _ = self.hydrate_all();
        let (tx, rx) = mpsc::channel();
        let mut watcher = match notify::recommended_watcher(move |event| {
            let _ = tx.send(event);
        }) {
            Ok(watcher) => watcher,
            Err(_) => {
                self.run_heartbeat_only();
                return;
            }
        };
        self.watch_current_paths(&mut watcher);
        let mut last_heartbeat = std::time::Instant::now();
        loop {
            match rx.recv_timeout(DEBOUNCE) {
                Ok(_) => {
                    while rx.try_recv().is_ok() {}
                    self.watch_current_paths(&mut watcher);
                    let projects = self.projects_with_roots().unwrap_or_default();
                    let _ = self.hydrate_projects(&projects, false, false, true);
                }
                Err(RecvTimeoutError::Timeout) => {
                    if last_heartbeat.elapsed() >= DIAGNOSTIC_HEARTBEAT {
                        last_heartbeat = std::time::Instant::now();
                        let projects = self.projects_with_roots().unwrap_or_default();
                        let _ = self.hydrate_projects(&projects, true, false, true);
                        self.watch_current_paths(&mut watcher);
                    }
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    fn run_heartbeat_only(self: Arc<Self>) {
        loop {
            thread::sleep(DIAGNOSTIC_HEARTBEAT);
            let projects = self.projects_with_roots().unwrap_or_default();
            let _ = self.hydrate_projects(&projects, true, false, true);
        }
    }

    fn watch_current_paths(&self, watcher: &mut notify::RecommendedWatcher) {
        use notify::{RecursiveMode, Watcher};

        let canonical_roots = self.canonical_project_roots();
        let mut paths = watched_claude_paths(&self.roots, &canonical_roots);
        paths.extend(watched_codex_paths(&self.roots));
        paths.extend(watched_cursor_paths(&self.roots, &canonical_roots));
        for path in paths {
            let mode = if path == self.roots.codex_sessions {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            };
            let _ = watcher.watch(&path, mode);
        }
    }

    fn projects_with_roots(&self) -> Result<Vec<utu_core::Project>, String> {
        Ok(self
            .store
            .list_projects()
            .map_err(|error| error.to_string())?
            .into_iter()
            .filter(|project| project.root_path.is_some())
            .collect())
    }

    fn canonical_project_roots(&self) -> Vec<String> {
        self.store
            .list_projects()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|project| canonical_stored_project_root(&project).ok())
            .collect()
    }

    fn cache_diagnostics(&self, report: DiagnosticReport) {
        *self
            .last_diagnostics
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(Arc::new(report));
    }

    fn emit(&self, reason: &str) {
        let Some(app) = self
            .app
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
        else {
            return;
        };
        let _ = app.emit(
            WORKSPACE_CHANGED_EVENT,
            WorkspaceChanged {
                reason: reason.to_owned(),
                generated_at_unix_ms: unix_ms(),
            },
        );
    }
}

pub fn path_is_session_source(roots: &SessionRoots, path: &Path) -> bool {
    path_is_under(&roots.claude_projects, path) || path_is_under(&roots.codex_sessions, path)
}

fn path_is_under(root: &PathBuf, path: &Path) -> bool {
    path.starts_with(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_source_paths_are_recognized() {
        let roots = SessionRoots::from_home("/tmp/utu-home");
        assert!(path_is_session_source(
            &roots,
            &roots.claude_projects.join("project/sessions-index.json")
        ));
        assert!(path_is_session_source(
            &roots,
            &roots.codex_sessions.join("2026/rollout.jsonl")
        ));
        assert!(!path_is_session_source(
            &roots,
            Path::new("/tmp/unrelated/file")
        ));
    }
}
