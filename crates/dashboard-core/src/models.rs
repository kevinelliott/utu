use serde::{Deserialize, Serialize};

pub type EntityId = String;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    #[default]
    LocalCli,
    CloudApi,
    BrowserMediated,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    #[default]
    Observed,
    Inferred,
    Stale,
    Unsupported,
}

impl EvidenceKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Observed => "Observed",
            Self::Inferred => "Inferred",
            Self::Stale => "Stale",
            Self::Unsupported => "Unsupported",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Blocked,
    NeedsAttention,
    #[default]
    Healthy,
    Unknown,
}

impl Severity {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Blocked => "Blocked",
            Self::NeedsAttention => "Needs attention",
            Self::Healthy => "Healthy",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Running,
    Waiting,
    #[default]
    Idle,
    Problem,
    Offline,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthState {
    Confirmed,
    Expired,
    Missing,
    #[default]
    Unknown,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsolationMode {
    #[default]
    Host,
    ProcessSandbox,
    Container,
    LocalVm,
    RemoteVm,
}

impl IsolationMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Host => "Host",
            Self::ProcessSandbox => "Process sandbox",
            Self::Container => "Container",
            Self::LocalVm => "Local VM",
            Self::RemoteVm => "Remote VM",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ControlCapabilities {
    pub direct: bool,
    pub pause: bool,
    pub resume: bool,
    pub stop: bool,
    pub handoff: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConnectorCapabilities {
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

impl ConnectorCapabilities {
    pub const OBSERVE_ONLY: Self = Self {
        observe: true,
        auth_probe: false,
        direct: false,
        pause: false,
        resume: false,
        stop: false,
        logs: false,
        costs: false,
        agent_messages: false,
    };
}

impl ControlCapabilities {
    pub const READ_ONLY: Self = Self {
        direct: false,
        pause: false,
        resume: false,
        stop: false,
        handoff: false,
    };

    pub const FULL: Self = Self {
        direct: true,
        pause: true,
        resume: true,
        stop: true,
        handoff: true,
    };
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostConfidence {
    Exact,
    Estimated,
    Partial,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CostAmount {
    pub currency: String,
    /// One millionth of the named currency. Integer storage avoids float drift.
    pub micros: u64,
    pub confidence: CostConfidence,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Evidence<T> {
    pub value: Option<T>,
    pub kind: EvidenceKind,
    pub source: String,
    pub observed_at_unix_ms: Option<u64>,
    pub note: Option<String>,
}

impl<T> Evidence<T> {
    pub fn unsupported(source: impl Into<String>, note: impl Into<String>) -> Self {
        Self {
            value: None,
            kind: EvidenceKind::Unsupported,
            source: source.into(),
            observed_at_unix_ms: None,
            note: Some(note.into()),
        }
    }
}

impl CostAmount {
    pub fn usd_estimate(micros: u64) -> Self {
        Self {
            currency: "USD".into(),
            micros,
            confidence: CostConfidence::Estimated,
        }
    }

    pub fn display(&self) -> String {
        if self.confidence == CostConfidence::Unknown {
            return "Unknown".into();
        }

        let dollars = self.micros / 1_000_000;
        let cents = (self.micros % 1_000_000) / 10_000;
        let prefix = match self.confidence {
            CostConfidence::Estimated | CostConfidence::Partial => "~",
            CostConfidence::Exact | CostConfidence::Unknown => "",
        };
        format!("{prefix}${dollars}.{cents:02}")
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentSnapshot {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub project: Option<String>,
    pub state: AgentState,
    pub auth: AuthState,
    pub evidence: EvidenceKind,
    pub evidence_age_seconds: Option<u64>,
    pub isolation: IsolationMode,
    pub controls: ControlCapabilities,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConnectorProbe {
    pub connector_id: String,
    pub installed: bool,
    pub reachable: bool,
    pub auth: AuthState,
    pub evidence: EvidenceKind,
    pub checked_at_unix_ms: u64,
    pub problem: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectState {
    #[default]
    Active,
    Paused,
    Archived,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: EntityId,
    pub name: String,
    pub root_path: Option<String>,
    pub state: ProjectState,
    pub created_at_unix_ms: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    #[default]
    Draft,
    Queued,
    Running,
    Waiting,
    Blocked,
    Completed,
    Canceled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: EntityId,
    pub project_id: EntityId,
    pub title: String,
    pub detail: String,
    pub state: TaskState,
    /// Explicitly supports one task assigned to multiple agents.
    pub assignee_agent_ids: Vec<EntityId>,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Agent {
    pub id: EntityId,
    pub provider_id: EntityId,
    pub connector_id: EntityId,
    pub display_name: String,
    pub model: Option<String>,
    pub capabilities: ConnectorCapabilities,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Provider {
    pub id: EntityId,
    pub display_name: String,
    pub kind: ProviderKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub id: EntityId,
    pub project_id: EntityId,
    pub task_id: Option<EntityId>,
    pub agent_id: EntityId,
    pub provider_session_id: Option<String>,
    pub state: AgentState,
    pub started_at_unix_ms: u64,
    pub last_observed_at_unix_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    #[default]
    Status,
    OwnerMessage,
    AgentMessage,
    ToolCall,
    FileChange,
    Cost,
    Problem,
    Handoff,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SessionEvent {
    pub id: EntityId,
    pub session_id: EntityId,
    pub sequence: u64,
    pub occurred_at_unix_ms: u64,
    pub kind: EventKind,
    pub summary: String,
    pub evidence: EvidenceKind,
    pub correlation_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentHandoff {
    pub id: EntityId,
    pub project_id: EntityId,
    pub task_id: EntityId,
    pub from_agent_id: EntityId,
    pub to_agent_id: EntityId,
    pub instruction: String,
    pub created_at_unix_ms: u64,
    pub approved_by_owner: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttentionFinding {
    pub severity: Severity,
    pub title: String,
    pub recovery: Option<String>,
}
