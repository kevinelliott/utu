#![allow(dead_code)] // Mirrors the complete native command contract; views project a safe subset.

use js_sys::{Function, Promise, Reflect};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeSurface {
    Desktop,
    Web,
}

impl RuntimeSurface {
    pub fn detect() -> Self {
        if web_status_requested() || tauri_invoke().is_none() {
            Self::Web
        } else {
            Self::Desktop
        }
    }

    pub const fn is_desktop(self) -> bool {
        matches!(self, Self::Desktop)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub generated_at_unix_ms: u64,
    pub store: StoreStatus,
    pub projects: Vec<ProjectRecord>,
    pub tasks: Vec<TaskRecord>,
    pub agents: Vec<AgentRecord>,
    pub sessions: Vec<SessionRecord>,
    pub integrations: Vec<IntegrationRecord>,
    pub attention: Vec<AttentionRecord>,
    pub costs: Vec<ProjectCostSummary>,
    pub provider_delivery: Vec<ProviderDeliveryEligibility>,
}

impl WorkspaceSnapshot {
    pub fn session_can_receive_direction(&self, session_id: &str) -> bool {
        self.provider_delivery
            .iter()
            .find(|eligibility| eligibility.session_id == session_id)
            .is_some_and(|eligibility| eligibility.eligible)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreStatus {
    pub schema_version: u32,
    pub latest_supported_schema_version: u32,
    pub integrity_ok: bool,
    pub foreign_keys_enabled: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub root_path: Option<String>,
    pub state: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub state: String,
    pub assignee_agent_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AgentRecord {
    pub id: String,
    pub connector_id: String,
    pub display_name: String,
    pub model: Option<String>,
    pub capabilities: CoreCapabilities,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub project_id: String,
    pub task_id: Option<String>,
    pub agent_id: String,
    pub provider_session_id: Option<String>,
    pub state: String,
    #[serde(default)]
    pub started_at_unix_ms: u64,
    pub last_observed_at_unix_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct CoreCapabilities {
    pub observe: bool,
    pub auth_probe: bool,
    pub direct: bool,
    pub pause: bool,
    pub resume: bool,
    pub stop: bool,
    pub logs: bool,
    pub costs: bool,
    pub agent_messages: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct IntegrationRecord {
    pub id: String,
    pub connector_key: String,
    pub display_name: String,
    pub kind: String,
    pub state: String,
    pub auth: String,
    pub evidence: String,
    pub checked_at_unix_ms: Option<u64>,
    pub problem: Option<String>,
    pub capabilities: CoreCapabilities,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AttentionRecord {
    pub severity: String,
    pub title: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDeliveryEligibility {
    pub session_id: String,
    pub provider_id: Option<String>,
    pub eligible: bool,
    pub requires_confirmation: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCostSummary {
    pub project_id: String,
    pub amount: CostAmount,
    pub complete: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CostAmount {
    pub currency: String,
    pub micros: Option<u64>,
    pub confidence: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticReport {
    pub checked_at_unix_ms: u64,
    pub connectors: Vec<ConnectorDiagnostic>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConnectorDiagnostic {
    pub descriptor: ConnectorDescriptor,
    pub installation: DiagnosticEvidence<String>,
    pub version: DiagnosticEvidence<String>,
    pub auth: AuthDiagnostic,
    pub readiness: String,
    pub health: String,
    pub problems: Vec<DiagnosticProblem>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorDescriptor {
    pub id: String,
    pub display_name: String,
    pub provider_id: String,
    pub executable: String,
    pub current_capabilities: AdapterCapabilities,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterCapabilities {
    pub discover: bool,
    pub version_probe: bool,
    pub auth_probe: bool,
    pub sessions: bool,
    pub chat: bool,
    pub files: bool,
    pub event_stream: bool,
    pub logs: bool,
    pub costs: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticEvidence<T> {
    pub status: String,
    pub kind: String,
    pub value: Option<T>,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AuthDiagnostic {
    pub state: String,
    pub status: String,
    pub kind: String,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DiagnosticProblem {
    pub code: String,
    pub severity: String,
    pub summary: String,
    pub recovery: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDirectory {
    pub relative_path: String,
    pub entries: Vec<ProjectFileEntry>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileEntry {
    pub name: String,
    pub relative_path: String,
    pub kind: String,
    pub size_bytes: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFilePreview {
    pub relative_path: String,
    pub content: Option<String>,
    pub size_bytes: u64,
    pub truncated: bool,
    pub binary: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DirectionResult {
    pub receipt: ControlReceipt,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ControlReceipt {
    pub outcome: String,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStream {
    pub session: SessionRecord,
    pub messages: Vec<MessageRecord>,
    pub events: Vec<SessionEventRecord>,
    pub file_changes: Vec<FileChangeRecord>,
    pub costs: Vec<SessionCostRecord>,
    pub control_requests: Vec<ControlRequestRecord>,
    pub control_receipts: Vec<ControlReceiptRecord>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MessageRecord {
    pub id: String,
    pub sequence: u64,
    pub role: String,
    pub author_agent_id: Option<String>,
    pub body: String,
    pub sent_at_unix_ms: u64,
    pub evidence: String,
    pub source: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SessionEventRecord {
    pub id: String,
    pub sequence: u64,
    pub occurred_at_unix_ms: u64,
    pub kind: String,
    pub summary: String,
    pub detail: Option<String>,
    pub evidence: String,
    pub source: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FileChangeRecord {
    pub id: String,
    pub path: String,
    pub kind: String,
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
    pub evidence: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SessionCostRecord {
    pub id: String,
    pub amount: CostAmount,
    pub evidence: String,
    pub source: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ControlRequestRecord {
    pub id: String,
    pub action: String,
    pub requested_at_unix_ms: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ControlReceiptRecord {
    pub id: String,
    pub request_id: String,
    pub outcome: String,
    pub received_at_unix_ms: u64,
    pub evidence: String,
    pub source: String,
    pub message: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotArgs<'a> {
    project_id: Option<&'a str>,
}

#[derive(Serialize)]
struct EmptyArgs {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateProjectArgs<'a> {
    input: CreateProjectInput<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateProjectInput<'a> {
    name: &'a str,
    root_path: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateTaskArgs<'a> {
    input: CreateTaskInput<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateTaskInput<'a> {
    project_id: &'a str,
    title: &'a str,
    detail: &'a str,
    assignee_agent_ids: &'a [String],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DirectionArgs<'a> {
    input: DirectionInput<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DirectionInput<'a> {
    session_id: &'a str,
    body: &'a str,
    allow_provider_delivery: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProjectSessionsSummary {
    pub metadata_only: bool,
    pub imported_sessions: u32,
    pub transcripts_imported: u32,
    pub agents: Vec<AgentSyncSummary>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSyncSummary {
    pub agent_id: String,
    pub display_name: String,
    pub status: String,
    pub imported_sessions: u32,
    pub detail: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncProjectSessionsArgs {
    input: SyncProjectSessionsInput,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncProjectSessionsInput {
    confirmed_metadata_sync: bool,
    project_ids: Vec<String>,
    all_projects: bool,
    import_transcripts: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionStreamArgs<'a> {
    session_id: &'a str,
    after_message_sequence: Option<u64>,
    after_event_sequence: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectDirectoryArgs<'a> {
    project_id: &'a str,
    relative_path: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectPreviewArgs<'a> {
    project_id: &'a str,
    relative_path: &'a str,
    max_bytes: Option<usize>,
}

pub async fn workspace_snapshot(project_id: Option<&str>) -> Result<WorkspaceSnapshot, String> {
    invoke("workspace_snapshot", &SnapshotArgs { project_id }).await
}

pub async fn refresh_connectors() -> Result<DiagnosticReport, String> {
    invoke("refresh_connectors", &EmptyArgs {}).await
}

pub async fn create_project(name: &str, root_path: &str) -> Result<ProjectRecord, String> {
    invoke(
        "create_project",
        &CreateProjectArgs {
            input: CreateProjectInput {
                name,
                root_path: Some(root_path),
            },
        },
    )
    .await
}

pub async fn create_task(
    project_id: &str,
    title: &str,
    detail: &str,
    assignee_agent_ids: &[String],
) -> Result<TaskRecord, String> {
    invoke(
        "create_task",
        &CreateTaskArgs {
            input: CreateTaskInput {
                project_id,
                title,
                detail,
                assignee_agent_ids,
            },
        },
    )
    .await
}

pub async fn send_direction(
    session_id: &str,
    body: &str,
    allow_provider_delivery: bool,
) -> Result<DirectionResult, String> {
    invoke(
        "send_direction",
        &DirectionArgs {
            input: DirectionInput {
                session_id,
                body,
                allow_provider_delivery,
            },
        },
    )
    .await
}

pub async fn sync_project_sessions(
    project_id: Option<&str>,
) -> Result<SyncProjectSessionsSummary, String> {
    let (project_ids, all_projects) = match project_id {
        Some(project_id) => (vec![project_id.to_owned()], false),
        None => (Vec::new(), true),
    };
    invoke(
        "sync_project_sessions",
        &SyncProjectSessionsArgs {
            input: SyncProjectSessionsInput {
                confirmed_metadata_sync: true,
                project_ids,
                all_projects,
                import_transcripts: false,
            },
        },
    )
    .await
}

pub async fn latest_connector_report() -> Result<Option<DiagnosticReport>, String> {
    invoke("latest_connector_report", &EmptyArgs {}).await
}

pub async fn session_stream(session_id: &str) -> Result<SessionStream, String> {
    invoke(
        "session_stream",
        &SessionStreamArgs {
            session_id,
            after_message_sequence: None,
            after_event_sequence: None,
        },
    )
    .await
}

pub async fn project_directory(
    project_id: &str,
    relative_path: Option<&str>,
) -> Result<ProjectDirectory, String> {
    invoke(
        "project_directory",
        &ProjectDirectoryArgs {
            project_id,
            relative_path,
        },
    )
    .await
}

pub async fn project_file_preview(
    project_id: &str,
    relative_path: &str,
    max_bytes: Option<usize>,
) -> Result<ProjectFilePreview, String> {
    invoke(
        "project_file_preview",
        &ProjectPreviewArgs {
            project_id,
            relative_path,
            max_bytes,
        },
    )
    .await
}

pub async fn pick_folder() -> Result<Option<String>, String> {
    invoke("pick_folder", &EmptyArgs {}).await
}

async fn invoke<T: DeserializeOwned>(command: &str, args: &impl Serialize) -> Result<T, String> {
    let invoke = tauri_invoke().ok_or_else(|| {
        "The native Utu command bridge is unavailable on this surface.".to_owned()
    })?;
    let args = serde_wasm_bindgen::to_value(args)
        .map_err(|error| format!("could not encode `{command}` input: {error}"))?;
    let promise = invoke
        .call2(&JsValue::UNDEFINED, &JsValue::from_str(command), &args)
        .map_err(js_error)?
        .dyn_into::<Promise>()
        .map_err(|_| format!("`{command}` did not return a Promise"))?;
    let value = JsFuture::from(promise).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(value)
        .map_err(|error| format!("could not decode `{command}` result: {error}"))
}

pub fn listen_workspace_changed(on_change: impl Fn() + 'static) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(tauri) = Reflect::get(window.as_ref(), &JsValue::from_str("__TAURI__")) else {
        return;
    };
    if tauri.is_null() || tauri.is_undefined() {
        return;
    }
    let Ok(event) = Reflect::get(&tauri, &JsValue::from_str("event")) else {
        return;
    };
    let Ok(listen) = Reflect::get(&event, &JsValue::from_str("listen")) else {
        return;
    };
    let Ok(listen) = listen.dyn_into::<Function>() else {
        return;
    };
    let callback = Closure::wrap(Box::new(move |_event: JsValue| {
        on_change();
    }) as Box<dyn FnMut(JsValue)>);
    let _ = listen.call2(
        &event,
        &JsValue::from_str("utu-workspace-changed"),
        callback.as_ref().unchecked_ref(),
    );
    callback.forget();
}

fn tauri_invoke() -> Option<Function> {
    let window = web_sys::window()?;
    let tauri = Reflect::get(window.as_ref(), &JsValue::from_str("__TAURI__")).ok()?;
    if tauri.is_null() || tauri.is_undefined() {
        return None;
    }
    let core = Reflect::get(&tauri, &JsValue::from_str("core")).ok()?;
    Reflect::get(&core, &JsValue::from_str("invoke"))
        .ok()?
        .dyn_into::<Function>()
        .ok()
}

fn web_status_requested() -> bool {
    query_contains("surface=web") || query_contains("readonly=1")
}

pub fn is_about_window() -> bool {
    query_contains("window=about")
}

pub fn query_param(name: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    param_from_search(&search, name)
}

pub(crate) fn param_from_search(search: &str, name: &str) -> Option<String> {
    let query = search.trim_start_matches('?');
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == name).then(|| value.to_owned())
    })
}

pub async fn close_about_window() -> Result<(), String> {
    invoke("close_about_window", &EmptyArgs {}).await
}

fn query_contains(needle: &str) -> bool {
    web_sys::window()
        .and_then(|window| window.location().search().ok())
        .is_some_and(|search| search.contains(needle))
}

fn js_error(value: JsValue) -> String {
    value
        .as_string()
        .or_else(|| {
            Reflect::get(&value, &JsValue::from_str("message"))
                .ok()?
                .as_string()
        })
        .unwrap_or_else(|| "native command failed without an error message".into())
}

#[cfg(test)]
mod provider_delivery_tests {
    use super::WorkspaceSnapshot;

    #[test]
    fn snapshot_decodes_provider_delivery_from_camel_case_wire_fields() {
        let snapshot: WorkspaceSnapshot = serde_json::from_value(serde_json::json!({
            "generatedAtUnixMs": 1,
            "store": {
                "schemaVersion": 3,
                "latestSupportedSchemaVersion": 3,
                "integrityOk": true,
                "foreignKeysEnabled": true
            },
            "projects": [],
            "tasks": [],
            "agents": [],
            "sessions": [],
            "integrations": [],
            "attention": [],
            "costs": [],
            "providerDelivery": [{
                "sessionId": "session-codex-1",
                "providerId": "openai",
                "eligible": true,
                "requiresConfirmation": true
            }]
        }))
        .expect("native snapshot wire shape should decode");

        assert!(snapshot.session_can_receive_direction("session-codex-1"));
        assert!(!snapshot.session_can_receive_direction("session-other"));
        assert_eq!(
            snapshot.provider_delivery[0].provider_id.as_deref(),
            Some("openai")
        );
        assert!(snapshot.provider_delivery[0].requires_confirmation);
    }
}

#[cfg(test)]
mod tests {
    use super::WorkspaceSnapshot;

    #[test]
    fn workspace_snapshot_matches_mixed_native_wire_casing() {
        let json = serde_json::json!({
            "generatedAtUnixMs": 42,
            "scope": { "projectId": null, "agentsAreGlobal": true, "integrationsAreGlobal": true },
            "store": {
                "schemaVersion": 1,
                "latestSupportedSchemaVersion": 1,
                "integrityOk": true,
                "foreignKeysEnabled": true
            },
            "projects": [{
                "id": "project-1", "name": "Utu", "root_path": "/tmp/utu", "state": "active",
                "created_at_unix_ms": 1
            }],
            "tasks": [],
            "agents": [{
                "id": "agent-1", "provider_id": "provider-1", "connector_id": "codex",
                "display_name": "Codex", "model": null,
                "capabilities": {
                    "observe": true, "auth_probe": true, "direct": true, "pause": false,
                    "resume": false, "stop": false, "logs": true, "costs": false,
                    "agent_messages": true
                }
            }],
            "sessions": [], "integrations": [], "attention": [], "handoffs": [],
            "costs": [{
                "projectId": "project-1",
                "amount": { "currency": "USD", "micros": null, "confidence": "unknown" },
                "knownRecords": 0,
                "unknownRecords": 0,
                "complete": false
            }],
            "providerDelivery": []
        });

        let snapshot: WorkspaceSnapshot =
            serde_json::from_value(json).expect("native snapshot contract should decode");
        assert_eq!(snapshot.projects[0].root_path.as_deref(), Some("/tmp/utu"));
        assert!(snapshot.agents[0].capabilities.auth_probe);
        assert!(snapshot.agents[0].capabilities.agent_messages);
    }
}

#[cfg(test)]
mod query_param_tests {
    use super::param_from_search;

    #[test]
    fn reads_about_window_and_version_from_search() {
        let search = "?window=about&version=0.1.0";
        assert_eq!(
            param_from_search(search, "window").as_deref(),
            Some("about")
        );
        assert_eq!(
            param_from_search(search, "version").as_deref(),
            Some("0.1.0")
        );
        assert_eq!(param_from_search(search, "surface"), None);
    }
}
