use rusqlite::Row;
use utu_core::{
    AgentState, AttentionState, AuthState, ConnectorCapabilities, ControlAction, ControlOutcome,
    CostConfidence, EventKind, EvidenceKind, FileChangeKind, HandoffState, IntegrationState,
    MessageRole, ProjectState, ProviderKind, Severity, TaskState,
};

use crate::{Result, StoreError};

pub(crate) trait DbEnum: Sized {
    const KIND: &'static str;
    fn db_value(self) -> &'static str;
    fn from_db(value: &str) -> Result<Self>;
}

macro_rules! db_enum {
    ($type:ty, $kind:literal, { $($variant:path => $value:literal),+ $(,)? }) => {
        impl DbEnum for $type {
            const KIND: &'static str = $kind;

            fn db_value(self) -> &'static str {
                match self { $($variant => $value),+ }
            }

            fn from_db(value: &str) -> Result<Self> {
                match value {
                    $($value => Ok($variant)),+,
                    _ => Err(StoreError::InvalidEnum {
                        kind: Self::KIND,
                        value: value.to_owned(),
                    }),
                }
            }
        }
    };
}

db_enum!(ProviderKind, "provider kind", {
    ProviderKind::LocalCli => "local_cli",
    ProviderKind::CloudApi => "cloud_api",
    ProviderKind::BrowserMediated => "browser_mediated",
});
db_enum!(EvidenceKind, "evidence kind", {
    EvidenceKind::Observed => "observed",
    EvidenceKind::Inferred => "inferred",
    EvidenceKind::Stale => "stale",
    EvidenceKind::Unsupported => "unsupported",
});
db_enum!(AuthState, "authentication state", {
    AuthState::Confirmed => "confirmed",
    AuthState::Expired => "expired",
    AuthState::Missing => "missing",
    AuthState::Unknown => "unknown",
    AuthState::Unsupported => "unsupported",
});
db_enum!(ProjectState, "project state", {
    ProjectState::Active => "active",
    ProjectState::Paused => "paused",
    ProjectState::Archived => "archived",
});
db_enum!(TaskState, "task state", {
    TaskState::Draft => "draft",
    TaskState::Queued => "queued",
    TaskState::Running => "running",
    TaskState::Waiting => "waiting",
    TaskState::Blocked => "blocked",
    TaskState::Completed => "completed",
    TaskState::Canceled => "canceled",
});
db_enum!(AgentState, "agent state", {
    AgentState::Running => "running",
    AgentState::Waiting => "waiting",
    AgentState::Idle => "idle",
    AgentState::Problem => "problem",
    AgentState::Offline => "offline",
});
db_enum!(IntegrationState, "integration state", {
    IntegrationState::Ready => "ready",
    IntegrationState::Degraded => "degraded",
    IntegrationState::Disabled => "disabled",
    IntegrationState::Unknown => "unknown",
});
db_enum!(EventKind, "event kind", {
    EventKind::Status => "status",
    EventKind::OwnerMessage => "owner_message",
    EventKind::AgentMessage => "agent_message",
    EventKind::ToolCall => "tool_call",
    EventKind::FileChange => "file_change",
    EventKind::Cost => "cost",
    EventKind::Problem => "problem",
    EventKind::Handoff => "handoff",
    EventKind::Log => "log",
});
db_enum!(MessageRole, "message role", {
    MessageRole::Owner => "owner",
    MessageRole::Agent => "agent",
    MessageRole::System => "system",
});
db_enum!(FileChangeKind, "file change kind", {
    FileChangeKind::Added => "added",
    FileChangeKind::Modified => "modified",
    FileChangeKind::Deleted => "deleted",
    FileChangeKind::Renamed => "renamed",
});
db_enum!(CostConfidence, "cost confidence", {
    CostConfidence::Exact => "exact",
    CostConfidence::Estimated => "estimated",
    CostConfidence::Partial => "partial",
    CostConfidence::Unknown => "unknown",
});
db_enum!(Severity, "severity", {
    Severity::Blocked => "blocked",
    Severity::NeedsAttention => "needs_attention",
    Severity::Healthy => "healthy",
    Severity::Unknown => "unknown",
});
db_enum!(AttentionState, "attention state", {
    AttentionState::Open => "open",
    AttentionState::Acknowledged => "acknowledged",
    AttentionState::Resolved => "resolved",
});
db_enum!(HandoffState, "handoff state", {
    HandoffState::Requested => "requested",
    HandoffState::Approved => "approved",
    HandoffState::Delivered => "delivered",
    HandoffState::Failed => "failed",
    HandoffState::Canceled => "canceled",
    HandoffState::Unknown => "unknown",
});
db_enum!(ControlAction, "control action", {
    ControlAction::Direct => "direct",
    ControlAction::Pause => "pause",
    ControlAction::Resume => "resume",
    ControlAction::Stop => "stop",
});
db_enum!(ControlOutcome, "control outcome", {
    ControlOutcome::Acknowledged => "acknowledged",
    ControlOutcome::Rejected => "rejected",
    ControlOutcome::TimedOut => "timed_out",
    ControlOutcome::Unsupported => "unsupported",
    ControlOutcome::Unknown => "unknown",
});

pub(crate) fn to_i64(value: u64, field: &'static str) -> Result<i64> {
    i64::try_from(value).map_err(|_| StoreError::IntegerOverflow { field, value })
}

pub(crate) fn optional_to_i64(value: Option<u64>, field: &'static str) -> Result<Option<i64>> {
    value.map(|value| to_i64(value, field)).transpose()
}

pub(crate) fn to_u64(value: i64, field: &'static str) -> Result<u64> {
    u64::try_from(value).map_err(|_| StoreError::NegativeInteger { field, value })
}

pub(crate) fn optional_to_u64(value: Option<i64>, field: &'static str) -> Result<Option<u64>> {
    value.map(|value| to_u64(value, field)).transpose()
}

pub(crate) fn bool_i64(value: bool) -> i64 {
    i64::from(value)
}

pub(crate) fn read_bool(row: &Row<'_>, index: usize) -> rusqlite::Result<bool> {
    Ok(row.get::<_, i64>(index)? != 0)
}

pub(crate) fn read_capabilities(
    row: &Row<'_>,
    start: usize,
) -> rusqlite::Result<ConnectorCapabilities> {
    Ok(ConnectorCapabilities {
        observe: read_bool(row, start)?,
        auth_probe: read_bool(row, start + 1)?,
        direct: read_bool(row, start + 2)?,
        pause: read_bool(row, start + 3)?,
        resume: read_bool(row, start + 4)?,
        stop: read_bool(row, start + 5)?,
        logs: read_bool(row, start + 6)?,
        costs: read_bool(row, start + 7)?,
        agent_messages: read_bool(row, start + 8)?,
    })
}
