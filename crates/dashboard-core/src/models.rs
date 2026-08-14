use serde::{Deserialize, Serialize};
use thiserror::Error;

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
    Observed,
    #[default]
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
    Healthy,
    #[default]
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
    /// `None` means the amount is genuinely unknown. Unknown must never be
    /// persisted as zero because zero is a concrete measured amount.
    pub micros: Option<u64>,
    pub confidence: CostConfidence,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CostInvariantError {
    #[error("an unknown cost cannot contain a numeric amount")]
    UnknownHasAmount,
    #[error("a known cost must contain a numeric amount")]
    KnownMissingAmount,
    #[error("currency must be a three-letter ASCII code")]
    InvalidCurrency,
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
    pub fn new(
        currency: impl Into<String>,
        micros: Option<u64>,
        confidence: CostConfidence,
    ) -> Result<Self, CostInvariantError> {
        let amount = Self {
            currency: currency.into().to_ascii_uppercase(),
            micros,
            confidence,
        };
        amount.validate()?;
        Ok(amount)
    }

    pub fn unknown(currency: impl Into<String>) -> Result<Self, CostInvariantError> {
        Self::new(currency, None, CostConfidence::Unknown)
    }

    pub fn usd_exact(micros: u64) -> Self {
        Self {
            currency: "USD".into(),
            micros: Some(micros),
            confidence: CostConfidence::Exact,
        }
    }

    pub fn usd_estimate(micros: u64) -> Self {
        Self {
            currency: "USD".into(),
            micros: Some(micros),
            confidence: CostConfidence::Estimated,
        }
    }

    pub fn validate(&self) -> Result<(), CostInvariantError> {
        if self.currency.len() != 3 || !self.currency.bytes().all(|byte| byte.is_ascii_alphabetic())
        {
            return Err(CostInvariantError::InvalidCurrency);
        }

        match (self.confidence, self.micros) {
            (CostConfidence::Unknown, Some(_)) => Err(CostInvariantError::UnknownHasAmount),
            (CostConfidence::Unknown, None) => Ok(()),
            (_, None) => Err(CostInvariantError::KnownMissingAmount),
            (_, Some(_)) => Ok(()),
        }
    }

    pub fn display(&self) -> String {
        let Some(micros) = self.micros else {
            return "Unknown".into();
        };

        let prefix = match self.confidence {
            CostConfidence::Estimated | CostConfidence::Partial => "~",
            CostConfidence::Exact | CostConfidence::Unknown => "",
        };
        if micros > 0 && micros < 10_000 {
            return if self.currency.eq_ignore_ascii_case("USD") {
                format!("{prefix}<$0.01")
            } else {
                format!("{prefix}<{} 0.01", self.currency)
            };
        }

        // Round half up to display precision instead of silently understating
        // known cost. `u128` keeps the upper `u64` boundary exact.
        let rounded_cents = (u128::from(micros) + 5_000) / 10_000;
        let units = rounded_cents / 100;
        let cents = rounded_cents % 100;
        if self.currency.eq_ignore_ascii_case("USD") {
            format!("{prefix}${units}.{cents:02}")
        } else {
            format!("{prefix}{} {units}.{cents:02}", self.currency)
        }
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

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationState {
    Ready,
    Degraded,
    Disabled,
    #[default]
    Unknown,
}

/// A configured connector boundary. It contains normalized status only;
/// credential material belongs in the operating-system keychain.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Integration {
    pub id: EntityId,
    pub provider_id: Option<EntityId>,
    pub connector_key: String,
    pub display_name: String,
    pub kind: ProviderKind,
    pub state: IntegrationState,
    pub auth: AuthState,
    pub evidence: EvidenceKind,
    pub checked_at_unix_ms: Option<u64>,
    pub problem: Option<String>,
    pub capabilities: ConnectorCapabilities,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_hint: Option<String>,
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
    Log,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SessionEvent {
    pub id: EntityId,
    pub session_id: EntityId,
    pub sequence: u64,
    pub occurred_at_unix_ms: u64,
    pub kind: EventKind,
    pub summary: String,
    pub detail: Option<String>,
    pub evidence: EvidenceKind,
    pub source: String,
    pub ingested_at_unix_ms: u64,
    pub provider_event_id: Option<String>,
    pub correlation_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    Owner,
    Agent,
    #[default]
    System,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: EntityId,
    pub session_id: EntityId,
    pub sequence: u64,
    pub role: MessageRole,
    pub author_agent_id: Option<EntityId>,
    pub body: String,
    pub sent_at_unix_ms: u64,
    pub ingested_at_unix_ms: u64,
    pub evidence: EvidenceKind,
    pub source: String,
    pub correlation_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeKind {
    Added,
    #[default]
    Modified,
    Deleted,
    Renamed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileChange {
    pub id: EntityId,
    pub session_id: EntityId,
    pub event_id: Option<EntityId>,
    pub path: String,
    pub previous_path: Option<String>,
    pub kind: FileChangeKind,
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
    pub occurred_at_unix_ms: u64,
    pub evidence: EvidenceKind,
    pub source: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CostRecord {
    pub id: EntityId,
    pub project_id: EntityId,
    pub task_id: Option<EntityId>,
    pub session_id: Option<EntityId>,
    pub agent_id: Option<EntityId>,
    pub amount: CostAmount,
    pub occurred_at_unix_ms: u64,
    pub ingested_at_unix_ms: u64,
    pub evidence: EvidenceKind,
    pub source: String,
    pub note: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionState {
    #[default]
    Open,
    Acknowledged,
    Resolved,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttentionRecord {
    pub id: EntityId,
    pub project_id: Option<EntityId>,
    pub task_id: Option<EntityId>,
    pub session_id: Option<EntityId>,
    pub agent_id: Option<EntityId>,
    pub integration_id: Option<EntityId>,
    pub severity: Severity,
    pub state: AttentionState,
    pub title: String,
    pub detail: Option<String>,
    pub recovery: Option<String>,
    pub detected_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
    pub evidence: EvidenceKind,
    pub source: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandoffState {
    #[default]
    Requested,
    Approved,
    Delivered,
    Failed,
    Canceled,
    Unknown,
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
    pub state: HandoffState,
    pub delivered_at_unix_ms: Option<u64>,
    pub delivery_evidence: EvidenceKind,
    pub source: String,
    pub resulting_session_id: Option<EntityId>,
    pub correlation_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlAction {
    Direct,
    Pause,
    Resume,
    Stop,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlOutcome {
    Acknowledged,
    Rejected,
    TimedOut,
    Unsupported,
    #[default]
    Unknown,
}

/// Owner intent is recorded separately from provider acknowledgement. Merely
/// writing this request never proves that a control reached the provider.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ControlRequest {
    pub id: EntityId,
    pub session_id: EntityId,
    pub action: ControlAction,
    pub instruction: Option<String>,
    pub requested_at_unix_ms: u64,
    pub requested_by_owner: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ControlReceipt {
    pub id: EntityId,
    pub request_id: EntityId,
    pub outcome: ControlOutcome,
    pub received_at_unix_ms: u64,
    pub evidence: EvidenceKind,
    pub source: String,
    pub message: Option<String>,
    pub provider_receipt_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchEntityKind {
    Message,
    Event,
    FileChange,
    Task,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    pub entity: SearchEntityKind,
    pub id: EntityId,
    pub project_id: EntityId,
    pub session_id: Option<EntityId>,
    pub occurred_at_unix_ms: u64,
    pub title: String,
    pub excerpt: String,
    pub evidence: EvidenceKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttentionFinding {
    pub severity: Severity,
    pub title: String,
    pub recovery: Option<String>,
}
