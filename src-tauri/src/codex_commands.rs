use std::{collections::HashSet, path::Path, sync::Arc};

use serde::{Deserialize, Serialize};
use tauri::State;
use utu_codex::{CodexError, ThreadSummary};
use utu_connectors::{Readiness, diagnose_known_connectors};
use utu_core::{
    Agent, AgentState, AuthState, ConnectorCapabilities, EvidenceKind, IntegrationState, Project,
    Session,
};
use utu_store::{NewMessage, Store};

use crate::clock::entity_id;
use crate::ids::deterministic_id;
use crate::{clock::unix_ms, codex_runtime::CodexRuntime, state::AppState};

pub(crate) const CODEX_PROVIDER_ID: &str = "codex";
pub(crate) const CODEX_DIAGNOSTIC_INTEGRATION_ID: &str = "codex";
pub(crate) const CODEX_TRANSPORT_INTEGRATION_ID: &str = "codex-app-server";
pub(crate) const CODEX_AGENT_ID: &str = "codex-app-server";
const MAX_PROVIDER_RECEIPT_ID_BYTES: usize = 512;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncCodexSessionsInput {
    /// The native API is fail-closed: callers must explicitly confirm that
    /// provider thread metadata may be read for exactly one selected project.
    pub confirmed_metadata_sync: bool,
    pub project_ids: Vec<String>,
    /// Reserved explicit privacy switch. This vertical slice is metadata-only,
    /// so true is rejected rather than silently ingesting unencrypted bodies.
    #[serde(default)]
    pub import_transcripts: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncCodexSessionsSummary {
    pub metadata_only: bool,
    pub handshake_confirmed: bool,
    pub server_version: String,
    pub discovered: u32,
    pub imported_projects: u32,
    pub imported_sessions: u32,
    pub skipped_missing_cwd: u32,
    pub skipped_noncanonical_cwd: u32,
    pub skipped_nonlocal_cwd: u32,
    pub transcripts_imported: u32,
}

#[derive(Debug)]
pub struct CodexDirectionOutcome {
    pub message: utu_core::Message,
    pub request: utu_core::ControlRequest,
    pub receipt: utu_core::ControlReceipt,
}

type RuntimeAuthorization = (String, String, String, String);

pub(crate) struct PersistedMetadata {
    pub summary: SyncCodexSessionsSummary,
    pub authorizations: Vec<RuntimeAuthorization>,
}

enum DirectionDelivery {
    Acknowledged(utu_codex::TurnRecord),
    PreTurnFailed,
    TurnFailed(CodexError),
}

#[tauri::command]
pub async fn sync_codex_sessions(
    state: State<'_, AppState>,
    input: SyncCodexSessionsInput,
) -> Result<SyncCodexSessionsSummary, String> {
    validate_sync_input(&input)?;
    let store = Arc::clone(&state.store);
    let runtime = Arc::clone(&state.codex);
    tauri::async_runtime::spawn_blocking(move || {
        let _lifecycle = runtime.lock_lifecycle();
        crate::commands::ensure_store_healthy(&store)?;
        let projects = confirmed_sync_projects(&store, &input)?;
        // A fresh diagnostic may represent a different local account. Never
        // reuse an initialized process or eligibility from an earlier consent.
        begin_codex_sync(&store, &runtime)?;
        let report = diagnose_known_connectors();
        let diagnostic = report
            .connectors
            .iter()
            .find(|diagnostic| diagnostic.descriptor.id == CODEX_DIAGNOSTIC_INTEGRATION_ID)
            .ok_or_else(|| "Codex diagnostics were unavailable".to_owned())?;
        let codex_path = match require_fresh_codex_diagnostic(diagnostic) {
            Ok(path) => path,
            Err(error) => {
                runtime.revoke_all();
                deactivate_codex_transport(&store)?;
                crate::commands::persist_diagnostics(&store, &report)?;
                return Err(error);
            }
        };
        crate::commands::persist_diagnostics(&store, &report)?;
        let mut aggregate = empty_sync_summary();
        let mut server_version = None;
        let mut authorizations = Vec::new();
        for (project, cwd) in projects {
            let (observed_version, threads) = runtime
                .connect_and_list(codex_path.clone(), &cwd)
                .map_err(codex_error_message)?;
            if let Some(expected) = server_version.as_deref()
                && expected != observed_version
            {
                runtime.revoke_all();
                return Err("Codex App Server identity changed during synchronization".into());
            }
            if server_version.is_none() {
                prepare_codex_identity(&store, &observed_version)?;
                server_version = Some(observed_version.clone());
            }
            let persisted =
                persist_thread_metadata(&store, &observed_version, &threads, &project, &cwd)?;
            merge_sync_summary(&mut aggregate, persisted.summary);
            authorizations.extend(persisted.authorizations);
        }
        let server_version = server_version
            .ok_or_else(|| "Select at least one project for Codex synchronization".to_owned())?;
        aggregate.server_version = server_version.clone();
        aggregate.handshake_confirmed = true;
        activate_codex_transport(&store, &server_version)?;
        runtime.replace_project_authorizations(&input.project_ids[0], authorizations);
        Ok(aggregate)
    })
    .await
    .map_err(|error| format!("Codex synchronization worker failed: {error}"))?
}

fn begin_codex_sync(store: &Store, runtime: &CodexRuntime) -> Result<(), String> {
    runtime.revoke_all();
    deactivate_codex_transport(store)
}

fn validate_sync_input(input: &SyncCodexSessionsInput) -> Result<(), String> {
    if !input.confirmed_metadata_sync {
        return Err("Codex metadata synchronization requires explicit confirmation".into());
    }
    if input.project_ids.len() != 1 || input.project_ids[0].trim().is_empty() {
        return Err("Select exactly one local project for Codex metadata synchronization".into());
    }
    if input.import_transcripts {
        return Err(
            "Codex transcript import is not available; Utu's local store is owner-only but not encrypted"
                .into(),
        );
    }
    Ok(())
}

fn confirmed_sync_projects(
    store: &Store,
    input: &SyncCodexSessionsInput,
) -> Result<Vec<(Project, String)>, String> {
    validate_sync_input(input)?;
    let mut ids = HashSet::new();
    let mut roots = HashSet::new();
    let mut projects = Vec::with_capacity(input.project_ids.len());
    for project_id in &input.project_ids {
        if !ids.insert(project_id.clone()) {
            return Err(format!(
                "project `{project_id}` was selected more than once"
            ));
        }
        let project = store
            .get_project(project_id)
            .map_err(store_error)?
            .ok_or_else(|| format!("project `{project_id}` was not found"))?;
        let root = canonical_stored_project_root(&project)?;
        if !roots.insert(root.clone()) {
            return Err("Selected Codex projects must have distinct canonical roots".into());
        }
        projects.push((project, root));
    }
    Ok(projects)
}

pub(crate) fn require_fresh_codex_diagnostic(
    diagnostic: &utu_connectors::ConnectorDiagnostic,
) -> Result<std::path::PathBuf, String> {
    if diagnostic.readiness != Readiness::Ready
        || diagnostic.auth.state != AuthState::Confirmed
        || diagnostic.auth.kind != EvidenceKind::Observed
        || diagnostic.installation.kind != EvidenceKind::Observed
    {
        return Err("Codex is not freshly observed, ready, and authenticated".into());
    }
    let path = diagnostic
        .installation
        .value
        .as_ref()
        .ok_or_else(|| "Codex diagnostics did not return an executable path".to_owned())?
        .canonicalize()
        .map_err(|_| "Codex executable path is no longer available".to_owned())?;
    if !path.is_absolute() || !path.is_file() {
        return Err("Codex executable path is not a file".into());
    }
    Ok(path)
}

fn empty_sync_summary() -> SyncCodexSessionsSummary {
    SyncCodexSessionsSummary {
        metadata_only: true,
        handshake_confirmed: false,
        server_version: String::new(),
        discovered: 0,
        imported_projects: 0,
        imported_sessions: 0,
        skipped_missing_cwd: 0,
        skipped_noncanonical_cwd: 0,
        skipped_nonlocal_cwd: 0,
        transcripts_imported: 0,
    }
}

fn merge_sync_summary(aggregate: &mut SyncCodexSessionsSummary, next: SyncCodexSessionsSummary) {
    aggregate.discovered = aggregate.discovered.saturating_add(next.discovered);
    aggregate.imported_projects = aggregate
        .imported_projects
        .saturating_add(next.imported_projects);
    aggregate.imported_sessions = aggregate
        .imported_sessions
        .saturating_add(next.imported_sessions);
    aggregate.skipped_missing_cwd = aggregate
        .skipped_missing_cwd
        .saturating_add(next.skipped_missing_cwd);
    aggregate.skipped_noncanonical_cwd = aggregate
        .skipped_noncanonical_cwd
        .saturating_add(next.skipped_noncanonical_cwd);
    aggregate.skipped_nonlocal_cwd = aggregate
        .skipped_nonlocal_cwd
        .saturating_add(next.skipped_nonlocal_cwd);
}

pub(crate) fn persist_thread_metadata(
    store: &Store,
    server_version: &str,
    threads: &[ThreadSummary],
    project: &Project,
    selected_root: &str,
) -> Result<PersistedMetadata, String> {
    let mut summary = SyncCodexSessionsSummary {
        metadata_only: true,
        handshake_confirmed: true,
        server_version: server_version.to_owned(),
        discovered: threads.len().try_into().unwrap_or(u32::MAX),
        imported_projects: 0,
        imported_sessions: 0,
        skipped_missing_cwd: 0,
        skipped_noncanonical_cwd: 0,
        skipped_nonlocal_cwd: 0,
        transcripts_imported: 0,
    };
    let mut imported_projects = HashSet::new();
    let mut authorizations = Vec::new();
    for thread in threads {
        let Some(cwd) = thread.cwd.as_deref() else {
            summary.skipped_missing_cwd = summary.skipped_missing_cwd.saturating_add(1);
            continue;
        };
        let path = Path::new(cwd);
        let Ok(canonical) = path.canonicalize() else {
            summary.skipped_noncanonical_cwd = summary.skipped_noncanonical_cwd.saturating_add(1);
            continue;
        };
        if !canonical.is_dir() || canonical != path {
            summary.skipped_noncanonical_cwd = summary.skipped_noncanonical_cwd.saturating_add(1);
            continue;
        }
        let canonical_key = canonical.to_string_lossy().into_owned();
        if canonical_key != selected_root {
            summary.skipped_nonlocal_cwd = summary.skipped_nonlocal_cwd.saturating_add(1);
            continue;
        }
        let session = Session {
            id: deterministic_id("codex-session", &thread.id),
            project_id: project.id.clone(),
            task_id: None,
            agent_id: CODEX_AGENT_ID.into(),
            provider_session_id: Some(thread.id.clone()),
            state: codex_thread_state(thread.status.as_deref()),
            started_at_unix_ms: timestamp_millis(thread.created_at).unwrap_or_else(unix_ms),
            last_observed_at_unix_ms: Some(unix_ms()),
            title_hint: thread.name.clone().or_else(|| thread.preview.clone()),
        };
        store.upsert_session(&session).map_err(store_error)?;
        authorizations.push((
            session.id.clone(),
            project.id.clone(),
            selected_root.to_owned(),
            thread.id.clone(),
        ));
        imported_projects.insert(project.id.clone());
        summary.imported_sessions = summary.imported_sessions.saturating_add(1);
    }
    summary.imported_projects = imported_projects.len().try_into().unwrap_or(u32::MAX);
    Ok(PersistedMetadata {
        summary,
        authorizations,
    })
}

pub(crate) fn activate_codex_transport(store: &Store, server_version: &str) -> Result<(), String> {
    let diagnostic = require_ready_codex_diagnostic(store)?;
    let capabilities = ConnectorCapabilities {
        observe: true,
        auth_probe: true,
        direct: true,
        pause: false,
        resume: false,
        stop: false,
        logs: false,
        costs: false,
        agent_messages: true,
    };
    let integration = utu_core::Integration {
        id: CODEX_TRANSPORT_INTEGRATION_ID.into(),
        provider_id: Some(CODEX_PROVIDER_ID.into()),
        connector_key: CODEX_TRANSPORT_INTEGRATION_ID.into(),
        display_name: "Codex App Server".into(),
        kind: utu_core::ProviderKind::LocalCli,
        state: IntegrationState::Ready,
        auth: diagnostic.auth,
        evidence: EvidenceKind::Observed,
        checked_at_unix_ms: Some(unix_ms()),
        problem: None,
        capabilities,
    };
    store
        .activate_integration_agent(
            &integration,
            &Agent {
                id: CODEX_AGENT_ID.into(),
                provider_id: CODEX_PROVIDER_ID.into(),
                connector_id: CODEX_TRANSPORT_INTEGRATION_ID.into(),
                display_name: "Codex App Server".into(),
                model: Some(server_version.to_owned()),
                capabilities,
            },
        )
        .map_err(store_error)
}

pub(crate) fn prepare_codex_identity(store: &Store, server_version: &str) -> Result<(), String> {
    let diagnostic = require_ready_codex_diagnostic(store)?;
    if store
        .get_integration(CODEX_TRANSPORT_INTEGRATION_ID)
        .map_err(store_error)?
        .is_none()
    {
        let integration = utu_core::Integration {
            id: CODEX_TRANSPORT_INTEGRATION_ID.into(),
            provider_id: Some(CODEX_PROVIDER_ID.into()),
            connector_key: CODEX_TRANSPORT_INTEGRATION_ID.into(),
            display_name: "Codex App Server".into(),
            kind: utu_core::ProviderKind::LocalCli,
            state: IntegrationState::Unknown,
            auth: diagnostic.auth,
            evidence: EvidenceKind::Observed,
            checked_at_unix_ms: Some(unix_ms()),
            problem: Some("Project-scoped Codex metadata synchronization is not complete.".into()),
            capabilities: ConnectorCapabilities::default(),
        };
        store
            .upsert_integration(&integration)
            .map_err(store_error)?;
    }
    match store.get_agent(CODEX_AGENT_ID).map_err(store_error)? {
        Some(mut agent) => {
            if agent.model.as_deref() != Some(server_version) {
                agent.model = Some(server_version.to_owned());
                store.upsert_agent(&agent).map_err(store_error)?;
            }
            Ok(())
        }
        None => store
            .upsert_agent(&Agent {
                id: CODEX_AGENT_ID.into(),
                provider_id: CODEX_PROVIDER_ID.into(),
                connector_id: CODEX_TRANSPORT_INTEGRATION_ID.into(),
                display_name: "Codex App Server".into(),
                model: Some(server_version.to_owned()),
                capabilities: ConnectorCapabilities::default(),
            })
            .map_err(store_error),
    }
}

pub(crate) fn deactivate_codex_transport(store: &Store) -> Result<(), String> {
    if let Some(mut agent) = store.get_agent(CODEX_AGENT_ID).map_err(store_error)? {
        agent.capabilities = ConnectorCapabilities::default();
        store.upsert_agent(&agent).map_err(store_error)?;
    }
    if let Some(mut integration) = store
        .get_integration(CODEX_TRANSPORT_INTEGRATION_ID)
        .map_err(store_error)?
    {
        integration.state = IntegrationState::Unknown;
        integration.auth = AuthState::Unknown;
        integration.evidence = EvidenceKind::Stale;
        integration.capabilities = ConnectorCapabilities::default();
        integration.problem =
            Some("Codex App Server is not attached to an observed authenticated runtime.".into());
        store
            .upsert_integration(&integration)
            .map_err(store_error)?;
    }
    Ok(())
}

#[cfg(test)]
fn ensure_codex_agent(store: &Store, server_version: &str) -> Result<(), String> {
    let integration = require_ready_codex_transport(store)?;
    let capabilities = ConnectorCapabilities {
        observe: true,
        auth_probe: true,
        direct: true,
        pause: false,
        resume: false,
        stop: false,
        logs: false,
        costs: false,
        agent_messages: true,
    };
    if !capabilities_subset(capabilities, integration.capabilities) {
        return Err("Codex integration does not grant the required App Server capabilities".into());
    }
    store
        .upsert_agent(&Agent {
            id: CODEX_AGENT_ID.into(),
            provider_id: CODEX_PROVIDER_ID.into(),
            connector_id: CODEX_TRANSPORT_INTEGRATION_ID.into(),
            display_name: "Codex App Server".into(),
            model: Some(server_version.to_owned()),
            capabilities,
        })
        .map_err(store_error)
}

fn require_ready_codex_diagnostic(store: &Store) -> Result<utu_core::Integration, String> {
    let integration = store
        .get_integration(CODEX_DIAGNOSTIC_INTEGRATION_ID)
        .map_err(store_error)?
        .ok_or_else(|| "Run connector diagnostics before using Codex App Server".to_owned())?;
    if integration.provider_id.as_deref() != Some(CODEX_PROVIDER_ID)
        || integration.kind != utu_core::ProviderKind::LocalCli
        || integration.state != IntegrationState::Ready
        || integration.auth != AuthState::Confirmed
        || integration.evidence != EvidenceKind::Observed
        || integration.checked_at_unix_ms.is_none()
    {
        return Err("Codex integration is not observed, ready, and authenticated".into());
    }
    Ok(integration)
}

#[cfg(test)]
fn require_ready_codex_transport(store: &Store) -> Result<utu_core::Integration, String> {
    let integration = store
        .get_integration(CODEX_TRANSPORT_INTEGRATION_ID)
        .map_err(store_error)?
        .ok_or_else(|| "Synchronize the selected project with Codex App Server first".to_owned())?;
    if integration.provider_id.as_deref() != Some(CODEX_PROVIDER_ID)
        || integration.kind != utu_core::ProviderKind::LocalCli
        || integration.state != IntegrationState::Ready
        || integration.auth != AuthState::Confirmed
        || integration.evidence != EvidenceKind::Observed
        || integration.checked_at_unix_ms.is_none()
    {
        return Err("Codex App Server transport is not observed and ready".into());
    }
    Ok(integration)
}

pub(crate) fn provider_delivery_eligibility(
    store: &Store,
    runtime: &CodexRuntime,
    session: &Session,
) -> crate::commands::ProviderDeliveryEligibility {
    let agents = store.list_agents().unwrap_or_default();
    let integrations = store.list_integrations().unwrap_or_default();
    let projects = store.list_projects().unwrap_or_default();
    provider_delivery_eligibility_from_projection(
        runtime,
        session,
        &agents,
        &integrations,
        &projects,
    )
}

pub(crate) fn provider_delivery_eligibility_from_projection(
    runtime: &CodexRuntime,
    session: &Session,
    agents: &[Agent],
    integrations: &[utu_core::Integration],
    projects: &[Project],
) -> crate::commands::ProviderDeliveryEligibility {
    let agent = agents.iter().find(|agent| agent.id == session.agent_id);
    let provider_id = agent.as_ref().map(|agent| agent.provider_id.clone());
    let eligible = (|| {
        let agent = agent?;
        let provider_thread_id = session.provider_session_id.as_deref()?;
        if session.agent_id != CODEX_AGENT_ID
            || session.id != deterministic_id("codex-session", provider_thread_id)
            || agent.provider_id != CODEX_PROVIDER_ID
            || agent.connector_id != CODEX_TRANSPORT_INTEGRATION_ID
            || !agent.capabilities.direct
        {
            return None;
        }
        let integration = integrations
            .iter()
            .find(|integration| integration.id == CODEX_TRANSPORT_INTEGRATION_ID)?;
        if integration.provider_id.as_deref() != Some(CODEX_PROVIDER_ID)
            || integration.kind != utu_core::ProviderKind::LocalCli
            || integration.state != IntegrationState::Ready
            || integration.auth != AuthState::Confirmed
            || integration.evidence != EvidenceKind::Observed
            || integration.checked_at_unix_ms.is_none()
            || !integration.capabilities.direct
        {
            return None;
        }
        let project = projects
            .iter()
            .find(|project| project.id == session.project_id)?;
        let root = canonical_stored_project_root(project).ok()?;
        runtime
            .is_session_authorized(&session.id, &session.project_id, &root, provider_thread_id)
            .then_some(())
    })()
    .is_some();
    crate::commands::ProviderDeliveryEligibility {
        session_id: session.id.clone(),
        provider_id,
        eligible,
        requires_confirmation: eligible,
    }
}

pub fn try_send_direction(
    store: &Store,
    runtime: &CodexRuntime,
    session: &Session,
    body: String,
    allow_provider_delivery: bool,
) -> Result<CodexDirectionOutcome, String> {
    if !allow_provider_delivery {
        return Err("Codex provider delivery requires explicit confirmation".into());
    }
    if !provider_delivery_eligibility(store, runtime, session).eligible {
        return Err("Selected session is not currently eligible for provider delivery".into());
    }
    if session.agent_id != CODEX_AGENT_ID || session.provider_session_id.is_none() {
        return Err("Selected session is not bound to Codex App Server delivery".into());
    }
    let Some(integration) = store
        .get_integration(CODEX_TRANSPORT_INTEGRATION_ID)
        .map_err(store_error)?
    else {
        return Err("Codex App Server transport is not active".into());
    };
    let Some(agent) = store.get_agent(&session.agent_id).map_err(store_error)? else {
        return Err("Stored Codex session references a missing agent".into());
    };
    let transport_active = integration.state == IntegrationState::Ready
        && integration.auth == AuthState::Confirmed
        && integration.evidence == EvidenceKind::Observed
        && integration.capabilities.direct
        && agent.capabilities.direct;
    if !transport_active {
        return Err("Codex App Server transport is not active".into());
    }
    if agent.provider_id != CODEX_PROVIDER_ID
        || agent.connector_id != CODEX_TRANSPORT_INTEGRATION_ID
        || !agent.capabilities.direct
        || !integration.capabilities.direct
    {
        return Err("Stored Codex session does not have effective direct capability".into());
    }
    let project = store
        .get_project(&session.project_id)
        .map_err(store_error)?
        .ok_or_else(|| "Stored Codex session references a missing project".to_owned())?;
    let cwd = canonical_stored_project_root(&project)?;
    let provider_thread_id = session
        .provider_session_id
        .as_deref()
        .ok_or_else(|| "Stored Codex session has no provider thread".to_owned())?;
    if session.id != deterministic_id("codex-session", provider_thread_id) {
        return Err("Stored Codex session has an invalid provider binding".into());
    }
    let now = unix_ms();
    let request = utu_core::ControlRequest {
        id: entity_id("control"),
        session_id: session.id.clone(),
        action: utu_core::ControlAction::Direct,
        instruction: Some(body.clone()),
        requested_at_unix_ms: now,
        requested_by_owner: true,
    };
    // The initial receipt is deliberately Unknown. A successful turn/start
    // only proves provider acknowledgement, never task or turn completion.
    let initial_receipt = utu_core::ControlReceipt {
        id: entity_id("receipt"),
        request_id: request.id.clone(),
        outcome: utu_core::ControlOutcome::Unknown,
        received_at_unix_ms: now,
        evidence: EvidenceKind::Inferred,
        source: "utu.codex-app-server".into(),
        message: Some("Codex delivery is pending.".into()),
        provider_receipt_id: None,
    };
    let owner_message = NewMessage {
        id: entity_id("message"),
        session_id: session.id.clone(),
        role: utu_core::MessageRole::Owner,
        author_agent_id: None,
        body: body.clone(),
        sent_at_unix_ms: now,
        ingested_at_unix_ms: now,
        evidence: EvidenceKind::Observed,
        source: "utu.owner".into(),
        correlation_id: Some(request.id.clone()),
    };
    let turn_cwd = cwd.clone();
    let (recorded, delivery) = runtime
        .with_authorized_client(
            &session.id,
            &session.project_id,
            &cwd,
            provider_thread_id,
            |client| {
                // The runtime authorization and durable owner intent share one
                // runtime lock, so revoke cannot interleave between them.
                let recorded = store
                    .record_owner_direction(owner_message, request, initial_receipt)
                    .map_err(|_| CodexError::Io {
                        operation: "recording owner direction",
                    })?;
                let resumed = match client.resume_thread(
                    provider_thread_id,
                    utu_codex::ResumeThreadOptions {
                        cwd: Some(cwd.clone()),
                        model: None,
                        sandbox: Some(utu_codex::SandboxMode::ReadOnly),
                        approval_policy: Some(utu_codex::ApprovalPolicy::Never),
                    },
                ) {
                    Ok(resumed) => resumed,
                    Err(_) => return Ok((recorded, DirectionDelivery::PreTurnFailed)),
                };
                if resumed.summary.id != provider_thread_id
                    || resumed.summary.cwd.as_deref() != Some(cwd.as_str())
                {
                    return Ok((recorded, DirectionDelivery::PreTurnFailed));
                }
                let delivery = match client.start_turn(
                    provider_thread_id,
                    &body,
                    utu_codex::TurnStartOptions {
                        cwd: Some(turn_cwd),
                        model: None,
                        reasoning_effort: None,
                        sandbox_policy: Some(utu_codex::TurnSandboxPolicy::ReadOnly {
                            network_access: false,
                        }),
                        approval_policy: Some(utu_codex::ApprovalPolicy::Never),
                        client_user_message_id: Some(recorded.message.id.clone()),
                    },
                ) {
                    Ok(turn) => DirectionDelivery::Acknowledged(turn),
                    Err(error) => DirectionDelivery::TurnFailed(error),
                };
                Ok((recorded, delivery))
            },
        )
        .map_err(|_| {
            "Codex provider delivery was not sent: synchronize this exact session again".to_owned()
        })?;
    let receipt = delivery_receipt(&recorded.request, Ok(delivery));
    store
        .upsert_control_receipt(&receipt)
        .map_err(store_error)?;
    Ok(CodexDirectionOutcome {
        message: recorded.message,
        request: recorded.request,
        receipt,
    })
}

fn delivery_receipt(
    request: &utu_core::ControlRequest,
    result: Result<DirectionDelivery, CodexError>,
) -> utu_core::ControlReceipt {
    let (outcome, evidence, message, provider_receipt_id) = match result {
        Ok(DirectionDelivery::Acknowledged(turn)) if valid_provider_receipt_id(&turn.id) => (
            utu_core::ControlOutcome::Acknowledged,
            EvidenceKind::Observed,
            "Codex accepted the turn; completion is not yet observed.".into(),
            Some(turn.id),
        ),
        Ok(DirectionDelivery::Acknowledged(_)) => (
            utu_core::ControlOutcome::Unknown,
            EvidenceKind::Inferred,
            "Codex returned an invalid turn receipt; acceptance remains unknown and requires reconciliation."
                .into(),
            None,
        ),
        Ok(DirectionDelivery::PreTurnFailed) => (
            utu_core::ControlOutcome::Rejected,
            EvidenceKind::Observed,
            "Codex direction was not sent because the provider thread could not be safely resumed.".into(),
            None,
        ),
        Ok(DirectionDelivery::TurnFailed(CodexError::Timeout { .. })) => (
            utu_core::ControlOutcome::TimedOut,
            EvidenceKind::Observed,
            "Codex delivery timed out; acceptance is unknown.".into(),
            None,
        ),
        Ok(DirectionDelivery::TurnFailed(CodexError::Rpc { .. })) => (
            utu_core::ControlOutcome::Rejected,
            EvidenceKind::Observed,
            "Codex explicitly rejected the turn.".into(),
            None,
        ),
        Ok(DirectionDelivery::TurnFailed(
            CodexError::InvalidInput(_) | CodexError::MessageTooLarge | CodexError::Overloaded,
        )) => (
            utu_core::ControlOutcome::Rejected,
            EvidenceKind::Observed,
            "Codex direction was not sent because local provider validation rejected it.".into(),
            None,
        ),
        Ok(DirectionDelivery::TurnFailed(_)) | Err(_) => (
            utu_core::ControlOutcome::Unknown,
            EvidenceKind::Inferred,
            "Codex delivery could not be confirmed; acceptance remains unknown and requires reconciliation.".into(),
            None,
        ),
    };
    utu_core::ControlReceipt {
        id: entity_id("receipt"),
        request_id: request.id.clone(),
        outcome,
        received_at_unix_ms: unix_ms(),
        evidence,
        source: "utu.codex-app-server".into(),
        message: Some(message),
        provider_receipt_id,
    }
}

fn valid_provider_receipt_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PROVIDER_RECEIPT_ID_BYTES
        && !value.chars().any(char::is_control)
}

pub(crate) fn canonical_stored_project_root(project: &Project) -> Result<String, String> {
    let root = project
        .root_path
        .as_deref()
        .ok_or_else(|| "Codex project has no local root".to_owned())?;
    let path = Path::new(root);
    let canonical = path
        .canonicalize()
        .map_err(|_| "Codex project root is no longer available".to_owned())?;
    if !canonical.is_dir() || canonical != path {
        return Err("Codex project root is not a canonical local directory".into());
    }
    Ok(canonical.to_string_lossy().into_owned())
}

#[cfg(test)]
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

fn codex_thread_state(status: Option<&str>) -> AgentState {
    match status.unwrap_or_default().to_ascii_lowercase().as_str() {
        "active" | "running" => AgentState::Running,
        "waiting" => AgentState::Waiting,
        "failed" | "error" => AgentState::Problem,
        _ => AgentState::Idle,
    }
}

fn timestamp_millis(value: Option<i64>) -> Option<u64> {
    value
        .and_then(|value| u64::try_from(value).ok())
        .map(|value| {
            if value < 10_000_000_000 {
                value.saturating_mul(1_000)
            } else {
                value
            }
        })
}

fn codex_error_message(error: CodexError) -> String {
    match error {
        CodexError::Timeout { .. } => "Codex App Server request timed out".into(),
        CodexError::Closed | CodexError::ProcessExited => "Codex App Server disconnected".into(),
        CodexError::Overloaded => "Codex App Server is busy".into(),
        _ => "Codex App Server synchronization failed".into(),
    }
}

fn store_error(error: utu_store::StoreError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use utu_core::{
        ControlAction, ControlOutcome, Integration, ProjectState, Provider, ProviderKind,
    };
    use utu_store::StreamQuery;

    use super::*;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("utu-codex-command-{}-{nonce}", std::process::id()));
            fs::create_dir_all(&path).expect("fixture directory");
            Self(path)
        }

        fn canonical(&self) -> PathBuf {
            self.0.canonicalize().expect("canonical fixture")
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
                id: CODEX_PROVIDER_ID.into(),
                display_name: "Codex".into(),
                kind: ProviderKind::LocalCli,
            })
            .unwrap();
        for (integration_id, direct) in [
            (CODEX_DIAGNOSTIC_INTEGRATION_ID, false),
            (CODEX_TRANSPORT_INTEGRATION_ID, true),
        ] {
            store
                .upsert_integration(&Integration {
                    id: integration_id.into(),
                    provider_id: Some(CODEX_PROVIDER_ID.into()),
                    connector_key: integration_id.into(),
                    display_name: integration_id.into(),
                    kind: ProviderKind::LocalCli,
                    state: IntegrationState::Ready,
                    auth: AuthState::Confirmed,
                    evidence: EvidenceKind::Observed,
                    checked_at_unix_ms: Some(1),
                    problem: None,
                    capabilities: ConnectorCapabilities {
                        observe: direct,
                        auth_probe: true,
                        direct,
                        pause: false,
                        resume: false,
                        stop: false,
                        logs: false,
                        costs: false,
                        agent_messages: direct,
                    },
                })
                .unwrap();
        }
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
    fn sync_is_metadata_only_and_rejects_unmapped_or_noncanonical_cwds() {
        let root = Fixture::new();
        let canonical = root.canonical();
        fs::create_dir(canonical.join("unmapped")).unwrap();
        let store = ready_store(&canonical);
        let missing = canonical.join("missing");
        let threads = vec![
            ThreadSummary {
                id: "valid".into(),
                cwd: Some(canonical.to_string_lossy().into_owned()),
                ..ThreadSummary::default()
            },
            ThreadSummary {
                id: "missing".into(),
                cwd: Some(missing.to_string_lossy().into_owned()),
                ..ThreadSummary::default()
            },
            ThreadSummary {
                id: "noncanonical".into(),
                cwd: Some(canonical.join(".").to_string_lossy().into_owned()),
                ..ThreadSummary::default()
            },
            ThreadSummary {
                id: "unmapped".into(),
                cwd: Some(
                    canonical
                        .join("unmapped")
                        .canonicalize()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                ),
                ..ThreadSummary::default()
            },
            ThreadSummary {
                id: "no-cwd".into(),
                cwd: None,
                ..ThreadSummary::default()
            },
        ];
        ensure_codex_agent(&store, "codex-test").unwrap();
        let project = store.get_project("project").unwrap().unwrap();
        let selected_root = canonical.to_string_lossy().into_owned();
        let summary =
            persist_thread_metadata(&store, "codex-test", &threads, &project, &selected_root)
                .unwrap()
                .summary;
        assert!(summary.metadata_only);
        assert_eq!(summary.transcripts_imported, 0);
        assert_eq!(summary.imported_sessions, 2);
        assert_eq!(summary.imported_projects, 1);
        assert_eq!(summary.skipped_noncanonical_cwd, 1);
        assert_eq!(summary.skipped_nonlocal_cwd, 1);
        assert_eq!(summary.skipped_missing_cwd, 1);
        let sessions = store.list_sessions(None).unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(
            sessions
                .iter()
                .any(|session| session.provider_session_id.as_deref() == Some("valid"))
        );
    }

    #[test]
    fn first_metadata_sync_creates_inactive_identity_before_session_records() {
        let root = Fixture::new();
        let canonical = root.canonical();
        let store = Store::open_in_memory().unwrap();
        store
            .upsert_provider(&Provider {
                id: CODEX_PROVIDER_ID.into(),
                display_name: "Codex".into(),
                kind: ProviderKind::LocalCli,
            })
            .unwrap();
        store
            .upsert_integration(&Integration {
                id: CODEX_DIAGNOSTIC_INTEGRATION_ID.into(),
                provider_id: Some(CODEX_PROVIDER_ID.into()),
                connector_key: CODEX_DIAGNOSTIC_INTEGRATION_ID.into(),
                display_name: "Codex diagnostics".into(),
                kind: ProviderKind::LocalCli,
                state: IntegrationState::Ready,
                auth: AuthState::Confirmed,
                evidence: EvidenceKind::Observed,
                checked_at_unix_ms: Some(1),
                problem: None,
                capabilities: ConnectorCapabilities {
                    auth_probe: true,
                    ..ConnectorCapabilities::default()
                },
            })
            .unwrap();
        let project = Project {
            id: "project".into(),
            name: "Project".into(),
            root_path: Some(canonical.to_string_lossy().into_owned()),
            state: ProjectState::Active,
            created_at_unix_ms: 1,
        };
        store.upsert_project(&project).unwrap();

        prepare_codex_identity(&store, "codex-test").unwrap();
        let summary = persist_thread_metadata(
            &store,
            "codex-test",
            &[ThreadSummary {
                id: "first-thread".into(),
                cwd: project.root_path.clone(),
                ..ThreadSummary::default()
            }],
            &project,
            project.root_path.as_deref().unwrap(),
        )
        .unwrap()
        .summary;

        assert_eq!(summary.imported_sessions, 1);
        let agent = store.get_agent(CODEX_AGENT_ID).unwrap().unwrap();
        assert!(!agent.capabilities.direct);
        let transport = store
            .get_integration(CODEX_TRANSPORT_INTEGRATION_ID)
            .unwrap()
            .unwrap();
        assert_eq!(transport.state, IntegrationState::Unknown);
        assert!(!transport.capabilities.direct);
    }

    #[test]
    fn prepare_codex_identity_does_not_invalidate_an_activated_agent() {
        let root = Fixture::new();
        let store = ready_store(&root.canonical());
        ensure_codex_agent(&store, "codex-test").unwrap();
        prepare_codex_identity(&store, "codex-next").unwrap();
        let agent = store.get_agent(CODEX_AGENT_ID).unwrap().unwrap();
        let transport = store
            .get_integration(CODEX_TRANSPORT_INTEGRATION_ID)
            .unwrap()
            .unwrap();
        assert!(agent.capabilities.direct);
        assert_eq!(agent.model.as_deref(), Some("codex-next"));
        assert_eq!(transport.state, IntegrationState::Ready);
        assert!(transport.capabilities.direct);
    }

    #[test]
    fn transcript_import_requires_a_future_explicit_encrypted_contract() {
        assert!(
            validate_sync_input(&SyncCodexSessionsInput {
                confirmed_metadata_sync: true,
                project_ids: vec!["project".into()],
                import_transcripts: false,
            })
            .is_ok()
        );
        let error = validate_sync_input(&SyncCodexSessionsInput {
            confirmed_metadata_sync: true,
            project_ids: vec!["project".into()],
            import_transcripts: true,
        })
        .unwrap_err();
        assert!(error.contains("not encrypted"));
    }

    #[test]
    fn disconnected_runtime_fails_without_fallback() {
        let runtime = CodexRuntime::default();
        let error = runtime
            .with_authorized_client("session", "project", "/tmp/project", "thread", |_| Ok(()))
            .unwrap_err();
        assert!(matches!(error, CodexError::InvalidInput(_)));
        assert!(!runtime.is_connected());
    }

    #[test]
    fn failed_resync_starts_with_runtime_and_persisted_delivery_revoked() {
        let root = Fixture::new();
        let store = ready_store(&root.canonical());
        ensure_codex_agent(&store, "codex-test").unwrap();
        let runtime = CodexRuntime::default();
        runtime.replace_authorized_sessions([(
            "session".into(),
            "project".into(),
            root.canonical().to_string_lossy().into_owned(),
            "thread".into(),
        )]);
        assert_eq!(runtime.authorized_session_count(), 1);

        begin_codex_sync(&store, &runtime).unwrap();

        assert_eq!(runtime.authorized_session_count(), 0);
        let transport = store
            .get_integration(CODEX_TRANSPORT_INTEGRATION_ID)
            .unwrap()
            .unwrap();
        let agent = store.get_agent(CODEX_AGENT_ID).unwrap().unwrap();
        assert_eq!(transport.state, IntegrationState::Unknown);
        assert!(!transport.capabilities.direct);
        assert!(!agent.capabilities.direct);
    }

    #[test]
    fn deterministic_ids_do_not_trust_provider_ids_as_local_ids() {
        assert_eq!(
            deterministic_id("session", "../spoof"),
            deterministic_id("session", "../spoof")
        );
        assert!(!deterministic_id("session", "../spoof").contains(".."));
        assert_ne!(
            deterministic_id("session", "a"),
            deterministic_id("session", "b")
        );
    }

    #[test]
    fn spoofed_session_binding_is_rejected_before_owner_intent_is_recorded() {
        let root = Fixture::new();
        let canonical = root.canonical();
        let store = ready_store(&canonical);
        ensure_codex_agent(&store, "codex-test").unwrap();
        let session = Session {
            id: "spoofed-local-session".into(),
            project_id: "project".into(),
            task_id: None,
            agent_id: CODEX_AGENT_ID.into(),
            provider_session_id: Some("provider-thread".into()),
            state: AgentState::Idle,
            started_at_unix_ms: 1,
            last_observed_at_unix_ms: Some(1),
            title_hint: None,
        };
        store.upsert_session(&session).unwrap();

        let error = try_send_direction(
            &store,
            &CodexRuntime::default(),
            &session,
            "inspect safely".into(),
            true,
        )
        .unwrap_err();
        assert!(error.contains("not currently eligible"));
        assert!(store.list_control_requests(&session.id).unwrap().is_empty());
        assert!(
            store
                .list_messages(&session.id, StreamQuery::default())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn codex_delivery_requires_explicit_per_direction_confirmation() {
        let root = Fixture::new();
        let canonical = root.canonical();
        let store = ready_store(&canonical);
        ensure_codex_agent(&store, "codex-test").unwrap();
        let provider_id = "provider-thread";
        let session = Session {
            id: deterministic_id("codex-session", provider_id),
            project_id: "project".into(),
            task_id: None,
            agent_id: CODEX_AGENT_ID.into(),
            provider_session_id: Some(provider_id.into()),
            state: AgentState::Idle,
            started_at_unix_ms: 1,
            last_observed_at_unix_ms: Some(1),
            title_hint: None,
        };
        store.upsert_session(&session).unwrap();

        let error = try_send_direction(
            &store,
            &CodexRuntime::default(),
            &session,
            "do not send this".into(),
            false,
        )
        .unwrap_err();
        assert!(error.contains("explicit confirmation"));
        assert!(store.list_control_requests(&session.id).unwrap().is_empty());
        assert!(
            store
                .list_messages(&session.id, StreamQuery::default())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn capability_escalation_is_rejected_before_provider_delivery() {
        let root = Fixture::new();
        let canonical = root.canonical();
        let store = ready_store(&canonical);
        let capabilities = ConnectorCapabilities {
            observe: true,
            ..ConnectorCapabilities::default()
        };
        store
            .upsert_agent(&Agent {
                id: CODEX_AGENT_ID.into(),
                provider_id: CODEX_PROVIDER_ID.into(),
                connector_id: CODEX_TRANSPORT_INTEGRATION_ID.into(),
                display_name: "Codex without direct".into(),
                model: None,
                capabilities,
            })
            .unwrap();
        let provider_id = "provider-thread";
        let session = Session {
            id: deterministic_id("codex-session", provider_id),
            project_id: "project".into(),
            task_id: None,
            agent_id: CODEX_AGENT_ID.into(),
            provider_session_id: Some(provider_id.into()),
            state: AgentState::Idle,
            started_at_unix_ms: 1,
            last_observed_at_unix_ms: Some(1),
            title_hint: None,
        };
        store.upsert_session(&session).unwrap();

        let error = try_send_direction(
            &store,
            &CodexRuntime::default(),
            &session,
            "attempt escalation".into(),
            true,
        )
        .unwrap_err();
        assert!(error.contains("not currently eligible"));
        assert!(store.list_control_requests(&session.id).unwrap().is_empty());
    }

    #[test]
    fn delivery_receipts_never_claim_turn_completion() {
        let request = utu_core::ControlRequest {
            id: "request".into(),
            session_id: "session".into(),
            action: ControlAction::Direct,
            instruction: Some("inspect".into()),
            requested_at_unix_ms: 1,
            requested_by_owner: true,
        };
        let acknowledged = delivery_receipt(
            &request,
            Ok(DirectionDelivery::Acknowledged(utu_codex::TurnRecord {
                id: "turn".into(),
                ..utu_codex::TurnRecord::default()
            })),
        );
        assert_eq!(acknowledged.outcome, ControlOutcome::Acknowledged);
        assert_eq!(acknowledged.evidence, EvidenceKind::Observed);
        assert!(
            acknowledged
                .message
                .as_deref()
                .unwrap()
                .contains("completion is not yet observed")
        );
        assert_eq!(acknowledged.provider_receipt_id.as_deref(), Some("turn"));

        for invalid_id in [
            String::new(),
            "x".repeat(MAX_PROVIDER_RECEIPT_ID_BYTES + 1),
            "turn\nprivate".into(),
        ] {
            let invalid = delivery_receipt(
                &request,
                Ok(DirectionDelivery::Acknowledged(utu_codex::TurnRecord {
                    id: invalid_id,
                    ..utu_codex::TurnRecord::default()
                })),
            );
            assert_eq!(invalid.outcome, ControlOutcome::Unknown);
            assert_eq!(invalid.evidence, EvidenceKind::Inferred);
            assert!(invalid.provider_receipt_id.is_none());
            assert!(!invalid.message.as_deref().unwrap().contains("private"));
        }

        let timed_out = delivery_receipt(
            &request,
            Ok(DirectionDelivery::TurnFailed(CodexError::Timeout {
                method: "turn/start",
                timeout_ms: 15_000,
            })),
        );
        assert_eq!(timed_out.outcome, ControlOutcome::TimedOut);
        assert!(timed_out.provider_receipt_id.is_none());
        assert!(timed_out.message.as_deref().unwrap().contains("unknown"));

        let pre_turn = delivery_receipt(&request, Ok(DirectionDelivery::PreTurnFailed));
        assert_eq!(pre_turn.outcome, ControlOutcome::Rejected);
        assert_eq!(pre_turn.evidence, EvidenceKind::Observed);
        assert!(pre_turn.message.as_deref().unwrap().contains("not sent"));
    }
}
