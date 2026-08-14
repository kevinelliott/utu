use std::sync::Arc;

use leptos::{prelude::*, task::spawn_local};

use crate::ipc::{
    self, DiagnosticReport, ProjectDirectory, ProjectFilePreview, SessionRecord, SessionStream,
    SyncProjectSessionsSummary, WorkspaceSnapshot,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectSummary {
    pub id: &'static str,
    pub name: &'static str,
    pub initials: &'static str,
    pub tone: &'static str,
    pub running: &'static str,
    pub waiting: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionSummary {
    pub id: &'static str,
    pub title: &'static str,
    pub agents: &'static str,
    pub freshness: &'static str,
    pub tone: &'static str,
    pub unread: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentChoice {
    pub id: &'static str,
    pub name: &'static str,
    pub initials: &'static str,
    pub tone: &'static str,
    pub provider: &'static str,
    pub model: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectorSummary {
    pub id: &'static str,
    pub name: &'static str,
    pub family: &'static str,
    pub status: &'static str,
    pub detail: &'static str,
    pub tone: &'static str,
    pub evidence: &'static str,
    pub capabilities: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileSummary {
    pub path: &'static str,
    pub state: &'static str,
    pub additions: u16,
    pub removals: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActiveWork {
    pub title: &'static str,
    pub project: &'static str,
    pub branch: &'static str,
    pub working_directory: &'static str,
    pub owner_direction: &'static str,
    pub agent_response: &'static str,
    pub permission_command: &'static str,
    pub streaming_activity: &'static str,
    pub estimated_cost: &'static str,
    pub token_usage: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanStep {
    pub label: &'static str,
    pub detail: &'static str,
    pub state: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ToolActivity {
    pub tool: &'static str,
    pub target: &'static str,
    pub result: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiffLine {
    pub number: &'static str,
    pub content: &'static str,
    pub kind: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct WorkspaceModel {
    pub active_project: &'static str,
    pub active_session: &'static str,
    pub projects: &'static [ProjectSummary],
    pub sessions: &'static [SessionSummary],
    pub agents: &'static [AgentChoice],
    pub connectors: &'static [ConnectorSummary],
    pub files: &'static [FileSummary],
    pub active_work: ActiveWork,
    pub plan: &'static [PlanStep],
    pub tools: &'static [ToolActivity],
    pub diff: &'static [DiffLine],
}

impl WorkspaceModel {
    pub const fn demo() -> Self {
        Self {
            active_project: "hometender",
            active_session: "release-handoff",
            projects: &DEMO_PROJECTS,
            sessions: &DEMO_SESSIONS,
            agents: &DEMO_AGENTS,
            connectors: &DEMO_CONNECTORS,
            files: &DEMO_FILES,
            active_work: DEMO_ACTIVE_WORK,
            plan: &DEMO_PLAN,
            tools: &DEMO_TOOLS,
            diff: &DEMO_DIFF,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum WorkspaceAction {
    SelectView(&'static str),
    SelectProject(String),
    SelectSession(String),
    SelectFile(String),
    OpenCreateProject,
    OpenCreateTask(String),
    CreateProject {
        name: String,
        root_path: String,
    },
    CreateTask {
        project_id: String,
        title: String,
        detail: String,
        assignee_agent_ids: Vec<String>,
    },
    AssignAgents(Vec<&'static str>),
    SubmitPrompt {
        project_id: Option<String>,
        session_id: Option<String>,
        body: String,
        allow_provider_delivery: bool,
    },
    ResolvePermission(&'static str),
    RefreshConnector(String),
    ConfigureConnector(String),
    SyncProjectSessions {
        project_id: Option<String>,
    },
}

#[derive(Clone, Copy)]
pub struct WorkspaceActionSink(pub Callback<WorkspaceAction>);

impl WorkspaceActionSink {
    pub fn dispatch(&self, action: WorkspaceAction) {
        self.0.run(action);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadPhase {
    Demo,
    Loading,
    Ready,
    Empty,
    Error,
}

impl LoadPhase {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Demo => "Demo",
            Self::Loading => "Connecting",
            Self::Ready => "Live local",
            Self::Empty => "Local store empty",
            Self::Error => "Connection problem",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LiveStatus {
    pub surface: ipc::RuntimeSurface,
    pub phase: RwSignal<LoadPhase>,
    pub snapshot: RwSignal<Option<Arc<WorkspaceSnapshot>>>,
    pub diagnostics: RwSignal<Option<Arc<DiagnosticReport>>>,
    pub selected_project_id: RwSignal<Option<String>>,
    pub selected_session_id: RwSignal<Option<String>>,
    pub selected_connector_id: RwSignal<Option<String>>,
    pub selected_file_path: RwSignal<Option<String>>,
    pub requested_directory_path: RwSignal<Option<String>>,
    pub connector_refreshing: RwSignal<bool>,
    pub session_syncing: RwSignal<bool>,
    pub project_creating: RwSignal<bool>,
    pub task_creating: RwSignal<bool>,
    pub project_create_error: RwSignal<Option<String>>,
    pub task_create_error: RwSignal<Option<String>>,
    pub stream_loading: RwSignal<bool>,
    pub requested_stream_session_id: RwSignal<Option<String>>,
    pub session_stream: RwSignal<Option<Arc<SessionStream>>>,
    pub project_directory: RwSignal<Option<Arc<ProjectDirectory>>>,
    pub file_preview: RwSignal<Option<Arc<ProjectFilePreview>>>,
    pub error: RwSignal<Option<String>>,
}

impl LiveStatus {
    pub fn new(surface: ipc::RuntimeSurface) -> Self {
        Self {
            surface,
            phase: RwSignal::new(if surface.is_desktop() {
                LoadPhase::Loading
            } else {
                LoadPhase::Demo
            }),
            snapshot: RwSignal::new(None),
            diagnostics: RwSignal::new(None),
            selected_project_id: RwSignal::new(None),
            selected_session_id: RwSignal::new(None),
            selected_connector_id: RwSignal::new(None),
            selected_file_path: RwSignal::new(None),
            requested_directory_path: RwSignal::new(None),
            connector_refreshing: RwSignal::new(false),
            session_syncing: RwSignal::new(false),
            project_creating: RwSignal::new(false),
            task_creating: RwSignal::new(false),
            project_create_error: RwSignal::new(None),
            task_create_error: RwSignal::new(None),
            stream_loading: RwSignal::new(false),
            requested_stream_session_id: RwSignal::new(None),
            session_stream: RwSignal::new(None),
            project_directory: RwSignal::new(None),
            file_preview: RwSignal::new(None),
            error: RwSignal::new(None),
        }
    }

    pub const fn is_desktop(&self) -> bool {
        self.surface.is_desktop()
    }

    pub fn start(&self) {
        if !self.is_desktop() {
            return;
        }
        let status = *self;
        spawn_local(async move {
            match ipc::workspace_snapshot(None).await {
                Ok(snapshot) => status.accept_snapshot(snapshot),
                Err(error) => {
                    status.phase.set(LoadPhase::Error);
                    status.error.set(Some(error));
                }
            }
            // Connector probes are deliberately started only after the durable
            // snapshot is published, so a slow CLI cannot hold the app in Loading.
            status.refresh_connectors();
            ipc::listen_workspace_changed(move || {
                spawn_local(async move {
                    if let Ok(snapshot) = ipc::workspace_snapshot(None).await {
                        status.accept_snapshot(snapshot);
                    }
                    if let Ok(Some(report)) = ipc::latest_connector_report().await {
                        status.diagnostics.set(Some(Arc::new(report)));
                    }
                    if let Some(session_id) = status.selected_session_id.get_untracked() {
                        status.load_session_stream(session_id);
                    }
                });
            });
        });
    }

    pub fn refresh_connectors(&self) {
        if !self.is_desktop() || self.connector_refreshing.get_untracked() {
            return;
        }
        self.connector_refreshing.set(true);
        self.error.set(None);
        let status = *self;
        spawn_local(async move {
            match ipc::refresh_connectors().await {
                Ok(report) => {
                    status.diagnostics.set(Some(Arc::new(report)));
                    match ipc::workspace_snapshot(None).await {
                        Ok(snapshot) => status.accept_snapshot(snapshot),
                        Err(error) => status.error.set(Some(error)),
                    }
                }
                Err(error) => status.error.set(Some(error)),
            }
            status.connector_refreshing.set(false);
        });
    }

    pub fn create_project(&self, name: String, root_path: String, on_created: Callback<String>) {
        if !self.is_desktop() || self.project_creating.get_untracked() {
            return;
        }
        self.project_creating.set(true);
        self.project_create_error.set(None);
        let status = *self;
        spawn_local(async move {
            match ipc::create_project(name.trim(), root_path.trim()).await {
                Ok(project) => {
                    let project_id = project.id.clone();
                    status.selected_project_id.set(Some(project_id.clone()));
                    match ipc::workspace_snapshot(None).await {
                        Ok(snapshot) => {
                            status.accept_snapshot(snapshot);
                            status.error.set(Some(format!(
                                "Added {} to this device.",
                                project.name
                            )));
                        }
                        Err(error) => status.error.set(Some(format!(
                            "The project was added, but Utu could not refresh the workspace: {error}"
                        ))),
                    }
                    on_created.run(project_id);
                }
                Err(error) => status.project_create_error.set(Some(error)),
            }
            status.project_creating.set(false);
        });
    }

    pub fn create_task(
        &self,
        project_id: String,
        title: String,
        detail: String,
        assignee_agent_ids: Vec<String>,
        on_created: Callback<String>,
    ) {
        if !self.is_desktop() || self.task_creating.get_untracked() {
            return;
        }
        self.task_creating.set(true);
        self.task_create_error.set(None);
        let status = *self;
        spawn_local(async move {
            match ipc::create_task(
                &project_id,
                title.trim(),
                detail.trim(),
                &assignee_agent_ids,
            )
            .await
            {
                Ok(task) => {
                    status.selected_project_id.set(Some(project_id.clone()));
                    match ipc::workspace_snapshot(None).await {
                        Ok(snapshot) => {
                            status.accept_snapshot(snapshot);
                            status.error.set(Some(format!(
                                "Created task “{}” as a local draft.",
                                task.title
                            )));
                        }
                        Err(error) => status.error.set(Some(format!(
                            "The task was created, but Utu could not refresh the workspace: {error}"
                        ))),
                    }
                    on_created.run(task.id);
                }
                Err(error) => status.task_create_error.set(Some(error)),
            }
            status.task_creating.set(false);
        });
    }

    fn accept_snapshot(&self, snapshot: WorkspaceSnapshot) {
        let selected_project = self
            .selected_project_id
            .get_untracked()
            .filter(|id| snapshot.projects.iter().any(|project| &project.id == id))
            .or_else(|| snapshot.projects.first().map(|project| project.id.clone()));
        let selected_session = self
            .selected_session_id
            .get_untracked()
            .filter(|id| {
                snapshot.sessions.iter().any(|session| {
                    &session.id == id
                        && selected_project.as_deref() == Some(session.project_id.as_str())
                })
            })
            .or_else(|| {
                snapshot
                    .sessions
                    .iter()
                    .filter(|session| {
                        selected_project.as_deref() == Some(session.project_id.as_str())
                    })
                    .find(|session| snapshot.session_can_receive_direction(&session.id))
                    .or_else(|| {
                        snapshot.sessions.iter().find(|session| {
                            selected_project.as_deref() == Some(session.project_id.as_str())
                        })
                    })
                    .map(|session| session.id.clone())
            });
        self.selected_project_id.set(selected_project.clone());
        self.selected_session_id.set(selected_session.clone());
        if !snapshot.store.integrity_ok || !snapshot.store.foreign_keys_enabled {
            self.phase.set(LoadPhase::Error);
            self.error.set(Some(
                "The local store failed its integrity or foreign-key safety check. Live actions are disabled."
                    .into(),
            ));
        } else {
            self.error.set(None);
            self.phase.set(
                if snapshot.projects.is_empty()
                    && snapshot.sessions.is_empty()
                    && snapshot.agents.is_empty()
                {
                    LoadPhase::Empty
                } else {
                    LoadPhase::Ready
                },
            );
        }
        self.snapshot.set(Some(Arc::new(snapshot)));
        if let Some(project_id) = selected_project {
            self.load_project_directory(project_id, None);
        }
        if let Some(session_id) = selected_session {
            self.load_session_stream(session_id);
        } else {
            self.requested_stream_session_id.set(None);
            self.session_stream.set(None);
            self.stream_loading.set(false);
        }
    }

    pub fn load_session_stream(&self, session_id: String) {
        if !self.is_desktop() {
            return;
        }
        self.requested_stream_session_id
            .set(Some(session_id.clone()));
        self.session_stream.set(None);
        self.stream_loading.set(true);
        let status = *self;
        spawn_local(async move {
            match ipc::session_stream(&session_id).await {
                Ok(stream)
                    if status.selected_session_id.get_untracked().as_deref()
                        == Some(session_id.as_str())
                        && status
                            .requested_stream_session_id
                            .get_untracked()
                            .as_deref()
                            == Some(session_id.as_str()) =>
                {
                    status.session_stream.set(Some(Arc::new(stream)));
                    status.stream_loading.set(false);
                }
                Ok(_) => {}
                Err(error)
                    if status.selected_session_id.get_untracked().as_deref()
                        == Some(session_id.as_str())
                        && status
                            .requested_stream_session_id
                            .get_untracked()
                            .as_deref()
                            == Some(session_id.as_str()) =>
                {
                    status.stream_loading.set(false);
                    status.error.set(Some(error));
                }
                Err(_) => {}
            }
        });
    }

    pub fn send_direction(
        &self,
        project_id: Option<String>,
        session_id: Option<String>,
        body: String,
        allow_provider_delivery: bool,
    ) {
        if !self.is_desktop() {
            self.error.set(Some(
                "This web status surface is read-only. Open Utu on the owner device.".into(),
            ));
            return;
        }
        let body = body.trim().to_owned();
        if body.is_empty() {
            self.error
                .set(Some("Add a direction before assigning work.".into()));
            return;
        }
        let Some(snapshot) = self.snapshot.get_untracked() else {
            self.error
                .set(Some("The local workspace is still loading.".into()));
            return;
        };
        let live_session = session_id
            .as_deref()
            .and_then(|id| snapshot.sessions.iter().find(|session| session.id == id))
            .filter(|session| project_id.as_deref() == Some(session.project_id.as_str()))
            .filter(|session| {
                snapshot.agents.iter().any(|agent| {
                    agent.id == session.agent_id && !agent.connector_id.starts_with("demo-")
                })
            });
        let Some(session) = live_session else {
            self.error.set(Some(
                "Direction not recorded: select a stored, non-demonstration session first.".into(),
            ));
            return;
        };
        let session_id = session.id.clone();
        let status = *self;
        spawn_local(async move {
            match ipc::send_direction(&session_id, &body, allow_provider_delivery).await {
                Ok(result) => {
                    let receipt = result.receipt.message.unwrap_or_else(|| {
                        format!(
                            "Direction recorded with {} receipt.",
                            result.receipt.outcome
                        )
                    });
                    status.error.set(Some(receipt));
                    status.load_session_stream(session_id);
                }
                Err(error) => status.error.set(Some(error)),
            }
        });
    }

    pub fn sync_project_sessions(&self, project_id: Option<String>) {
        if !self.is_desktop() || self.session_syncing.get_untracked() {
            return;
        }
        self.session_syncing.set(true);
        self.error.set(None);
        let status = *self;
        spawn_local(async move {
            match ipc::sync_project_sessions(project_id.as_deref()).await {
                Ok(summary) => {
                    status.error.set(Some(format_sync_summary(&summary)));
                    match ipc::workspace_snapshot(None).await {
                        Ok(snapshot) => status.accept_snapshot(snapshot),
                        Err(error) => status.error.set(Some(error)),
                    }
                }
                Err(error) => status.error.set(Some(error)),
            }
            status.session_syncing.set(false);
        });
    }

    pub fn load_project_directory(&self, project_id: String, relative_path: Option<String>) {
        if !self.is_desktop() {
            return;
        }
        self.requested_directory_path.set(relative_path.clone());
        self.project_directory.set(None);
        let status = *self;
        spawn_local(async move {
            match ipc::project_directory(&project_id, relative_path.as_deref()).await {
                Ok(directory)
                    if status.selected_project_id.get_untracked().as_deref()
                        == Some(project_id.as_str())
                        && status.requested_directory_path.get_untracked() == relative_path =>
                {
                    status.project_directory.set(Some(Arc::new(directory)));
                }
                Ok(_) => {}
                Err(error)
                    if status.selected_project_id.get_untracked().as_deref()
                        == Some(project_id.as_str())
                        && status.requested_directory_path.get_untracked() == relative_path =>
                {
                    status.error.set(Some(error));
                }
                Err(_) => {}
            }
        });
    }

    pub fn load_file_preview(&self, project_id: String, relative_path: String) {
        if !self.is_desktop() {
            return;
        }
        self.selected_file_path.set(Some(relative_path.clone()));
        self.file_preview.set(None);
        let status = *self;
        spawn_local(async move {
            match ipc::project_file_preview(&project_id, &relative_path, Some(256 * 1024)).await {
                Ok(preview)
                    if status.selected_project_id.get_untracked().as_deref()
                        == Some(project_id.as_str())
                        && status.selected_file_path.get_untracked().as_deref()
                            == Some(relative_path.as_str()) =>
                {
                    status.file_preview.set(Some(Arc::new(preview)));
                }
                Ok(_) => {}
                Err(error)
                    if status.selected_project_id.get_untracked().as_deref()
                        == Some(project_id.as_str())
                        && status.selected_file_path.get_untracked().as_deref()
                            == Some(relative_path.as_str()) =>
                {
                    status.error.set(Some(error));
                }
                Err(_) => {}
            }
        });
    }

    pub fn active_project_name(&self) -> Option<String> {
        let snapshot = self.snapshot.get()?;
        let selected = self.selected_project_id.get();
        snapshot
            .projects
            .iter()
            .find(|project| Some(project.id.as_str()) == selected.as_deref())
            .map(|project| project.name.clone())
    }

    pub fn recordable_session_id(&self) -> Option<String> {
        let snapshot = self.snapshot.get()?;
        let project = self.selected_project_id.get()?;
        let selected = self.selected_session_id.get()?;
        snapshot
            .sessions
            .iter()
            .find(|session| session.id == selected && session.project_id == project)
            .filter(|session| {
                snapshot.agents.iter().any(|agent| {
                    agent.id == session.agent_id && !agent.connector_id.starts_with("demo-")
                })
            })
            .map(|session| session.id.clone())
    }

    pub fn selected_session_can_deliver(&self) -> bool {
        let Some(snapshot) = self.snapshot.get() else {
            return false;
        };
        let Some(session_id) = self.selected_session_id.get() else {
            return false;
        };
        snapshot
            .sessions
            .iter()
            .any(|session| session.id == session_id)
            && snapshot.session_can_receive_direction(&session_id)
    }

    /// Returns `true` when the selected session was imported from an external
    /// agent's local files (has a `provider_session_id`) and is NOT eligible
    /// for live provider delivery. Such sessions are observe-only: Utu may
    /// read and display their transcripts but must not send directions to the
    /// provider or interrupt the running session in any way.
    pub fn selected_session_is_externally_observed(&self) -> bool {
        let Some(snapshot) = self.snapshot.get() else {
            return false;
        };
        let Some(session_id) = self.selected_session_id.get() else {
            return false;
        };
        let has_provider_id = snapshot
            .sessions
            .iter()
            .find(|s| s.id == session_id)
            .is_some_and(|s| s.provider_session_id.is_some());
        has_provider_id && !snapshot.session_can_receive_direction(&session_id)
    }
}

fn format_sync_summary(summary: &SyncProjectSessionsSummary) -> String {
    let agents = summary
        .agents
        .iter()
        .map(|agent| {
            let summary = format!(
                "{} {} ({})",
                agent.display_name, agent.status, agent.imported_sessions
            );
            match agent.detail.as_deref() {
                Some(detail) if !detail.is_empty() => format!("{summary}: {detail}"),
                _ => summary,
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Session metadata sync completed: {} session records imported. {agents}.",
        summary.imported_sessions
    )
}

pub fn session_title(snapshot: &WorkspaceSnapshot, session: &SessionRecord) -> String {
    // 1. Task title — the most meaningful label when available.
    if let Some(title) = session.task_id.as_deref().and_then(|task_id| {
        snapshot
            .tasks
            .iter()
            .find(|task| task.id == task_id)
            .map(|task| task.title.clone())
    }) {
        return title;
    }
    // 2. Title extracted from the first user message in the transcript.
    if let Some(hint) = session.title_hint.as_deref().filter(|s| !s.is_empty()) {
        return hint.to_owned();
    }
    // 3. Short provider session identifier — never includes the agent CLI name.
    if let Some(provider_id) = session.provider_session_id.as_deref().filter(|s| !s.is_empty()) {
        return format!("Session {}", short_provider_id(provider_id));
    }
    "Untitled session".to_owned()
}

pub fn session_detail(
    snapshot: &WorkspaceSnapshot,
    session: &SessionRecord,
    include_project: bool,
) -> String {
    let observed = relative_unix_ms(
        snapshot.generated_at_unix_ms,
        session.last_observed_at_unix_ms,
    );
    if include_project {
        let project = snapshot
            .projects
            .iter()
            .find(|project| project.id == session.project_id)
            .map(|project| project.name.as_str())
            .unwrap_or("Unknown project");
        format!("{project} · {observed}")
    } else {
        observed
    }
}

pub fn session_state_tone(state: &str) -> &'static str {
    match state {
        "running" => "healthy",
        "waiting" => "attention",
        "problem" => "problem",
        "idle" | "offline" => "quiet",
        _ => "quiet",
    }
}

pub fn session_is_running(session: &SessionRecord) -> bool {
    session.state == "running"
}

pub fn relative_unix_ms(now_ms: u64, then_ms: Option<u64>) -> String {
    let Some(then_ms) = then_ms else {
        return "Not observed".into();
    };
    let secs = now_ms.saturating_sub(then_ms) / 1000;
    if secs < 15 {
        "Just now".into()
    } else if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86_400)
    }
}

fn short_provider_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= 8 {
        trimmed.to_owned()
    } else {
        trimmed.chars().take(8).collect()
    }
}

pub fn demo_action_notice(action: &WorkspaceAction, read_only: bool) -> String {
    if read_only {
        return "This web status surface is read-only. Open Utu on the owner device to take action.".into();
    }

    match action {
        WorkspaceAction::SelectView(_) => String::new(),
        WorkspaceAction::SelectProject(project) => {
            format!("Project selection staged: {project}. Live project state is not connected yet.")
        }
        WorkspaceAction::SelectSession(session) => {
            format!(
                "Session selection staged: {session}. Live session history is not connected yet."
            )
        }
        WorkspaceAction::SelectFile(path) => {
            format!("Selected {path} from demonstration file activity.")
        }
        WorkspaceAction::OpenCreateProject | WorkspaceAction::CreateProject { .. } => {
            "Project creation is available only in the native owner app.".into()
        }
        WorkspaceAction::OpenCreateTask(_) | WorkspaceAction::CreateTask { .. } => {
            "Task creation is available only in the native owner app.".into()
        }
        WorkspaceAction::AssignAgents(agents) => {
            if agents.is_empty() {
                "Select at least one agent before assigning work.".into()
            } else {
                format!("Demo assignment updated for {}.", agents.join(" + "))
            }
        }
        WorkspaceAction::SubmitPrompt { body, .. } => {
            if body.trim().is_empty() {
                "Add a direction before assigning work.".into()
            } else {
                "Direction staged in this prototype. A live connector will require explicit confirmation.".into()
            }
        }
        WorkspaceAction::ResolvePermission(decision) => {
            format!("Permission decision staged: {decision}. No command was executed.")
        }
        WorkspaceAction::RefreshConnector(connector) => {
            format!("Demo readiness check staged for {connector}. No provider was contacted.")
        }
        WorkspaceAction::ConfigureConnector(connector) => {
            format!(
                "Demo setup opened for {connector}. Credentials are not collected by this prototype."
            )
        }
        WorkspaceAction::SyncProjectSessions { .. } => {
            "Session synchronization is available only in the native owner app.".into()
        }
    }
}

const DEMO_PROJECTS: [ProjectSummary; 3] = [
    ProjectSummary {
        id: "hometender",
        name: "HomeTender",
        initials: "HT",
        tone: "teal",
        running: "3",
        waiting: "1",
    },
    ProjectSummary {
        id: "noctivox",
        name: "NOCTIVOX",
        initials: "NV",
        tone: "violet",
        running: "2",
        waiting: "0",
    },
    ProjectSummary {
        id: "utu",
        name: "Utu",
        initials: "UT",
        tone: "blue",
        running: "1",
        waiting: "0",
    },
];

const DEMO_SESSIONS: [SessionSummary; 3] = [
    SessionSummary {
        id: "release-handoff",
        title: "Release handoff",
        agents: "Codex + Claude",
        freshness: "now",
        tone: "healthy",
        unread: Some("2"),
    },
    SessionSummary {
        id: "auth-boundary",
        title: "Auth boundary",
        agents: "Claude",
        freshness: "18m",
        tone: "attention",
        unread: None,
    },
    SessionSummary {
        id: "ci-cleanup",
        title: "CI cleanup",
        agents: "Codex",
        freshness: "2h",
        tone: "quiet",
        unread: None,
    },
];

const DEMO_AGENTS: [AgentChoice; 3] = [
    AgentChoice {
        id: "codex",
        name: "Codex",
        initials: "CO",
        tone: "teal",
        provider: "OpenAI",
        model: "GPT-5",
    },
    AgentChoice {
        id: "claude",
        name: "Claude",
        initials: "CL",
        tone: "amber",
        provider: "Anthropic",
        model: "Sonnet",
    },
    AgentChoice {
        id: "local-reviewer",
        name: "Local reviewer",
        initials: "LR",
        tone: "violet",
        provider: "Local",
        model: "Auto",
    },
];

const CAP_CLI: [&str; 5] = ["Chat", "Files", "Commands", "Approvals", "Usage"];
const CAP_CLOUD: [&str; 3] = ["Chat", "Tasks", "Usage"];
const CAP_LOCAL: [&str; 4] = ["Chat", "Files", "Commands", "Sandbox"];

const DEMO_CONNECTORS: [ConnectorSummary; 6] = [
    ConnectorSummary {
        id: "codex-cli",
        name: "Codex CLI",
        family: "Local CLI",
        status: "Ready pattern",
        detail: "Executable and account checks represented",
        tone: "healthy",
        evidence: "Demo observed",
        capabilities: &CAP_CLI,
    },
    ConnectorSummary {
        id: "claude-code",
        name: "Claude Code",
        family: "Local CLI",
        status: "Sign-in needed",
        detail: "Authentication recovery pattern",
        tone: "attention",
        evidence: "Demo stale",
        capabilities: &CAP_CLI,
    },
    ConnectorSummary {
        id: "antigravity",
        name: "Antigravity",
        family: "Local CLI",
        status: "CLI not found",
        detail: "Install or locate the executable",
        tone: "problem",
        evidence: "Demo failed",
        capabilities: &CAP_LOCAL,
    },
    ConnectorSummary {
        id: "chatgpt-work",
        name: "ChatGPT Work",
        family: "Cloud",
        status: "Planned",
        detail: "Provider API or permissioned browser mediation",
        tone: "quiet",
        evidence: "Unsupported",
        capabilities: &CAP_CLOUD,
    },
    ConnectorSummary {
        id: "claude-work",
        name: "Claude Work",
        family: "Cloud",
        status: "Planned",
        detail: "Provider API or permissioned browser mediation",
        tone: "quiet",
        evidence: "Unsupported",
        capabilities: &CAP_CLOUD,
    },
    ConnectorSummary {
        id: "cursor",
        name: "Cursor",
        family: "Cloud",
        status: "Planned",
        detail: "Capability discovery required",
        tone: "quiet",
        evidence: "Unsupported",
        capabilities: &CAP_CLOUD,
    },
];

const DEMO_FILES: [FileSummary; 5] = [
    FileSummary {
        path: "crates/dashboard-connectors/src/readiness.rs",
        state: "Modified",
        additions: 18,
        removals: 4,
    },
    FileSummary {
        path: "src/workspace.rs",
        state: "Modified",
        additions: 8,
        removals: 4,
    },
    FileSummary {
        path: "tests/connector_auth.rs",
        state: "Added",
        additions: 42,
        removals: 0,
    },
    FileSummary {
        path: "docs/integrations.md",
        state: "Added",
        additions: 31,
        removals: 0,
    },
    FileSummary {
        path: "Cargo.lock",
        state: "Modified",
        additions: 3,
        removals: 3,
    },
];

const DEMO_ACTIVE_WORK: ActiveWork = ActiveWork {
    title: "Release handoff",
    project: "HomeTender",
    branch: "release/1.2",
    working_directory: "~/Projects/hometender",
    owner_direction: "Finish the release handoff. Have Codex implement the connector readiness checks, then ask Claude to review the permission boundary and evidence labels. Keep every provider claim honest.",
    agent_response: "I’ll add the local readiness probe behind the connector boundary, update the demonstration states, run focused tests, and hand the exact diff to Claude for an independent review.",
    permission_command: "cargo test -p utu-connectors connector_readiness",
    streaming_activity: "Codex is checking the final authentication edge case",
    estimated_cost: "~$0.18",
    token_usage: "≈ 184K tokens · partial evidence",
};

const DEMO_PLAN: [PlanStep; 4] = [
    PlanStep {
        label: "Inspect connector contracts",
        detail: "Capability and evidence boundary",
        state: "done",
    },
    PlanStep {
        label: "Implement readiness probe",
        detail: "Executable, version, and auth checks",
        state: "done",
    },
    PlanStep {
        label: "Run focused verification",
        detail: "Permission required for command",
        state: "waiting",
    },
    PlanStep {
        label: "Hand off for review",
        detail: "Claude checks claims and recovery",
        state: "queued",
    },
];

const DEMO_TOOLS: [ToolActivity; 4] = [
    ToolActivity {
        tool: "read",
        target: "crates/dashboard-connectors/src/lib.rs",
        result: "4.2 KB",
    },
    ToolActivity {
        tool: "edit",
        target: "crates/dashboard-connectors/src/readiness.rs",
        result: "+96 −12",
    },
    ToolActivity {
        tool: "write",
        target: "tests/connector_auth.rs",
        result: "+42",
    },
    ToolActivity {
        tool: "command",
        target: "cargo fmt --check",
        result: "passed",
    },
];

const DEMO_DIFF: [DiffLine; 5] = [
    DiffLine {
        number: "48",
        content: " pub enum ReadinessEvidence {",
        kind: "context",
    },
    DiffLine {
        number: "49",
        content: "-    Ready,",
        kind: "removed",
    },
    DiffLine {
        number: "49",
        content: "+    Observed(ProbeEvidence),",
        kind: "added",
    },
    DiffLine {
        number: "50",
        content: "+    Inferred { reason: String },",
        kind: "added",
    },
    DiffLine {
        number: "51",
        content: "+    Unsupported,",
        kind: "added",
    },
];
