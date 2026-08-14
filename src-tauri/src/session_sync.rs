use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use tauri::State;
use utu_connectors::{ConnectorDiagnostic, DiagnosticReport, Readiness, diagnose_known_connectors};
use utu_core::{
    Agent, AuthState, ConnectorCapabilities, EvidenceKind, Integration, IntegrationState, Project,
    ProjectState, ProviderKind, Session,
};
use utu_store::Store;

use crate::{
    agent_sessions::{
        self, CURSOR_PROVIDER_ID, ObservedSession, SessionRoots, list_all_claude_sessions,
        list_all_codex_file_sessions, list_all_cursor_sessions, list_claude_sessions,
        list_codex_file_sessions,
    },
    clock::unix_ms,
    codex_commands::{
        self, CODEX_AGENT_ID, CODEX_DIAGNOSTIC_INTEGRATION_ID, CODEX_PROVIDER_ID,
        CODEX_TRANSPORT_INTEGRATION_ID, canonical_stored_project_root, persist_thread_metadata,
    },
    codex_runtime::CodexRuntime,
    commands,
    ids::deterministic_id,
    state::AppState,
};

pub(crate) const CLAUDE_PROVIDER_ID: &str = "claude";
pub(crate) const CLAUDE_DIAGNOSTIC_INTEGRATION_ID: &str = "claude";
pub(crate) const CLAUDE_TRANSPORT_INTEGRATION_ID: &str = "claude-sessions";
pub(crate) const CLAUDE_AGENT_ID: &str = "claude-code";
pub(crate) const CURSOR_AGENT_ID: &str = "cursor-agent";
pub(crate) const CURSOR_TRANSPORT_INTEGRATION_ID: &str = "cursor-sessions";

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProjectSessionsInput {
    pub confirmed_metadata_sync: bool,
    #[serde(default)]
    pub project_ids: Vec<String>,
    #[serde(default)]
    pub all_projects: bool,
    #[serde(default)]
    pub import_transcripts: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSyncSummary {
    pub agent_id: String,
    pub display_name: String,
    pub status: String,
    pub imported_sessions: u32,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProjectSessionsSummary {
    pub metadata_only: bool,
    pub imported_sessions: u32,
    pub transcripts_imported: u32,
    pub agents: Vec<AgentSyncSummary>,
}

#[tauri::command]
pub async fn sync_project_sessions(
    state: State<'_, AppState>,
    input: SyncProjectSessionsInput,
) -> Result<SyncProjectSessionsSummary, String> {
    validate_sync_input(&input)?;
    let store = Arc::clone(&state.store);
    let runtime = Arc::clone(&state.codex);
    let roots = state.session_roots();
    tauri::async_runtime::spawn_blocking(move || {
        let _lifecycle = runtime.lock_lifecycle();
        commands::ensure_store_healthy(&store)?;
        let projects = confirmed_sync_projects(&store, &input)?;
        let report = diagnose_known_connectors();
        commands::persist_diagnostics(&store, &report)?;
        import_ready_agent_sessions(
            &store,
            &runtime,
            &roots,
            &projects,
            &report,
            true,
            input.all_projects,
        )
    })
    .await
    .map_err(|error| format!("session synchronization worker failed: {error}"))?
}

pub(crate) fn import_ready_agent_sessions(
    store: &Store,
    runtime: &CodexRuntime,
    roots: &SessionRoots,
    projects: &[Project],
    report: &DiagnosticReport,
    attach_app_server: bool,
    discover_roots: bool,
) -> Result<SyncProjectSessionsSummary, String> {
    let mut summary = SyncProjectSessionsSummary {
        metadata_only: true,
        imported_sessions: 0,
        transcripts_imported: 0,
        agents: Vec::new(),
    };
    let mut claude = AgentSyncSummary {
        agent_id: CLAUDE_AGENT_ID.into(),
        display_name: "Claude Code".into(),
        status: "skipped".into(),
        imported_sessions: 0,
        detail: Some("Claude Code is not freshly observed, ready, and authenticated".into()),
    };
    let mut codex = AgentSyncSummary {
        agent_id: CODEX_AGENT_ID.into(),
        display_name: "Codex".into(),
        status: "skipped".into(),
        imported_sessions: 0,
        detail: Some("Codex is not freshly observed, ready, and authenticated".into()),
    };
    let claude_by_root = list_all_claude_sessions(roots)?;
    let codex_by_root = list_all_codex_file_sessions(roots)?;
    let cursor_by_root = list_all_cursor_sessions(roots)?;
    let projects = if discover_roots {
        ensure_projects_for_roots(
            store,
            claude_by_root
                .keys()
                .chain(codex_by_root.keys())
                .chain(cursor_by_root.keys()),
        )?;
        store
            .list_projects()
            .map_err(|error| error.to_string())?
            .into_iter()
            .filter(|project| project.root_path.is_some())
            .collect::<Vec<_>>()
    } else {
        projects.to_vec()
    };

    if let Some(diagnostic) = diagnostic(report, CLAUDE_DIAGNOSTIC_INTEGRATION_ID) {
        match require_fresh_diagnostic(diagnostic, "Claude Code") {
            Ok(_) => {
                prepare_claude_identity(store)?;
                let mut imported = 0;
                for project in &projects {
                    imported += persist_observed_for_project(
                        store,
                        project,
                        CLAUDE_AGENT_ID,
                        "claude-session",
                        &claude_by_root,
                    )?;
                }
                activate_claude_observation(store)?;
                claude.status = "synced".into();
                claude.imported_sessions = imported;
                claude.detail = None;
                summary.imported_sessions = summary.imported_sessions.saturating_add(imported);
            }
            Err(error) => {
                deactivate_claude_observation(store)?;
                claude.status = "skipped".into();
                claude.detail = Some(error);
            }
        }
    }

    if let Some(diagnostic) = diagnostic(report, CODEX_DIAGNOSTIC_INTEGRATION_ID) {
        match require_fresh_diagnostic(diagnostic, "Codex")
            .and_then(|_| executable_path(diagnostic, "Codex"))
        {
            Ok(codex_path) => {
                if store
                    .get_agent(CODEX_AGENT_ID)
                    .map_err(|error| error.to_string())?
                    .is_none()
                {
                    prepare_codex_observation(store)?;
                }
                let mut imported: u32 = 0;
                let mut server_version = None;
                let mut app_server_error = None;
                for project in &projects {
                    let Ok(cwd) = canonical_stored_project_root(project) else {
                        continue;
                    };
                    // When the App Server is attached, prefer its thread listing
                    // over file-based session discovery. The App Server provides
                    // richer metadata and its IDs may differ from rollout file
                    // IDs, causing duplicate records if both sources are imported.
                    // Only fall back to file sessions when the App Server fails.
                    let mut app_server_ok = false;
                    if attach_app_server {
                        match runtime.connect_and_list(codex_path.clone(), &cwd) {
                            Ok((observed_version, threads)) => {
                                if let Some(expected) = server_version.as_deref()
                                    && expected != observed_version
                                {
                                    runtime.revoke_all();
                                    return Err(
                                        "Codex App Server identity changed during synchronization"
                                            .into(),
                                    );
                                }
                                if server_version.is_none() {
                                    codex_commands::prepare_codex_identity(
                                        store,
                                        &observed_version,
                                    )?;
                                    server_version = Some(observed_version.clone());
                                }
                                let persisted = persist_thread_metadata(
                                    store,
                                    &observed_version,
                                    &threads,
                                    project,
                                    &cwd,
                                )?;
                                imported =
                                    imported.saturating_add(persisted.summary.imported_sessions);
                                runtime.replace_project_authorizations(
                                    &project.id,
                                    persisted.authorizations,
                                );
                                app_server_ok = true;
                            }
                            Err(error) => {
                                app_server_error = Some(error.to_string());
                            }
                        }
                    }
                    if !app_server_ok {
                        // App Server not attached or failed — import from files.
                        imported += persist_observed_for_project(
                            store,
                            project,
                            CODEX_AGENT_ID,
                            "codex-session",
                            &codex_by_root,
                        )?;
                    }
                }
                if attach_app_server {
                    if let Some(version) = server_version {
                        codex_commands::activate_codex_transport(store, &version)?;
                        codex.detail = app_server_error.map(|error| {
                            format!("App Server synced; file fallback used for some projects: {error}")
                        });
                    } else {
                        activate_codex_observation(store)?;
                        codex.detail = app_server_error.or_else(|| {
                            Some("Codex session files were imported without App Server".into())
                        });
                    }
                }
                codex.status = "synced".into();
                codex.imported_sessions = imported;
                summary.imported_sessions = summary.imported_sessions.saturating_add(imported);
            }
            Err(error) => {
                codex_commands::deactivate_codex_transport(store)?;
                runtime.revoke_all();
                codex.status = "skipped".into();
                codex.detail = Some(error);
            }
        }
    }

    // Cursor IDE agent sessions are always imported from file system — no
    // connector readiness check is required since we only observe, never write.
    let mut cursor_summary = AgentSyncSummary {
        agent_id: CURSOR_AGENT_ID.into(),
        display_name: "Cursor".into(),
        status: "skipped".into(),
        imported_sessions: 0,
        detail: None,
    };
    if !cursor_by_root.is_empty() {
        prepare_cursor_identity(store)?;
        let mut cursor_imported: u32 = 0;
        for project in &projects {
            cursor_imported = cursor_imported.saturating_add(persist_observed_for_project(
                store,
                project,
                CURSOR_AGENT_ID,
                "cursor-session",
                &cursor_by_root,
            )?);
        }
        cursor_summary.status = "synced".into();
        cursor_summary.imported_sessions = cursor_imported;
        summary.imported_sessions = summary
            .imported_sessions
            .saturating_add(cursor_imported);
    }

    summary.agents = vec![codex, claude, cursor_summary];
    Ok(summary)
}

pub(crate) fn validate_sync_input(input: &SyncProjectSessionsInput) -> Result<(), String> {
    if !input.confirmed_metadata_sync {
        return Err("Session metadata synchronization requires explicit confirmation".into());
    }
    if input.import_transcripts {
        return Err(
            "Transcript import is not available; Utu's local store is owner-only but not encrypted"
                .into(),
        );
    }
    if input.all_projects {
        if !input.project_ids.is_empty() {
            return Err("Sync for all projects does not take a project id list".into());
        }
        return Ok(());
    }
    if input.project_ids.len() != 1 || input.project_ids[0].trim().is_empty() {
        return Err("Select exactly one local project, or sync all stored projects".into());
    }
    Ok(())
}

fn confirmed_sync_projects(
    store: &Store,
    input: &SyncProjectSessionsInput,
) -> Result<Vec<Project>, String> {
    validate_sync_input(input)?;
    if input.all_projects {
        return store
            .list_projects()
            .map_err(|error| error.to_string())?
            .into_iter()
            .filter(|project| project.root_path.is_some())
            .map(|project| {
                canonical_stored_project_root(&project)?;
                Ok(project)
            })
            .collect();
    }
    let mut ids = HashSet::new();
    let mut projects = Vec::with_capacity(input.project_ids.len());
    for project_id in &input.project_ids {
        if !ids.insert(project_id.clone()) {
            return Err(format!(
                "project `{project_id}` was selected more than once"
            ));
        }
        let project = store
            .get_project(project_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("project `{project_id}` was not found"))?;
        canonical_stored_project_root(&project)?;
        projects.push(project);
    }
    Ok(projects)
}

fn persist_claude_project(
    store: &Store,
    roots: &SessionRoots,
    project: &Project,
) -> Result<u32, String> {
    let cwd = canonical_stored_project_root(project)?;
    let sessions = list_claude_sessions(roots, &cwd)?;
    persist_observed_sessions(store, project, CLAUDE_AGENT_ID, "claude-session", &sessions)
}

fn persist_codex_file_sessions(
    store: &Store,
    roots: &SessionRoots,
    project: &Project,
    cwd: &str,
) -> Result<u32, String> {
    let sessions = list_codex_file_sessions(roots, cwd)?;
    persist_observed_sessions(store, project, CODEX_AGENT_ID, "codex-session", &sessions)
}

fn persist_observed_for_project(
    store: &Store,
    project: &Project,
    agent_id: &str,
    id_prefix: &str,
    sessions_by_root: &HashMap<String, Vec<ObservedSession>>,
) -> Result<u32, String> {
    let cwd = canonical_stored_project_root(project)?;
    persist_observed_sessions(
        store,
        project,
        agent_id,
        id_prefix,
        sessions_by_root.get(&cwd).map(Vec::as_slice).unwrap_or(&[]),
    )
}

fn ensure_projects_for_roots<'a>(
    store: &Store,
    roots: impl IntoIterator<Item = &'a String>,
) -> Result<(), String> {
    let existing = store.list_projects().map_err(|error| error.to_string())?;
    let mut used_names = existing
        .iter()
        .map(|project| project.name.clone())
        .collect::<HashSet<_>>();
    let mut known_roots = HashSet::new();
    for project in &existing {
        if let Ok(root) = canonical_stored_project_root(project) {
            known_roots.insert(root);
        }
    }
    for root in roots {
        if !agent_sessions::is_importable_project_root(root) || !known_roots.insert(root.clone()) {
            continue;
        }
        let name = project_name_for_root(root, &used_names);
        used_names.insert(name.clone());
        store
            .upsert_project(&Project {
                id: deterministic_id("project", root),
                name,
                root_path: Some(root.clone()),
                state: ProjectState::Active,
                created_at_unix_ms: unix_ms(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn project_name_for_root(root: &str, used: &HashSet<String>) -> String {
    let path = Path::new(root);
    let base = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Local project");
    if !used.contains(base) {
        return base.to_owned();
    }
    if let Some(parent) = path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
    {
        let candidate = format!("{parent}/{base}");
        if !used.contains(&candidate) {
            return candidate;
        }
    }
    root.to_owned()
}

fn persist_observed_sessions(
    store: &Store,
    project: &Project,
    agent_id: &str,
    id_prefix: &str,
    sessions: &[ObservedSession],
) -> Result<u32, String> {
    let mut imported = 0;
    for observed in sessions {
        let id = deterministic_id(id_prefix, &observed.provider_session_id);
        let existing = store.get_session(&id).map_err(|error| error.to_string())?;
        let session = Session {
            id,
            project_id: project.id.clone(),
            task_id: existing
                .as_ref()
                .and_then(|session| session.task_id.clone()),
            agent_id: agent_id.into(),
            provider_session_id: Some(observed.provider_session_id.clone()),
            state: observed.state,
            started_at_unix_ms: existing
                .as_ref()
                .map(|session| session.started_at_unix_ms)
                .unwrap_or(observed.started_at_unix_ms),
            last_observed_at_unix_ms: Some(observed.last_observed_at_unix_ms),
            title_hint: observed.title_hint.clone(),
        };
        store
            .upsert_session(&session)
            .map_err(|error| error.to_string())?;
        imported += 1;
    }
    Ok(imported)
}

fn prepare_cursor_identity(store: &Store) -> Result<(), String> {
    ensure_observation_identity(
        store,
        CURSOR_PROVIDER_ID,
        "Cursor",
        CURSOR_TRANSPORT_INTEGRATION_ID,
        "Cursor agent sessions",
        CURSOR_AGENT_ID,
        "Project-scoped Cursor agent metadata observation is not complete.",
    )
}

fn prepare_claude_identity(store: &Store) -> Result<(), String> {
    ensure_observation_identity(
        store,
        CLAUDE_PROVIDER_ID,
        "Claude Code",
        CLAUDE_TRANSPORT_INTEGRATION_ID,
        "Claude Code sessions",
        CLAUDE_AGENT_ID,
        "Project-scoped Claude Code metadata observation is not complete.",
    )
}

fn activate_claude_observation(store: &Store) -> Result<(), String> {
    let capabilities = observation_capabilities();
    store
        .activate_integration_agent(
            &observation_integration(
                CLAUDE_TRANSPORT_INTEGRATION_ID,
                CLAUDE_PROVIDER_ID,
                "Claude Code sessions",
                IntegrationState::Ready,
                None,
                capabilities,
            ),
            &Agent {
                id: CLAUDE_AGENT_ID.into(),
                provider_id: CLAUDE_PROVIDER_ID.into(),
                connector_id: CLAUDE_TRANSPORT_INTEGRATION_ID.into(),
                display_name: "Claude Code".into(),
                model: None,
                capabilities,
            },
        )
        .map_err(|error| error.to_string())
}

fn deactivate_claude_observation(store: &Store) -> Result<(), String> {
    deactivate_observation(
        store,
        CLAUDE_AGENT_ID,
        CLAUDE_TRANSPORT_INTEGRATION_ID,
        "Claude Code session files are not being observed.",
    )
}

fn prepare_codex_observation(store: &Store) -> Result<(), String> {
    ensure_observation_identity(
        store,
        CODEX_PROVIDER_ID,
        "Codex",
        CODEX_TRANSPORT_INTEGRATION_ID,
        "Codex sessions",
        CODEX_AGENT_ID,
        "Project-scoped Codex metadata observation is not complete.",
    )
}

fn activate_codex_observation(store: &Store) -> Result<(), String> {
    let capabilities = observation_capabilities();
    if let Some(agent) = store
        .get_agent(CODEX_AGENT_ID)
        .map_err(|error| error.to_string())?
        && !capabilities_subset(agent.capabilities, capabilities)
    {
        // App Server delivery is already granted; do not shrink that boundary.
        return Ok(());
    }
    store
        .activate_integration_agent(
            &observation_integration(
                CODEX_TRANSPORT_INTEGRATION_ID,
                CODEX_PROVIDER_ID,
                "Codex sessions",
                IntegrationState::Ready,
                Some("Codex sessions are observed from local files; App Server delivery is not attached.".into()),
                capabilities,
            ),
            &Agent {
                id: CODEX_AGENT_ID.into(),
                provider_id: CODEX_PROVIDER_ID.into(),
                connector_id: CODEX_TRANSPORT_INTEGRATION_ID.into(),
                display_name: "Codex".into(),
                model: None,
                capabilities,
            },
        )
        .map_err(|error| error.to_string())
}

fn deactivate_observation(
    store: &Store,
    agent_id: &str,
    integration_id: &str,
    problem: &str,
) -> Result<(), String> {
    if let Some(mut agent) = store
        .get_agent(agent_id)
        .map_err(|error| error.to_string())?
    {
        agent.capabilities = ConnectorCapabilities::default();
        store
            .upsert_agent(&agent)
            .map_err(|error| error.to_string())?;
    }
    if let Some(mut integration) = store
        .get_integration(integration_id)
        .map_err(|error| error.to_string())?
    {
        integration.state = IntegrationState::Unknown;
        integration.auth = AuthState::Unknown;
        integration.evidence = EvidenceKind::Stale;
        integration.capabilities = ConnectorCapabilities::default();
        integration.problem = Some(problem.into());
        store
            .upsert_integration(&integration)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn ensure_observation_identity(
    store: &Store,
    provider_id: &str,
    provider_name: &str,
    integration_id: &str,
    integration_name: &str,
    agent_id: &str,
    incomplete_problem: &str,
) -> Result<(), String> {
    upsert_provider(store, provider_id, provider_name)?;
    if store
        .get_integration(integration_id)
        .map_err(|error| error.to_string())?
        .is_none()
    {
        store
            .upsert_integration(&observation_integration(
                integration_id,
                provider_id,
                integration_name,
                IntegrationState::Unknown,
                Some(incomplete_problem.into()),
                ConnectorCapabilities::default(),
            ))
            .map_err(|error| error.to_string())?;
    }
    if store
        .get_agent(agent_id)
        .map_err(|error| error.to_string())?
        .is_none()
    {
        store
            .upsert_agent(&Agent {
                id: agent_id.into(),
                provider_id: provider_id.into(),
                connector_id: integration_id.into(),
                display_name: provider_name.into(),
                model: None,
                capabilities: ConnectorCapabilities::default(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn capabilities_subset(candidate: ConnectorCapabilities, boundary: ConnectorCapabilities) -> bool {
    (!candidate.observe || boundary.observe)
        && (!candidate.auth_probe || boundary.auth_probe)
        && (!candidate.direct || boundary.direct)
        && (!candidate.pause || boundary.pause)
        && (!candidate.resume || boundary.resume)
        && (!candidate.stop || boundary.stop)
        && (!candidate.logs || boundary.logs)
        && (!candidate.costs || boundary.costs)
        && (!candidate.agent_messages || boundary.agent_messages)
}

fn observation_capabilities() -> ConnectorCapabilities {
    ConnectorCapabilities {
        observe: true,
        auth_probe: true,
        direct: false,
        pause: false,
        resume: false,
        stop: false,
        logs: false,
        costs: false,
        agent_messages: false,
    }
}

fn observation_integration(
    id: &str,
    provider_id: &str,
    display_name: &str,
    state: IntegrationState,
    problem: Option<String>,
    capabilities: ConnectorCapabilities,
) -> Integration {
    Integration {
        id: id.into(),
        provider_id: Some(provider_id.into()),
        connector_key: id.into(),
        display_name: display_name.into(),
        kind: ProviderKind::LocalCli,
        state,
        auth: AuthState::Confirmed,
        evidence: EvidenceKind::Observed,
        checked_at_unix_ms: Some(unix_ms()),
        problem,
        capabilities,
    }
}

fn upsert_provider(store: &Store, id: &str, display_name: &str) -> Result<(), String> {
    store
        .upsert_provider(&utu_core::Provider {
            id: id.into(),
            display_name: display_name.into(),
            kind: ProviderKind::LocalCli,
        })
        .map_err(|error| error.to_string())
}

fn diagnostic<'a>(report: &'a DiagnosticReport, id: &str) -> Option<&'a ConnectorDiagnostic> {
    report
        .connectors
        .iter()
        .find(|diagnostic| diagnostic.descriptor.id == id)
}

fn require_fresh_diagnostic(diagnostic: &ConnectorDiagnostic, name: &str) -> Result<(), String> {
    if diagnostic.readiness != Readiness::Ready
        || diagnostic.auth.state != AuthState::Confirmed
        || diagnostic.auth.kind != EvidenceKind::Observed
        || diagnostic.installation.kind != EvidenceKind::Observed
    {
        return Err(format!(
            "{name} is not freshly observed, ready, and authenticated"
        ));
    }
    Ok(())
}

fn executable_path(
    diagnostic: &ConnectorDiagnostic,
    name: &str,
) -> Result<std::path::PathBuf, String> {
    let path = diagnostic
        .installation
        .value
        .as_ref()
        .ok_or_else(|| format!("{name} diagnostics did not return an executable path"))?
        .canonicalize()
        .map_err(|_| format!("{name} executable path is no longer available"))?;
    if !path.is_absolute() || !path.is_file() {
        return Err(format!("{name} executable path is not a file"));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };
    use utu_core::{ProjectState, Provider};

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("utu-session-sync-{}-{nonce}", std::process::id()));
            fs::create_dir_all(&path).expect("fixture");
            Self(path)
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

    fn ready_store(root: &Path) -> Store {
        let store = Store::open_in_memory().unwrap();
        store
            .upsert_provider(&Provider {
                id: CLAUDE_PROVIDER_ID.into(),
                display_name: "Claude Code".into(),
                kind: ProviderKind::LocalCli,
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
    }

    #[test]
    fn claude_and_codex_files_import_for_the_same_project() {
        let fixture = Fixture::new();
        let root = fixture.project_root();
        let canonical = root.to_string_lossy().into_owned();
        let store = ready_store(&root);
        prepare_claude_identity(&store).unwrap();
        prepare_codex_observation(&store).unwrap();
        let roots = SessionRoots::from_home(&fixture.0);
        let claude_dir = agent_sessions::claude_project_dir(&roots, &canonical);
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(
            claude_dir.join("sessions-index.json"),
            format!(
                r#"{{"entries":[{{"sessionId":"claude-1","projectPath":"{canonical}","fileMtime":1000}}]}}"#
            ),
        )
        .unwrap();
        let day = roots.codex_sessions.join("2026").join("08").join("12");
        fs::create_dir_all(&day).unwrap();
        fs::write(
            day.join("rollout.jsonl"),
            serde_json::json!({
                "type": "session_meta",
                "payload": {"id": "codex-1", "cwd": canonical}
            })
            .to_string(),
        )
        .unwrap();

        let project = store.get_project("project").unwrap().unwrap();
        persist_claude_project(&store, &roots, &project).unwrap();
        persist_codex_file_sessions(&store, &roots, &project, &canonical).unwrap();
        let sessions = store.list_sessions(Some("project")).unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(
            sessions
                .iter()
                .any(|session| session.agent_id == CLAUDE_AGENT_ID
                    && session.provider_session_id.as_deref() == Some("claude-1"))
        );
        assert!(
            sessions
                .iter()
                .any(|session| session.agent_id == CODEX_AGENT_ID
                    && session.provider_session_id.as_deref() == Some("codex-1"))
        );
    }

    #[test]
    fn all_projects_sync_rejects_a_project_id_list() {
        let error = validate_sync_input(&SyncProjectSessionsInput {
            confirmed_metadata_sync: true,
            project_ids: vec!["project".into()],
            all_projects: true,
            import_transcripts: false,
        })
        .unwrap_err();
        assert!(error.contains("does not take a project id list"));
    }

    #[test]
    fn sync_for_all_creates_projects_for_discovered_session_roots() {
        let fixture = Fixture::new();
        let stored = fixture.project_root();
        let other = fixture.0.join("other");
        fs::create_dir_all(&other).unwrap();
        let other = other.canonicalize().unwrap();
        let other_root = other.to_string_lossy().into_owned();
        let store = ready_store(&stored);
        prepare_claude_identity(&store).unwrap();
        prepare_codex_observation(&store).unwrap();
        let roots = SessionRoots::from_home(&fixture.0);
        let claude_dir = agent_sessions::claude_project_dir(&roots, &other_root);
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(
            claude_dir.join("sessions-index.json"),
            format!(
                r#"{{"entries":[{{"sessionId":"claude-other","projectPath":"{other_root}","fileMtime":1}}]}}"#
            ),
        )
        .unwrap();
        let day = roots.codex_sessions.join("2026").join("08").join("12");
        fs::create_dir_all(&day).unwrap();
        fs::write(
            day.join("rollout.jsonl"),
            serde_json::json!({
                "type": "session_meta",
                "payload": {"id": "codex-other", "cwd": other_root}
            })
            .to_string(),
        )
        .unwrap();

        let claude = list_all_claude_sessions(&roots).unwrap();
        let codex = list_all_codex_file_sessions(&roots).unwrap();
        assert_eq!(
            claude
                .get(&other_root)
                .map(|sessions| sessions.len())
                .unwrap_or(0),
            1
        );
        assert_eq!(
            codex
                .get(&other_root)
                .map(|sessions| sessions.len())
                .unwrap_or(0),
            1
        );
        ensure_projects_for_roots(&store, claude.keys().chain(codex.keys())).unwrap();
        let projects = store.list_projects().unwrap();
        let discovered = projects
            .iter()
            .find(|project| project.root_path.as_deref() == Some(other_root.as_str()))
            .expect("discovered project");
        persist_observed_for_project(
            &store,
            discovered,
            CLAUDE_AGENT_ID,
            "claude-session",
            &claude,
        )
        .unwrap();
        persist_observed_for_project(&store, discovered, CODEX_AGENT_ID, "codex-session", &codex)
            .unwrap();
        let sessions = store.list_sessions(Some(&discovered.id)).unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn preparing_claude_again_does_not_invalidate_an_activated_agent() {
        let fixture = Fixture::new();
        let store = ready_store(&fixture.project_root());
        prepare_claude_identity(&store).unwrap();
        activate_claude_observation(&store).unwrap();
        prepare_claude_identity(&store).unwrap();
        activate_claude_observation(&store).unwrap();
        let agent = store.get_agent(CLAUDE_AGENT_ID).unwrap().unwrap();
        let integration = store
            .get_integration(CLAUDE_TRANSPORT_INTEGRATION_ID)
            .unwrap()
            .unwrap();
        assert!(agent.capabilities.observe);
        assert!(agent.capabilities.auth_probe);
        assert_eq!(integration.state, IntegrationState::Ready);
    }
}
