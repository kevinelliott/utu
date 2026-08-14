use std::{
    path::Path,
    sync::{Arc, mpsc},
};

use serde::{Deserialize, Serialize};
use tauri::State;
use tauri_plugin_dialog::DialogExt;
use utu_connectors::{
    AdapterCapabilities, ConnectorDescriptor, DiagnosticReport, ProblemCode, Readiness,
    diagnose_known_connectors, known_connector_descriptors,
};
use utu_core::{
    Agent, AgentHandoff, AgentState, AttentionRecord, AttentionState, AuthState,
    ConnectorCapabilities, ControlAction, ControlOutcome, ControlReceipt, ControlRequest,
    CostAmount, EvidenceKind, HandoffState, Integration, IntegrationState, Message, MessageRole,
    Project, ProjectState, Provider, ProviderKind, SearchHit, Session, SessionEvent, Severity,
    Task, TaskState,
};
use utu_store::{
    NewMessage, SearchQuery, Store, StreamQuery, WorkspaceScope as StoreWorkspaceScope,
};

use crate::{
    clock::{entity_id, unix_ms},
    project_files::{
        ProjectDirectory, ProjectFilePreview, list_project_directory, preview_project_file,
    },
    state::AppState,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreStatus {
    pub schema_version: u32,
    pub latest_supported_schema_version: u32,
    pub integrity_ok: bool,
    pub foreign_keys_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCostSummary {
    pub project_id: String,
    pub amount: CostAmount,
    pub known_records: u64,
    pub unknown_records: u64,
    pub complete: bool,
}

/// Identifies the work-record scope of a snapshot. Provider integrations and
/// agents are installation-wide catalogs; every other collection is scoped to
/// `project_id` when it is present.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceScope {
    pub project_id: Option<String>,
    pub agents_are_global: bool,
    pub integrations_are_global: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub generated_at_unix_ms: u64,
    pub scope: WorkspaceScope,
    pub store: StoreStatus,
    pub projects: Vec<Project>,
    pub tasks: Vec<Task>,
    pub agents: Vec<Agent>,
    pub sessions: Vec<Session>,
    pub integrations: Vec<Integration>,
    pub attention: Vec<AttentionRecord>,
    pub handoffs: Vec<AgentHandoff>,
    pub costs: Vec<ProjectCostSummary>,
    pub provider_delivery: Vec<ProviderDeliveryEligibility>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDeliveryEligibility {
    pub session_id: String,
    pub provider_id: Option<String>,
    pub eligible: bool,
    pub requires_confirmation: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStream {
    pub session: Session,
    pub messages: Vec<Message>,
    pub events: Vec<SessionEvent>,
    pub file_changes: Vec<utu_core::FileChange>,
    pub costs: Vec<utu_core::CostRecord>,
    pub control_requests: Vec<ControlRequest>,
    pub control_receipts: Vec<ControlReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectInput {
    pub name: String,
    pub root_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskInput {
    pub project_id: String,
    pub title: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub assignee_agent_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionInput {
    pub project_id: String,
    pub task_id: Option<String>,
    pub agent_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionInput {
    pub session_id: String,
    pub body: String,
    #[serde(default)]
    pub allow_provider_delivery: bool,
}

const MAX_DIRECTION_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionResult {
    pub message: Message,
    pub request: ControlRequest,
    pub receipt: ControlReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlInput {
    pub session_id: String,
    pub action: ControlAction,
    pub instruction: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffInput {
    pub project_id: String,
    pub task_id: String,
    pub from_agent_id: String,
    pub to_agent_id: String,
    pub instruction: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchInput {
    pub text: String,
    pub project_id: Option<String>,
    pub session_id: Option<String>,
    pub limit: Option<u32>,
}

/// Destructive record removal requires the caller to echo the exact entity ID.
/// Project deletion removes Utu's local records only; it never touches project files.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteInput {
    pub id: String,
    pub confirmation: String,
}

#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    let (tx, rx) = mpsc::channel();
    app.dialog().file().pick_folder(move |path| {
        let _ = tx.send(path);
    });
    tauri::async_runtime::spawn_blocking(move || rx.recv().ok().flatten().map(|p| p.to_string()))
        .await
        .ok()
        .flatten()
}

#[tauri::command]
pub fn connector_catalog() -> Vec<ConnectorDescriptor> {
    known_connector_descriptors()
}

#[tauri::command]
pub async fn refresh_connectors(state: State<'_, AppState>) -> Result<DiagnosticReport, String> {
    let store = Arc::clone(&state.store);
    let codex = Arc::clone(&state.codex);
    let supervisor = Arc::clone(&state.supervisor);
    tauri::async_runtime::spawn_blocking(move || {
        let _lifecycle = codex.lock_lifecycle();
        ensure_store_healthy(&store)?;
        let report = diagnose_known_connectors();
        persist_diagnostics(&store, &report)?;
        drop(_lifecycle);
        let _ = supervisor.import_with_report(&report);
        Ok(report)
    })
    .await
    .map_err(|error| format!("native worker failed: {error}"))?
}

#[tauri::command]
pub fn latest_connector_report(state: State<'_, AppState>) -> Option<DiagnosticReport> {
    state
        .supervisor
        .last_diagnostics()
        .map(|report| (*report).clone())
}

#[tauri::command]
pub async fn workspace_snapshot(
    state: State<'_, AppState>,
    project_id: Option<String>,
) -> Result<WorkspaceSnapshot, String> {
    let store = Arc::clone(&state.store);
    let codex = Arc::clone(&state.codex);
    tauri::async_runtime::spawn_blocking(move || snapshot(&store, &codex, project_id.as_deref()))
        .await
        .map_err(|error| format!("native worker failed: {error}"))?
}

#[tauri::command]
pub async fn session_stream(
    state: State<'_, AppState>,
    session_id: String,
    after_message_sequence: Option<u64>,
    after_event_sequence: Option<u64>,
) -> Result<SessionStream, String> {
    run_blocking(&state, move |store| {
        let message_query = StreamQuery {
            after_sequence: after_message_sequence,
            limit: 500,
        };
        let event_query = StreamQuery {
            after_sequence: after_event_sequence,
            limit: 500,
        };
        let projection = store
            .read_session_projection(&session_id, message_query, event_query, 500)
            .map_err(store_error)?
            .ok_or_else(|| format!("session `{session_id}` was not found"))?;
        Ok(SessionStream {
            session: projection.session,
            messages: projection.messages,
            events: projection.events,
            file_changes: projection.file_changes,
            costs: projection.costs,
            control_requests: projection.control_requests,
            control_receipts: projection.control_receipts,
        })
    })
    .await
}

#[tauri::command]
pub async fn create_project(
    state: State<'_, AppState>,
    input: CreateProjectInput,
) -> Result<Project, String> {
    let supervisor = Arc::clone(&state.supervisor);
    let project = run_mutating(&state, move |store| {
        let name = required_text("project name", input.name)?;
        let root_path = input
            .root_path
            .filter(|path| !path.trim().is_empty())
            .map(|path| canonical_directory(&path))
            .transpose()?;
        let project = Project {
            id: entity_id("project"),
            name,
            root_path,
            state: ProjectState::Active,
            created_at_unix_ms: unix_ms(),
        };
        store.upsert_project(&project).map_err(store_error)?;
        Ok(project)
    })
    .await?;
    let project_id = project.id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _ = supervisor.hydrate_project(&project_id);
    });
    Ok(project)
}

#[tauri::command]
pub async fn save_project(
    state: State<'_, AppState>,
    mut project: Project,
) -> Result<Project, String> {
    run_mutating(&state, move |store| {
        project.name = required_text("project name", project.name)?;
        project.root_path = project
            .root_path
            .filter(|path| !path.trim().is_empty())
            .map(|path| canonical_directory(&path))
            .transpose()?;
        store.upsert_project(&project).map_err(store_error)?;
        Ok(project)
    })
    .await
}

#[tauri::command]
pub async fn delete_project(
    state: State<'_, AppState>,
    input: DeleteInput,
) -> Result<bool, String> {
    run_mutating(&state, move |store| {
        let id = confirmed_deletion(input)?;
        store.delete_project(&id).map_err(store_error)
    })
    .await
}

#[tauri::command]
pub async fn create_task(
    state: State<'_, AppState>,
    input: CreateTaskInput,
) -> Result<Task, String> {
    run_mutating(&state, move |store| {
        if store
            .get_project(&input.project_id)
            .map_err(store_error)?
            .is_none()
        {
            return Err(format!("project `{}` was not found", input.project_id));
        }
        let now = unix_ms();
        let assignee_agent_ids = validated_agent_ids(store, input.assignee_agent_ids)?;
        let task = Task {
            id: entity_id("task"),
            project_id: input.project_id,
            title: required_text("task title", input.title)?,
            detail: input.detail.trim().to_owned(),
            state: TaskState::Draft,
            assignee_agent_ids,
            created_at_unix_ms: now,
            updated_at_unix_ms: now,
        };
        store.upsert_task(&task).map_err(store_error)?;
        Ok(task)
    })
    .await
}

#[tauri::command]
pub async fn save_task(state: State<'_, AppState>, mut task: Task) -> Result<Task, String> {
    run_mutating(&state, move |store| {
        ensure_project(store, &task.project_id)?;
        task.title = required_text("task title", task.title)?;
        task.detail = task.detail.trim().to_owned();
        task.assignee_agent_ids = validated_agent_ids(store, task.assignee_agent_ids)?;
        task.updated_at_unix_ms = unix_ms().max(task.created_at_unix_ms);
        store.upsert_task(&task).map_err(store_error)?;
        Ok(task)
    })
    .await
}

#[tauri::command]
pub async fn delete_task(state: State<'_, AppState>, input: DeleteInput) -> Result<bool, String> {
    run_mutating(&state, move |store| {
        let id = confirmed_deletion(input)?;
        store.delete_task(&id).map_err(store_error)
    })
    .await
}

#[tauri::command]
pub async fn assign_task_agents(
    state: State<'_, AppState>,
    task_id: String,
    agent_ids: Vec<String>,
) -> Result<Task, String> {
    run_mutating(&state, move |store| {
        let agent_ids = validated_agent_ids(store, agent_ids)?;
        store
            .assign_task_agents(&task_id, &agent_ids, unix_ms())
            .map_err(store_error)
    })
    .await
}

#[tauri::command]
pub async fn delete_agent(state: State<'_, AppState>, input: DeleteInput) -> Result<bool, String> {
    run_mutating(&state, move |store| {
        let id = confirmed_deletion(input)?;
        store.delete_agent(&id).map_err(store_error)
    })
    .await
}

#[tauri::command]
pub async fn create_session(
    state: State<'_, AppState>,
    input: CreateSessionInput,
) -> Result<Session, String> {
    run_mutating(&state, move |store| {
        let session = Session {
            id: entity_id("session"),
            project_id: input.project_id,
            task_id: input.task_id,
            agent_id: input.agent_id,
            provider_session_id: None,
            state: AgentState::Idle,
            started_at_unix_ms: unix_ms(),
            last_observed_at_unix_ms: None,
            title_hint: None,
        };
        validate_session_relations(store, &session)?;
        store.upsert_session(&session).map_err(store_error)?;
        Ok(session)
    })
    .await
}

#[tauri::command]
pub async fn delete_session(
    state: State<'_, AppState>,
    input: DeleteInput,
) -> Result<bool, String> {
    run_mutating(&state, move |store| {
        let id = confirmed_deletion(input)?;
        store.delete_session(&id).map_err(store_error)
    })
    .await
}

#[tauri::command]
pub async fn send_direction(
    state: State<'_, AppState>,
    input: DirectionInput,
) -> Result<DirectionResult, String> {
    let store = Arc::clone(&state.store);
    let codex = Arc::clone(&state.codex);
    tauri::async_runtime::spawn_blocking(move || {
        let body = bounded_direction(input.body)?;
        let session = ensure_session(&store, &input.session_id)?;
        ensure_store_healthy(&store)?;
        if input.allow_provider_delivery {
            let delivery = crate::codex_commands::try_send_direction(
                &store,
                &codex,
                &session,
                body.clone(),
                true,
            )?;
            return Ok(DirectionResult {
                message: delivery.message,
                request: delivery.request,
                receipt: delivery.receipt,
            });
        }
        let now = unix_ms();
        let (request, receipt) = unsupported_control_records(
            input.session_id.clone(),
            ControlAction::Direct,
            Some(body.clone()),
            now,
        );
        let recorded = store
            .record_owner_direction(
                NewMessage {
                    id: entity_id("message"),
                    session_id: input.session_id.clone(),
                    role: MessageRole::Owner,
                    author_agent_id: None,
                    body: body.clone(),
                    sent_at_unix_ms: now,
                    ingested_at_unix_ms: now,
                    evidence: EvidenceKind::Observed,
                    source: "utu.owner".into(),
                    correlation_id: None,
                },
                request,
                receipt,
            )
            .map_err(store_error)?;
        Ok(DirectionResult {
            message: recorded.message,
            request: recorded.request,
            receipt: recorded.receipt,
        })
    })
    .await
    .map_err(|error| format!("native worker failed: {error}"))?
}

#[tauri::command]
pub async fn request_control(
    state: State<'_, AppState>,
    input: ControlInput,
) -> Result<ControlReceipt, String> {
    run_mutating(&state, move |store| {
        ensure_session(store, &input.session_id)?;
        let (_, receipt) = record_unsupported_control(
            store,
            input.session_id,
            input.action,
            input.instruction,
            unix_ms(),
        )?;
        Ok(receipt)
    })
    .await
}

#[tauri::command]
pub async fn create_handoff(
    state: State<'_, AppState>,
    input: HandoffInput,
) -> Result<AgentHandoff, String> {
    run_mutating(&state, move |store| {
        validate_handoff_relations(store, &input)?;
        let handoff = AgentHandoff {
            id: entity_id("handoff"),
            project_id: input.project_id,
            task_id: input.task_id,
            from_agent_id: input.from_agent_id,
            to_agent_id: input.to_agent_id,
            instruction: required_text("handoff instruction", input.instruction)?,
            created_at_unix_ms: unix_ms(),
            approved_by_owner: true,
            state: HandoffState::Approved,
            delivered_at_unix_ms: None,
            delivery_evidence: EvidenceKind::Unsupported,
            source: "utu.coordination".into(),
            resulting_session_id: None,
            correlation_id: None,
        };
        store.upsert_handoff(&handoff).map_err(store_error)?;
        Ok(handoff)
    })
    .await
}

#[tauri::command]
pub async fn resolve_attention(
    state: State<'_, AppState>,
    attention_id: String,
    resolved: bool,
) -> Result<AttentionRecord, String> {
    run_mutating(&state, move |store| {
        store
            .set_attention_state(
                &attention_id,
                if resolved {
                    AttentionState::Resolved
                } else {
                    AttentionState::Acknowledged
                },
                unix_ms(),
            )
            .map_err(store_error)
    })
    .await
}

#[tauri::command]
pub async fn search_workspace(
    state: State<'_, AppState>,
    input: SearchInput,
) -> Result<Vec<SearchHit>, String> {
    run_blocking(&state, move |store| {
        let text = required_text("search text", input.text)?;
        store
            .search(&SearchQuery {
                text,
                project_id: input.project_id,
                session_id: input.session_id,
                limit: input.limit.unwrap_or(100).clamp(1, 500),
            })
            .map_err(store_error)
    })
    .await
}

#[tauri::command]
pub async fn project_directory(
    state: State<'_, AppState>,
    project_id: String,
    relative_path: Option<String>,
) -> Result<ProjectDirectory, String> {
    run_blocking(&state, move |store| {
        let root = project_root(store, &project_id)?;
        list_project_directory(root, relative_path.as_deref()).map_err(|error| error.to_string())
    })
    .await
}

#[tauri::command]
pub async fn project_file_preview(
    state: State<'_, AppState>,
    project_id: String,
    relative_path: String,
    max_bytes: Option<usize>,
) -> Result<ProjectFilePreview, String> {
    run_blocking(&state, move |store| {
        let root = project_root(store, &project_id)?;
        preview_project_file(root, &relative_path, max_bytes).map_err(|error| error.to_string())
    })
    .await
}

async fn run_blocking<T, F>(state: &State<'_, AppState>, operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&Store) -> Result<T, String> + Send + 'static,
{
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || operation(&store))
        .await
        .map_err(|error| format!("native worker failed: {error}"))?
}

async fn run_mutating<T, F>(state: &State<'_, AppState>, operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&Store) -> Result<T, String> + Send + 'static,
{
    run_blocking(state, move |store| {
        ensure_store_healthy(store)?;
        operation(store)
    })
    .await
}

fn snapshot(
    store: &Store,
    runtime: &crate::codex_runtime::CodexRuntime,
    project_id: Option<&str>,
) -> Result<WorkspaceSnapshot, String> {
    let projection = store
        .read_workspace_projection(
            project_id.map_or(StoreWorkspaceScope::Global, |project_id| {
                StoreWorkspaceScope::Project(project_id.to_owned())
            }),
            "USD",
        )
        .map_err(store_error)?;
    let projection_healthy = store_health_allows_mutation(&projection.health);
    let provider_delivery = projection
        .sessions
        .iter()
        .map(|session| {
            let mut eligibility =
                crate::codex_commands::provider_delivery_eligibility_from_projection(
                    runtime,
                    session,
                    &projection.agents,
                    &projection.integrations,
                    &projection.projects,
                );
            if !projection_healthy {
                eligibility.eligible = false;
                eligibility.requires_confirmation = false;
            }
            eligibility
        })
        .collect();
    Ok(WorkspaceSnapshot {
        generated_at_unix_ms: unix_ms(),
        scope: WorkspaceScope {
            project_id: project_id.map(str::to_owned),
            agents_are_global: true,
            integrations_are_global: true,
        },
        store: StoreStatus {
            schema_version: projection.health.schema_version,
            latest_supported_schema_version: projection.health.latest_supported_schema_version,
            integrity_ok: projection.health.integrity_ok,
            foreign_keys_enabled: projection.health.foreign_keys_enabled,
        },
        projects: projection.projects,
        tasks: projection.tasks,
        agents: projection.agents,
        sessions: projection.sessions,
        integrations: projection.integrations,
        attention: projection
            .attention
            .into_iter()
            .filter(|finding| finding.state == AttentionState::Open)
            .collect(),
        handoffs: projection.handoffs,
        costs: projection
            .costs
            .into_iter()
            .map(|cost| ProjectCostSummary {
                project_id: cost.project_id,
                amount: cost.summary.amount(),
                known_records: cost.summary.known_records,
                unknown_records: cost.summary.unknown_records,
                complete: cost.summary.is_complete(),
            })
            .collect(),
        provider_delivery,
    })
}

pub(crate) fn ensure_store_healthy(store: &Store) -> Result<(), String> {
    let health = store.health().map_err(store_error)?;
    if !store_health_allows_mutation(&health) {
        return Err("Utu local store health check failed; native mutation is disabled".into());
    }
    Ok(())
}

fn store_health_allows_mutation(health: &utu_store::StoreHealth) -> bool {
    health.integrity_ok
        && health.foreign_keys_enabled
        && health.schema_version == health.latest_supported_schema_version
}

pub(crate) fn persist_diagnostics(store: &Store, report: &DiagnosticReport) -> Result<(), String> {
    for diagnostic in &report.connectors {
        let provider_id = diagnostic.descriptor.provider_id.to_owned();
        store
            .upsert_provider(&Provider {
                id: provider_id.clone(),
                display_name: diagnostic.descriptor.display_name.to_owned(),
                kind: ProviderKind::LocalCli,
            })
            .map_err(store_error)?;

        let evidence = if diagnostic.auth.state == AuthState::Confirmed {
            diagnostic.auth.kind
        } else if diagnostic.installation.value.is_none() {
            diagnostic.installation.kind
        } else if diagnostic.auth.kind == EvidenceKind::Unsupported {
            EvidenceKind::Unsupported
        } else {
            diagnostic.auth.kind
        };
        let integration = Integration {
            id: diagnostic.descriptor.id.to_owned(),
            provider_id: Some(provider_id),
            connector_key: diagnostic.descriptor.id.to_owned(),
            display_name: diagnostic.descriptor.display_name.to_owned(),
            kind: ProviderKind::LocalCli,
            state: match diagnostic.readiness {
                Readiness::Ready => IntegrationState::Ready,
                Readiness::NeedsAttention => IntegrationState::Degraded,
                Readiness::InstalledUnverified | Readiness::Unavailable => {
                    IntegrationState::Unknown
                }
            },
            auth: diagnostic.auth.state,
            evidence,
            checked_at_unix_ms: Some(report.checked_at_unix_ms),
            problem: (!diagnostic.problems.is_empty()).then(|| {
                diagnostic
                    .problems
                    .iter()
                    .map(|problem| problem.summary.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            }),
            capabilities: core_capabilities(diagnostic.descriptor.current_capabilities),
        };
        store
            .upsert_integration(&integration)
            .map_err(store_error)?;
        reconcile_connector_attention(store, report.checked_at_unix_ms, diagnostic)?;
    }
    Ok(())
}

fn reconcile_connector_attention(
    store: &Store,
    checked_at_unix_ms: u64,
    diagnostic: &utu_connectors::ConnectorDiagnostic,
) -> Result<(), String> {
    let id = format!("connector-health-{}", diagnostic.descriptor.id);
    let actionable = diagnostic
        .problems
        .iter()
        .filter(|problem| {
            matches!(
                problem.severity,
                Severity::Blocked | Severity::NeedsAttention
            )
        })
        .collect::<Vec<_>>();
    if actionable.is_empty() {
        if store.get_attention(&id).map_err(store_error)?.is_some() {
            store
                .set_attention_state(&id, AttentionState::Resolved, checked_at_unix_ms)
                .map_err(store_error)?;
        }
        return Ok(());
    }

    let severity = if actionable
        .iter()
        .any(|problem| problem.severity == Severity::Blocked)
    {
        Severity::Blocked
    } else {
        Severity::NeedsAttention
    };
    let detail = actionable
        .iter()
        .map(|problem| problem.summary.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let evidence = actionable
        .iter()
        .map(|problem| diagnostic_problem_evidence(diagnostic, problem.code))
        .max_by_key(|evidence| evidence_strength(*evidence))
        .unwrap_or(EvidenceKind::Inferred);
    let existing = store.get_attention(&id).map_err(store_error)?;
    let same_finding = existing.as_ref().is_some_and(|existing| {
        existing.severity == severity
            && existing.detail.as_deref() == Some(detail.as_str())
            && existing.evidence == evidence
    });
    let state = match existing.as_ref() {
        Some(existing) if same_finding && existing.state == AttentionState::Acknowledged => {
            AttentionState::Acknowledged
        }
        _ => AttentionState::Open,
    };
    let detected_at_unix_ms = match existing.as_ref() {
        Some(existing) if same_finding && existing.state != AttentionState::Resolved => {
            existing.detected_at_unix_ms
        }
        _ => checked_at_unix_ms,
    };
    store
        .upsert_attention(&AttentionRecord {
            id,
            project_id: None,
            task_id: None,
            session_id: None,
            agent_id: None,
            integration_id: Some(diagnostic.descriptor.id.to_owned()),
            severity,
            state,
            title: format!("{} needs attention", diagnostic.descriptor.display_name),
            detail: Some(detail),
            recovery: actionable
                .iter()
                .find_map(|problem| problem.recovery.clone()),
            detected_at_unix_ms,
            updated_at_unix_ms: checked_at_unix_ms,
            evidence,
            source: diagnostic.descriptor.id.to_owned(),
        })
        .map_err(store_error)
}

fn diagnostic_problem_evidence(
    diagnostic: &utu_connectors::ConnectorDiagnostic,
    code: ProblemCode,
) -> EvidenceKind {
    match code {
        ProblemCode::ExecutableMissing => diagnostic.installation.kind,
        ProblemCode::VersionProbeUnsupported
        | ProblemCode::VersionProbeFailed
        | ProblemCode::VersionProbeTimedOut
        | ProblemCode::VersionOutputMalformed => diagnostic.version.kind,
        ProblemCode::AuthProbeUnsupported
        | ProblemCode::AuthProbeFailed
        | ProblemCode::AuthProbeTimedOut
        | ProblemCode::AuthOutputMalformed
        | ProblemCode::AuthMissing
        | ProblemCode::AuthExpired
        | ProblemCode::AuthUnknown => diagnostic.auth.kind,
    }
}

const fn evidence_strength(evidence: EvidenceKind) -> u8 {
    match evidence {
        EvidenceKind::Observed => 3,
        EvidenceKind::Stale => 2,
        EvidenceKind::Inferred => 1,
        EvidenceKind::Unsupported => 0,
    }
}

fn core_capabilities(capabilities: AdapterCapabilities) -> ConnectorCapabilities {
    ConnectorCapabilities {
        observe: capabilities.sessions || capabilities.event_stream,
        auth_probe: capabilities.auth_probe,
        direct: capabilities.chat,
        pause: false,
        resume: false,
        stop: false,
        logs: capabilities.logs,
        costs: capabilities.costs,
        agent_messages: capabilities.chat,
    }
}

fn record_unsupported_control(
    store: &Store,
    session_id: String,
    action: ControlAction,
    instruction: Option<String>,
    now: u64,
) -> Result<(ControlRequest, ControlReceipt), String> {
    let (request, receipt) = unsupported_control_records(session_id, action, instruction, now);
    let recorded = store
        .record_control(request, receipt)
        .map_err(store_error)?;
    Ok((recorded.request, recorded.receipt))
}

fn unsupported_control_records(
    session_id: String,
    action: ControlAction,
    instruction: Option<String>,
    now: u64,
) -> (ControlRequest, ControlReceipt) {
    let request = ControlRequest {
        id: entity_id("control"),
        session_id,
        action,
        instruction,
        requested_at_unix_ms: now,
        requested_by_owner: true,
    };
    let receipt = ControlReceipt {
        id: entity_id("receipt"),
        request_id: request.id.clone(),
        outcome: ControlOutcome::Unsupported,
        received_at_unix_ms: now,
        evidence: EvidenceKind::Unsupported,
        source: "utu.connector-runtime".into(),
        message: Some(
            "No active provider control transport supports this session yet; the owner request was recorded locally."
                .into(),
        ),
        provider_receipt_id: None,
    };
    (request, receipt)
}

fn ensure_session(store: &Store, session_id: &str) -> Result<Session, String> {
    store
        .get_session(session_id)
        .map_err(store_error)?
        .ok_or_else(|| format!("session `{session_id}` was not found"))
}

fn ensure_project(store: &Store, project_id: &str) -> Result<Project, String> {
    store
        .get_project(project_id)
        .map_err(store_error)?
        .ok_or_else(|| format!("project `{project_id}` was not found"))
}

fn validated_agent_ids(store: &Store, agent_ids: Vec<String>) -> Result<Vec<String>, String> {
    let mut validated = Vec::with_capacity(agent_ids.len());
    for agent_id in agent_ids {
        let agent_id = required_text("agent ID", agent_id)?;
        if validated.contains(&agent_id) {
            continue;
        }
        if store.get_agent(&agent_id).map_err(store_error)?.is_none() {
            return Err(format!("agent `{agent_id}` was not found"));
        }
        validated.push(agent_id);
    }
    Ok(validated)
}

fn validate_session_relations(store: &Store, session: &Session) -> Result<(), String> {
    ensure_project(store, &session.project_id)?;
    if store
        .get_agent(&session.agent_id)
        .map_err(store_error)?
        .is_none()
    {
        return Err(format!("agent `{}` was not found", session.agent_id));
    }
    if let Some(task_id) = session.task_id.as_deref() {
        let task = store
            .get_task(task_id)
            .map_err(store_error)?
            .ok_or_else(|| format!("task `{task_id}` was not found"))?;
        if task.project_id != session.project_id {
            return Err(format!(
                "task `{task_id}` does not belong to project `{}`",
                session.project_id
            ));
        }
    }
    Ok(())
}

fn validate_handoff_relations(store: &Store, input: &HandoffInput) -> Result<(), String> {
    ensure_project(store, &input.project_id)?;
    if input.from_agent_id == input.to_agent_id {
        return Err("a handoff requires two different agents".into());
    }
    let task = store
        .get_task(&input.task_id)
        .map_err(store_error)?
        .ok_or_else(|| format!("task `{}` was not found", input.task_id))?;
    if task.project_id != input.project_id {
        return Err(format!(
            "task `{}` does not belong to project `{}`",
            input.task_id, input.project_id
        ));
    }
    for agent_id in [&input.from_agent_id, &input.to_agent_id] {
        if store.get_agent(agent_id).map_err(store_error)?.is_none() {
            return Err(format!("agent `{agent_id}` was not found"));
        }
    }
    Ok(())
}

fn confirmed_deletion(input: DeleteInput) -> Result<String, String> {
    let id = required_text("entity ID", input.id)?;
    if input.confirmation != id {
        return Err("record deletion requires an exact entity ID confirmation".into());
    }
    Ok(id)
}

fn project_root(store: &Store, project_id: &str) -> Result<String, String> {
    store
        .get_project(project_id)
        .map_err(store_error)?
        .ok_or_else(|| format!("project `{project_id}` was not found"))?
        .root_path
        .ok_or_else(|| format!("project `{project_id}` has no local root"))
}

fn canonical_directory(path: &str) -> Result<String, String> {
    let path = Path::new(path);
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("could not resolve project root {}: {error}", path.display()))?;
    if !canonical.is_dir() {
        return Err(format!(
            "project root {} is not a directory",
            canonical.display()
        ));
    }
    Ok(canonical.to_string_lossy().into_owned())
}

fn required_text(label: &str, value: String) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        Err(format!("{label} cannot be empty"))
    } else {
        Ok(value.to_owned())
    }
}

fn bounded_direction(value: String) -> Result<String, String> {
    let value = required_text("direction", value)?;
    if value.len() > MAX_DIRECTION_BYTES {
        return Err(format!(
            "direction cannot exceed {MAX_DIRECTION_BYTES} UTF-8 bytes"
        ));
    }
    Ok(value)
}

fn store_error(error: utu_store::StoreError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use utu_core::{ConnectorCapabilities, CostConfidence, ProviderKind};

    use super::*;

    fn seeded_store() -> Store {
        let store = Store::open_in_memory().expect("store");
        store
            .upsert_provider(&Provider {
                id: "provider".into(),
                display_name: "Provider".into(),
                kind: ProviderKind::LocalCli,
            })
            .expect("provider");
        store
            .upsert_integration(&Integration {
                id: "connector".into(),
                provider_id: Some("provider".into()),
                connector_key: "connector".into(),
                display_name: "Connector".into(),
                kind: ProviderKind::LocalCli,
                state: IntegrationState::Unknown,
                auth: AuthState::Unknown,
                evidence: EvidenceKind::Unsupported,
                checked_at_unix_ms: None,
                problem: None,
                capabilities: ConnectorCapabilities::default(),
            })
            .expect("integration");
        store
    }

    #[test]
    fn empty_snapshot_keeps_unknown_cost_unknown() {
        let store = seeded_store();
        store
            .upsert_project(&Project {
                id: "project".into(),
                name: "Project".into(),
                root_path: None,
                state: ProjectState::Active,
                created_at_unix_ms: 1,
            })
            .expect("project");
        let snapshot = snapshot(&store, &crate::codex_runtime::CodexRuntime::default(), None)
            .expect("snapshot");
        assert_eq!(snapshot.costs[0].amount.micros, None);
        assert_eq!(snapshot.costs[0].amount.confidence, CostConfidence::Unknown);
    }

    #[test]
    fn project_snapshot_scopes_work_but_keeps_global_catalogs_explicit() {
        let store = seeded_store();
        for id in ["project-a", "project-b"] {
            store
                .upsert_project(&Project {
                    id: id.into(),
                    name: id.into(),
                    root_path: None,
                    state: ProjectState::Active,
                    created_at_unix_ms: 1,
                })
                .expect("project");
        }

        let snapshot = snapshot(
            &store,
            &crate::codex_runtime::CodexRuntime::default(),
            Some("project-b"),
        )
        .expect("snapshot");
        assert_eq!(snapshot.scope.project_id.as_deref(), Some("project-b"));
        assert!(snapshot.scope.agents_are_global);
        assert!(snapshot.scope.integrations_are_global);
        assert_eq!(snapshot.projects.len(), 1);
        assert_eq!(snapshot.projects[0].id, "project-b");
        assert_eq!(snapshot.costs.len(), 1);
        assert_eq!(snapshot.costs[0].project_id, "project-b");
        assert_eq!(snapshot.integrations.len(), 1);
    }

    #[test]
    fn unsupported_control_records_request_and_receipt_separately() {
        let store = seeded_store();
        store
            .upsert_project(&Project {
                id: "project".into(),
                name: "Project".into(),
                root_path: None,
                state: ProjectState::Active,
                created_at_unix_ms: 1,
            })
            .expect("project");
        store
            .upsert_agent(&Agent {
                id: "agent".into(),
                provider_id: "provider".into(),
                connector_id: "connector".into(),
                display_name: "Agent".into(),
                model: None,
                capabilities: ConnectorCapabilities::default(),
            })
            .expect("agent");
        store
            .upsert_session(&Session {
                id: "session".into(),
                project_id: "project".into(),
                task_id: None,
                agent_id: "agent".into(),
                provider_session_id: None,
                state: AgentState::Idle,
                started_at_unix_ms: 1,
                last_observed_at_unix_ms: None,
                title_hint: None,
            })
            .expect("session");
        let (request, receipt) =
            record_unsupported_control(&store, "session".into(), ControlAction::Pause, None, 2)
                .expect("receipt");
        assert_eq!(receipt.request_id, request.id);
        assert_eq!(receipt.outcome, ControlOutcome::Unsupported);
        assert_eq!(
            store
                .list_control_requests("session")
                .expect("requests")
                .len(),
            1
        );
    }

    #[test]
    fn deletion_requires_an_exact_id_confirmation() {
        assert!(
            confirmed_deletion(DeleteInput {
                id: "project-1".into(),
                confirmation: "project-2".into(),
            })
            .is_err()
        );
        assert_eq!(
            confirmed_deletion(DeleteInput {
                id: "project-1".into(),
                confirmation: "project-1".into(),
            })
            .expect("confirmed deletion"),
            "project-1"
        );
    }

    #[test]
    fn direction_byte_bound_is_checked_before_persistence_or_transport() {
        assert!(bounded_direction("x".repeat(MAX_DIRECTION_BYTES)).is_ok());
        assert!(bounded_direction("x".repeat(MAX_DIRECTION_BYTES + 1)).is_err());
        let escaped = "\"\\\n".repeat(MAX_DIRECTION_BYTES / 3);
        assert!(escaped.len() <= MAX_DIRECTION_BYTES);
        assert!(bounded_direction(escaped).is_ok());
    }

    #[test]
    fn unhealthy_store_projection_never_allows_native_mutation_or_delivery() {
        let healthy = utu_store::StoreHealth {
            schema_version: 3,
            latest_supported_schema_version: 3,
            integrity_ok: true,
            foreign_keys_enabled: true,
        };
        assert!(store_health_allows_mutation(&healthy));
        for unhealthy in [
            utu_store::StoreHealth {
                integrity_ok: false,
                ..healthy.clone()
            },
            utu_store::StoreHealth {
                foreign_keys_enabled: false,
                ..healthy.clone()
            },
            utu_store::StoreHealth {
                schema_version: 2,
                ..healthy
            },
        ] {
            assert!(!store_health_allows_mutation(&unhealthy));
        }
    }

    #[test]
    fn diagnostic_refresh_keeps_an_attached_codex_runtime() {
        use utu_connectors::{
            AuthDiagnostic, ConnectorDiagnostic, DiagnosticEvidence, ProbeStatus,
        };

        let store = Store::open_in_memory().unwrap();
        store
            .upsert_provider(&Provider {
                id: "codex".into(),
                display_name: "Codex".into(),
                kind: ProviderKind::LocalCli,
            })
            .unwrap();
        let direct = ConnectorCapabilities {
            observe: true,
            auth_probe: true,
            direct: true,
            agent_messages: true,
            ..ConnectorCapabilities::default()
        };
        store
            .upsert_integration(&Integration {
                id: "codex-app-server".into(),
                provider_id: Some("codex".into()),
                connector_key: "codex-app-server".into(),
                display_name: "Codex App Server".into(),
                kind: ProviderKind::LocalCli,
                state: IntegrationState::Ready,
                auth: AuthState::Confirmed,
                evidence: EvidenceKind::Observed,
                checked_at_unix_ms: Some(1),
                problem: None,
                capabilities: direct,
            })
            .unwrap();
        store
            .upsert_agent(&Agent {
                id: "codex-app-server".into(),
                provider_id: "codex".into(),
                connector_id: "codex-app-server".into(),
                display_name: "Codex App Server".into(),
                model: None,
                capabilities: direct,
            })
            .unwrap();
        let runtime = crate::codex_runtime::CodexRuntime::default();
        runtime.replace_authorized_sessions([(
            "session".into(),
            "project".into(),
            "/project".into(),
            "thread".into(),
        )]);
        assert_eq!(runtime.authorized_session_count(), 1);

        let descriptor = known_connector_descriptors()
            .into_iter()
            .find(|descriptor| descriptor.id == "codex")
            .unwrap();
        let report = DiagnosticReport {
            checked_at_unix_ms: 2,
            connectors: vec![ConnectorDiagnostic {
                descriptor,
                installation: DiagnosticEvidence {
                    status: ProbeStatus::Observed,
                    kind: EvidenceKind::Observed,
                    value: Some("/usr/bin/codex".into()),
                    source: "PATH".into(),
                    observed_at_unix_ms: Some(2),
                    detail: None,
                },
                version: DiagnosticEvidence {
                    status: ProbeStatus::Observed,
                    kind: EvidenceKind::Observed,
                    value: Some("codex-test".into()),
                    source: "codex --version".into(),
                    observed_at_unix_ms: Some(2),
                    detail: None,
                },
                auth: AuthDiagnostic {
                    state: AuthState::Confirmed,
                    status: ProbeStatus::Observed,
                    kind: EvidenceKind::Observed,
                    source: "codex login status".into(),
                    observed_at_unix_ms: Some(2),
                    detail: None,
                },
                readiness: Readiness::Ready,
                health: Severity::Healthy,
                problems: Vec::new(),
                command_evidence: Vec::new(),
            }],
        };
        persist_diagnostics(&store, &report).unwrap();

        assert_eq!(runtime.authorized_session_count(), 1);
        let transport = store.get_integration("codex-app-server").unwrap().unwrap();
        assert_eq!(transport.state, IntegrationState::Ready);
        assert!(transport.capabilities.direct);
    }

    #[test]
    fn session_validation_rejects_a_task_from_another_project() {
        let store = seeded_store();
        for id in ["project-a", "project-b"] {
            store
                .upsert_project(&Project {
                    id: id.into(),
                    name: id.into(),
                    root_path: None,
                    state: ProjectState::Active,
                    created_at_unix_ms: 1,
                })
                .expect("project");
        }
        store
            .upsert_agent(&Agent {
                id: "agent".into(),
                provider_id: "provider".into(),
                connector_id: "connector".into(),
                display_name: "Agent".into(),
                model: None,
                capabilities: ConnectorCapabilities::default(),
            })
            .expect("agent");
        store
            .upsert_task(&Task {
                id: "task".into(),
                project_id: "project-a".into(),
                title: "Task".into(),
                detail: String::new(),
                state: TaskState::Draft,
                assignee_agent_ids: vec!["agent".into()],
                created_at_unix_ms: 1,
                updated_at_unix_ms: 1,
            })
            .expect("task");

        let result = validate_session_relations(
            &store,
            &Session {
                id: "session".into(),
                project_id: "project-b".into(),
                task_id: Some("task".into()),
                agent_id: "agent".into(),
                provider_session_id: None,
                state: AgentState::Idle,
                started_at_unix_ms: 1,
                last_observed_at_unix_ms: None,
                title_hint: None,
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn acknowledged_connector_problem_stays_acknowledged_until_it_changes() {
        use utu_connectors::{
            AuthDiagnostic, ConnectorDiagnostic, DiagnosticEvidence, DiagnosticProblem, ProbeStatus,
        };

        let store = seeded_store();
        let descriptor = known_connector_descriptors()
            .into_iter()
            .find(|descriptor| descriptor.id == "codex")
            .expect("codex descriptor");
        let diagnostic = ConnectorDiagnostic {
            descriptor,
            installation: DiagnosticEvidence {
                status: ProbeStatus::Observed,
                kind: EvidenceKind::Observed,
                value: Some("codex".into()),
                source: "PATH".into(),
                observed_at_unix_ms: Some(10),
                detail: None,
            },
            version: DiagnosticEvidence {
                status: ProbeStatus::Observed,
                kind: EvidenceKind::Observed,
                value: Some("codex 1".into()),
                source: "codex --version".into(),
                observed_at_unix_ms: Some(10),
                detail: None,
            },
            auth: AuthDiagnostic {
                state: AuthState::Missing,
                status: ProbeStatus::Observed,
                kind: EvidenceKind::Observed,
                source: "codex login status".into(),
                observed_at_unix_ms: Some(10),
                detail: None,
            },
            readiness: Readiness::NeedsAttention,
            health: Severity::NeedsAttention,
            problems: vec![DiagnosticProblem {
                code: ProblemCode::AuthMissing,
                severity: Severity::NeedsAttention,
                summary: "Codex is signed out.".into(),
                recovery: Some("Run codex login.".into()),
            }],
            command_evidence: vec![],
        };

        store
            .upsert_provider(&Provider {
                id: diagnostic.descriptor.provider_id.into(),
                display_name: diagnostic.descriptor.display_name.into(),
                kind: ProviderKind::LocalCli,
            })
            .expect("diagnostic provider");
        store
            .upsert_integration(&Integration {
                id: diagnostic.descriptor.id.into(),
                provider_id: Some(diagnostic.descriptor.provider_id.into()),
                connector_key: diagnostic.descriptor.id.into(),
                display_name: diagnostic.descriptor.display_name.into(),
                kind: ProviderKind::LocalCli,
                state: IntegrationState::Degraded,
                auth: AuthState::Missing,
                evidence: EvidenceKind::Observed,
                checked_at_unix_ms: Some(10),
                problem: Some("Codex is signed out.".into()),
                capabilities: ConnectorCapabilities::default(),
            })
            .expect("diagnostic integration");
        reconcile_connector_attention(&store, 10, &diagnostic).expect("first finding");
        store
            .set_attention_state("connector-health-codex", AttentionState::Acknowledged, 11)
            .expect("acknowledge");
        reconcile_connector_attention(&store, 12, &diagnostic).expect("refresh finding");
        let finding = store
            .get_attention("connector-health-codex")
            .expect("read")
            .expect("finding");
        assert_eq!(finding.state, AttentionState::Acknowledged);
        assert_eq!(finding.detected_at_unix_ms, 10);
        assert_eq!(finding.evidence, EvidenceKind::Observed);
    }
}
