use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use utu_core::{
    Agent, AgentHandoff, AgentState, AttentionRecord, AttentionState, AuthState,
    ConnectorCapabilities, ControlAction, ControlOutcome, ControlReceipt, ControlRequest,
    CostAmount, CostConfidence, CostRecord, EventKind, EvidenceKind, FileChange, FileChangeKind,
    HandoffState, Integration, IntegrationState, Message, MessageRole, Project, ProjectState,
    Provider, ProviderKind, SearchEntityKind, SearchHit, Session, SessionEvent, Severity, Task,
    TaskState,
};

use crate::{
    Result, Store, StoreError,
    codec::{
        DbEnum, bool_i64, optional_to_i64, optional_to_u64, read_bool, read_capabilities, to_i64,
        to_u64,
    },
};

const CAPABILITY_COLUMNS: &str = "can_observe, can_auth_probe, can_direct, can_pause, \
    can_resume, can_stop, can_logs, can_costs, can_agent_messages";
const COST_SUMMARY_PAGE_ROWS: i64 = 4_096;
const COST_SUMMARY_LIMB_BASE: u128 = 4_294_967_296;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamQuery {
    pub after_sequence: Option<u64>,
    pub limit: u32,
}

impl Default for StreamQuery {
    fn default() -> Self {
        Self {
            after_sequence: None,
            limit: 200,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewMessage {
    pub id: String,
    pub session_id: String,
    pub role: MessageRole,
    pub author_agent_id: Option<String>,
    pub body: String,
    pub sent_at_unix_ms: u64,
    pub ingested_at_unix_ms: u64,
    pub evidence: EvidenceKind,
    pub source: String,
    pub correlation_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedDirection {
    pub message: Message,
    pub request: ControlRequest,
    pub receipt: ControlReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedControl {
    pub request: ControlRequest,
    pub receipt: ControlReceipt,
}

/// Selects the operational slice of a coherent workspace projection. Provider,
/// integration, and agent inventory remains global in both modes so consumers
/// never mistake a project filter for a partial Fleet inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceScope {
    Global,
    Project(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCostProjection {
    pub project_id: String,
    pub summary: CostSummary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceProjection {
    pub scope: WorkspaceScope,
    pub health: crate::StoreHealth,
    /// Global inventory, independent of `scope`.
    pub providers: Vec<Provider>,
    /// Global inventory, independent of `scope`.
    pub integrations: Vec<Integration>,
    /// Global inventory, independent of `scope`.
    pub agents: Vec<Agent>,
    /// Global or exactly one requested project.
    pub projects: Vec<Project>,
    /// Operational records restricted to `scope`.
    pub tasks: Vec<Task>,
    pub sessions: Vec<Session>,
    pub attention: Vec<AttentionRecord>,
    pub handoffs: Vec<AgentHandoff>,
    pub costs: Vec<ProjectCostProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionProjection {
    pub session: Session,
    pub messages: Vec<Message>,
    pub events: Vec<SessionEvent>,
    pub file_changes: Vec<FileChange>,
    pub costs: Vec<CostRecord>,
    pub control_requests: Vec<ControlRequest>,
    pub control_receipts: Vec<ControlReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewSessionEvent {
    pub id: String,
    pub session_id: String,
    pub occurred_at_unix_ms: u64,
    pub ingested_at_unix_ms: u64,
    pub kind: EventKind,
    pub summary: String,
    pub detail: Option<String>,
    pub evidence: EvidenceKind,
    pub source: String,
    pub provider_event_id: Option<String>,
    pub correlation_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchQuery {
    pub text: String,
    pub project_id: Option<String>,
    pub session_id: Option<String>,
    pub limit: u32,
}

impl SearchQuery {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            project_id: None,
            session_id: None,
            limit: 100,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CostQuery {
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub currency: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CostSummary {
    pub currency: String,
    pub known_micros: u64,
    pub known_records: u64,
    pub unknown_records: u64,
    pub confidence: CostConfidence,
}

impl CostSummary {
    pub fn amount(&self) -> CostAmount {
        let confidence = if self.known_records == 0 {
            CostConfidence::Unknown
        } else {
            self.confidence
        };
        CostAmount::new(
            self.currency.clone(),
            (self.known_records > 0).then_some(self.known_micros),
            confidence,
        )
        .expect("CostSummary always satisfies CostAmount invariants")
    }

    pub const fn is_complete(&self) -> bool {
        self.known_records > 0
            && self.unknown_records == 0
            && matches!(self.confidence, CostConfidence::Exact)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AttentionQuery {
    pub project_id: Option<String>,
    pub state: Option<AttentionState>,
    pub minimum_severity: Option<Severity>,
    pub limit: Option<u32>,
}

impl Store {
    /// Reads one coherent workspace projection under a SQLite read transaction.
    /// External writers may continue in WAL mode, but no field in the returned
    /// model can observe a different database snapshot from another field.
    pub fn read_workspace_projection(
        &self,
        scope: WorkspaceScope,
        currency: &str,
    ) -> Result<WorkspaceProjection> {
        self.read_workspace_projection_with_hook(scope, currency, || {})
    }

    fn read_workspace_projection_with_hook(
        &self,
        scope: WorkspaceScope,
        currency: &str,
        after_projects: impl FnOnce(),
    ) -> Result<WorkspaceProjection> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let projection =
            read_workspace_projection_on(&transaction, scope, currency, after_projects)?;
        transaction.commit()?;
        Ok(projection)
    }

    /// Reads a session and all bounded child streams from one database
    /// snapshot. `related_limit` applies independently to file changes, costs,
    /// control requests, and receipts and is clamped to 1..=5,000.
    pub fn read_session_projection(
        &self,
        session_id: &str,
        message_query: StreamQuery,
        event_query: StreamQuery,
        related_limit: u32,
    ) -> Result<Option<SessionProjection>> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let projection = read_session_projection_on(
            &transaction,
            session_id,
            message_query,
            event_query,
            related_limit,
        )?;
        transaction.commit()?;
        Ok(projection)
    }

    pub fn upsert_provider(&self, provider: &Provider) -> Result<()> {
        validate_id("provider", &provider.id)?;
        validate_text("provider", "display_name", &provider.display_name)?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let incompatible = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM integrations WHERE provider_id = ?1 AND kind <> ?2)",
            params![provider.id, provider.kind.db_value()],
            |row| read_bool(row, 0),
        )?;
        if incompatible {
            return Err(invalid(
                "provider",
                "kind change would invalidate a linked integration",
            ));
        }
        transaction.execute(
            "INSERT INTO providers (id, display_name, kind) VALUES (?1, ?2, ?3) \
             ON CONFLICT(id) DO UPDATE SET display_name = excluded.display_name, kind = excluded.kind",
            params![provider.id, provider.display_name, provider.kind.db_value()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_provider(&self, id: &str) -> Result<Option<Provider>> {
        let connection = self.connection()?;
        let mut statement =
            connection.prepare("SELECT id, display_name, kind FROM providers WHERE id = ?1")?;
        let mut rows = statement.query([id])?;
        rows.next()?.map(decode_provider).transpose()
    }

    pub fn list_providers(&self) -> Result<Vec<Provider>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, display_name, kind FROM providers ORDER BY display_name COLLATE NOCASE, id",
        )?;
        collect_rows(statement.query([])?, decode_provider)
    }

    pub fn delete_provider(&self, id: &str) -> Result<bool> {
        let connection = self.connection()?;
        delete_by_id(&connection, "providers", id)
    }

    pub fn upsert_integration(&self, integration: &Integration) -> Result<()> {
        validate_id("integration", &integration.id)?;
        validate_text("integration", "connector_key", &integration.connector_key)?;
        validate_text("integration", "display_name", &integration.display_name)?;
        validate_evidence_source(
            "integration",
            integration.evidence,
            &integration.connector_key,
        )?;
        if integration.auth == AuthState::Confirmed
            && (integration.evidence != EvidenceKind::Observed
                || integration.checked_at_unix_ms.is_none())
        {
            return Err(invalid(
                "integration",
                "confirmed authentication requires observed evidence and a check timestamp",
            ));
        }
        if integration.state == IntegrationState::Ready {
            if integration.auth != AuthState::Confirmed {
                return Err(invalid(
                    "integration",
                    "ready state requires confirmed authentication",
                ));
            }
            if integration.evidence != EvidenceKind::Observed
                || integration.checked_at_unix_ms.is_none()
            {
                return Err(invalid(
                    "integration",
                    "ready state requires observed evidence and a check timestamp",
                ));
            }
        }
        let checked_at = optional_to_i64(integration.checked_at_unix_ms, "checked_at_unix_ms")?;
        let c = integration.capabilities;
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(provider_id) = integration.provider_id.as_deref() {
            let provider_kind = transaction
                .query_row(
                    "SELECT kind FROM providers WHERE id = ?1",
                    [provider_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            let Some(provider_kind) = provider_kind else {
                return Err(invalid("integration", "referenced provider was not found"));
            };
            if ProviderKind::from_db(&provider_kind)? != integration.kind {
                return Err(invalid(
                    "integration",
                    "kind must match the referenced provider kind",
                ));
            }
        }
        ensure_integration_agent_compatibility(&transaction, integration)?;
        transaction.execute(
            "INSERT INTO integrations (id, provider_id, connector_key, display_name, kind, state, auth, \
                 evidence, checked_at_unix_ms, problem, can_observe, can_auth_probe, can_direct, can_pause, \
                 can_resume, can_stop, can_logs, can_costs, can_agent_messages) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19) \
             ON CONFLICT(id) DO UPDATE SET provider_id=excluded.provider_id, connector_key=excluded.connector_key, \
                 display_name=excluded.display_name, kind=excluded.kind, state=excluded.state, auth=excluded.auth, \
                 evidence=excluded.evidence, checked_at_unix_ms=excluded.checked_at_unix_ms, problem=excluded.problem, \
                 can_observe=excluded.can_observe, can_auth_probe=excluded.can_auth_probe, can_direct=excluded.can_direct, \
                 can_pause=excluded.can_pause, can_resume=excluded.can_resume, can_stop=excluded.can_stop, \
                 can_logs=excluded.can_logs, can_costs=excluded.can_costs, can_agent_messages=excluded.can_agent_messages",
            params![
                integration.id, integration.provider_id, integration.connector_key, integration.display_name,
                integration.kind.db_value(), integration.state.db_value(), integration.auth.db_value(),
                integration.evidence.db_value(), checked_at, integration.problem, bool_i64(c.observe),
                bool_i64(c.auth_probe), bool_i64(c.direct), bool_i64(c.pause), bool_i64(c.resume),
                bool_i64(c.stop), bool_i64(c.logs), bool_i64(c.costs), bool_i64(c.agent_messages),
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_integration(&self, id: &str) -> Result<Option<Integration>> {
        let connection = self.connection()?;
        let sql = format!(
            "SELECT id, provider_id, connector_key, display_name, kind, state, auth, evidence, \
             checked_at_unix_ms, problem, {CAPABILITY_COLUMNS} FROM integrations WHERE id = ?1"
        );
        let mut statement = connection.prepare(&sql)?;
        let mut rows = statement.query([id])?;
        rows.next()?.map(decode_integration).transpose()
    }

    pub fn list_integrations(&self) -> Result<Vec<Integration>> {
        let connection = self.connection()?;
        let sql = format!(
            "SELECT id, provider_id, connector_key, display_name, kind, state, auth, evidence, \
             checked_at_unix_ms, problem, {CAPABILITY_COLUMNS} FROM integrations \
             ORDER BY display_name COLLATE NOCASE, id"
        );
        let mut statement = connection.prepare(&sql)?;
        collect_rows(statement.query([])?, decode_integration)
    }

    pub fn delete_integration(&self, id: &str) -> Result<bool> {
        let connection = self.connection()?;
        delete_by_id(&connection, "integrations", id)
    }

    pub fn upsert_project(&self, project: &Project) -> Result<()> {
        validate_id("project", &project.id)?;
        validate_text("project", "name", &project.name)?;
        let created_at = to_i64(project.created_at_unix_ms, "created_at_unix_ms")?;
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO projects (id, name, root_path, state, created_at_unix_ms) VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, root_path=excluded.root_path, state=excluded.state",
            params![project.id, project.name, project.root_path, project.state.db_value(), created_at],
        )?;
        Ok(())
    }

    pub fn get_project(&self, id: &str) -> Result<Option<Project>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, name, root_path, state, created_at_unix_ms FROM projects WHERE id = ?1",
        )?;
        let mut rows = statement.query([id])?;
        rows.next()?.map(decode_project).transpose()
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, name, root_path, state, created_at_unix_ms FROM projects \
             ORDER BY CASE state WHEN 'active' THEN 0 WHEN 'paused' THEN 1 ELSE 2 END, name COLLATE NOCASE, id",
        )?;
        collect_rows(statement.query([])?, decode_project)
    }

    pub fn delete_project(&self, id: &str) -> Result<bool> {
        let connection = self.connection()?;
        delete_by_id(&connection, "projects", id)
    }

    pub fn upsert_agent(&self, agent: &Agent) -> Result<()> {
        validate_id("agent", &agent.id)?;
        validate_text("agent", "display_name", &agent.display_name)?;
        let c = agent.capabilities;
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let integration_sql = format!(
            "SELECT provider_id, kind, {CAPABILITY_COLUMNS} FROM integrations WHERE id = ?1"
        );
        let integration = transaction
            .query_row(&integration_sql, [&agent.connector_id], |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, String>(1)?,
                    read_capabilities(row, 2)?,
                ))
            })
            .optional()?;
        let Some((integration_provider_id, integration_kind, integration_capabilities)) =
            integration
        else {
            return Err(invalid("agent", "referenced integration was not found"));
        };
        if integration_provider_id.as_deref() != Some(&agent.provider_id) {
            return Err(invalid(
                "agent",
                "provider must match the referenced integration provider",
            ));
        }
        if !capabilities_are_subset(c, integration_capabilities) {
            return Err(invalid(
                "agent",
                "capabilities must be a subset of the referenced integration capabilities",
            ));
        }
        let provider_kind = transaction
            .query_row(
                "SELECT kind FROM providers WHERE id = ?1",
                [&agent.provider_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(provider_kind) = provider_kind else {
            return Err(invalid("agent", "referenced provider was not found"));
        };
        if ProviderKind::from_db(&provider_kind)? != ProviderKind::from_db(&integration_kind)? {
            return Err(invalid(
                "agent",
                "integration kind must match the referenced provider kind",
            ));
        }
        let attention_conflict = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM attention_findings WHERE agent_id = ?1 \
             AND integration_id IS NOT NULL AND integration_id <> ?2)",
            params![agent.id, agent.connector_id],
            |row| read_bool(row, 0),
        )?;
        if attention_conflict {
            return Err(invalid(
                "agent",
                "connector change would invalidate an attention finding",
            ));
        }
        transaction.execute(
            "INSERT INTO agents (id, provider_id, connector_id, display_name, model, can_observe, can_auth_probe, \
                 can_direct, can_pause, can_resume, can_stop, can_logs, can_costs, can_agent_messages) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14) \
             ON CONFLICT(id) DO UPDATE SET provider_id=excluded.provider_id, connector_id=excluded.connector_id, \
                 display_name=excluded.display_name, model=excluded.model, can_observe=excluded.can_observe, \
                 can_auth_probe=excluded.can_auth_probe, can_direct=excluded.can_direct, can_pause=excluded.can_pause, \
                 can_resume=excluded.can_resume, can_stop=excluded.can_stop, can_logs=excluded.can_logs, \
                 can_costs=excluded.can_costs, can_agent_messages=excluded.can_agent_messages",
            params![
                agent.id, agent.provider_id, agent.connector_id, agent.display_name, agent.model,
                bool_i64(c.observe), bool_i64(c.auth_probe), bool_i64(c.direct), bool_i64(c.pause),
                bool_i64(c.resume), bool_i64(c.stop), bool_i64(c.logs), bool_i64(c.costs),
                bool_i64(c.agent_messages),
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Activates one integration and its provider-bound agent as one truth
    /// boundary. If either write or any relation validation fails, neither
    /// Ready capabilities nor the agent update become visible.
    pub fn activate_integration_agent(
        &self,
        integration: &Integration,
        agent: &Agent,
    ) -> Result<()> {
        validate_id("integration", &integration.id)?;
        validate_text("integration", "connector_key", &integration.connector_key)?;
        validate_text("integration", "display_name", &integration.display_name)?;
        validate_evidence_source(
            "integration",
            integration.evidence,
            &integration.connector_key,
        )?;
        if integration.state != IntegrationState::Ready
            || integration.auth != AuthState::Confirmed
            || integration.evidence != EvidenceKind::Observed
            || integration.checked_at_unix_ms.is_none()
        {
            return Err(invalid(
                "integration activation",
                "ready activation requires confirmed authentication, observed evidence, and a check timestamp",
            ));
        }
        validate_id("agent", &agent.id)?;
        validate_text("agent", "display_name", &agent.display_name)?;
        if agent.connector_id != integration.id {
            return Err(invalid(
                "integration activation",
                "agent connector must match the activated integration",
            ));
        }
        if integration.provider_id.as_deref() != Some(agent.provider_id.as_str()) {
            return Err(invalid(
                "integration activation",
                "agent provider must match the activated integration provider",
            ));
        }
        if !capabilities_are_subset(agent.capabilities, integration.capabilities) {
            return Err(invalid(
                "integration activation",
                "agent capabilities must be a subset of the activated integration capabilities",
            ));
        }

        let checked_at = optional_to_i64(integration.checked_at_unix_ms, "checked_at_unix_ms")?;
        let integration_capabilities = integration.capabilities;
        let agent_capabilities = agent.capabilities;
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let provider_kind = transaction
            .query_row(
                "SELECT kind FROM providers WHERE id = ?1",
                [&agent.provider_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(provider_kind) = provider_kind else {
            return Err(invalid(
                "integration activation",
                "referenced provider was not found",
            ));
        };
        if ProviderKind::from_db(&provider_kind)? != integration.kind {
            return Err(invalid(
                "integration activation",
                "integration kind must match the referenced provider kind",
            ));
        }
        ensure_integration_agent_compatibility(&transaction, integration)?;
        transaction.execute(
            "INSERT INTO integrations (id, provider_id, connector_key, display_name, kind, state, auth, \
                 evidence, checked_at_unix_ms, problem, can_observe, can_auth_probe, can_direct, can_pause, \
                 can_resume, can_stop, can_logs, can_costs, can_agent_messages) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19) \
             ON CONFLICT(id) DO UPDATE SET provider_id=excluded.provider_id, connector_key=excluded.connector_key, \
                 display_name=excluded.display_name, kind=excluded.kind, state=excluded.state, auth=excluded.auth, \
                 evidence=excluded.evidence, checked_at_unix_ms=excluded.checked_at_unix_ms, problem=excluded.problem, \
                 can_observe=excluded.can_observe, can_auth_probe=excluded.can_auth_probe, can_direct=excluded.can_direct, \
                 can_pause=excluded.can_pause, can_resume=excluded.can_resume, can_stop=excluded.can_stop, \
                 can_logs=excluded.can_logs, can_costs=excluded.can_costs, can_agent_messages=excluded.can_agent_messages",
            params![
                integration.id,
                integration.provider_id,
                integration.connector_key,
                integration.display_name,
                integration.kind.db_value(),
                integration.state.db_value(),
                integration.auth.db_value(),
                integration.evidence.db_value(),
                checked_at,
                integration.problem,
                bool_i64(integration_capabilities.observe),
                bool_i64(integration_capabilities.auth_probe),
                bool_i64(integration_capabilities.direct),
                bool_i64(integration_capabilities.pause),
                bool_i64(integration_capabilities.resume),
                bool_i64(integration_capabilities.stop),
                bool_i64(integration_capabilities.logs),
                bool_i64(integration_capabilities.costs),
                bool_i64(integration_capabilities.agent_messages),
            ],
        )?;

        let attention_conflict = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM attention_findings WHERE agent_id = ?1 \
             AND integration_id IS NOT NULL AND integration_id <> ?2)",
            params![agent.id, agent.connector_id],
            |row| read_bool(row, 0),
        )?;
        if attention_conflict {
            return Err(invalid(
                "integration activation",
                "agent connector change would invalidate an attention finding",
            ));
        }
        transaction.execute(
            "INSERT INTO agents (id, provider_id, connector_id, display_name, model, can_observe, can_auth_probe, \
                 can_direct, can_pause, can_resume, can_stop, can_logs, can_costs, can_agent_messages) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14) \
             ON CONFLICT(id) DO UPDATE SET provider_id=excluded.provider_id, connector_id=excluded.connector_id, \
                 display_name=excluded.display_name, model=excluded.model, can_observe=excluded.can_observe, \
                 can_auth_probe=excluded.can_auth_probe, can_direct=excluded.can_direct, can_pause=excluded.can_pause, \
                 can_resume=excluded.can_resume, can_stop=excluded.can_stop, can_logs=excluded.can_logs, \
                 can_costs=excluded.can_costs, can_agent_messages=excluded.can_agent_messages",
            params![
                agent.id,
                agent.provider_id,
                agent.connector_id,
                agent.display_name,
                agent.model,
                bool_i64(agent_capabilities.observe),
                bool_i64(agent_capabilities.auth_probe),
                bool_i64(agent_capabilities.direct),
                bool_i64(agent_capabilities.pause),
                bool_i64(agent_capabilities.resume),
                bool_i64(agent_capabilities.stop),
                bool_i64(agent_capabilities.logs),
                bool_i64(agent_capabilities.costs),
                bool_i64(agent_capabilities.agent_messages),
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_agent(&self, id: &str) -> Result<Option<Agent>> {
        let connection = self.connection()?;
        let sql = format!(
            "SELECT id, provider_id, connector_id, display_name, model, {CAPABILITY_COLUMNS} \
             FROM agents WHERE id = ?1"
        );
        let mut statement = connection.prepare(&sql)?;
        let mut rows = statement.query([id])?;
        rows.next()?.map(decode_agent).transpose()
    }

    pub fn list_agents(&self) -> Result<Vec<Agent>> {
        let connection = self.connection()?;
        let sql = format!(
            "SELECT id, provider_id, connector_id, display_name, model, {CAPABILITY_COLUMNS} \
             FROM agents ORDER BY display_name COLLATE NOCASE, id"
        );
        let mut statement = connection.prepare(&sql)?;
        collect_rows(statement.query([])?, decode_agent)
    }

    pub fn delete_agent(&self, id: &str) -> Result<bool> {
        let connection = self.connection()?;
        delete_by_id(&connection, "agents", id)
    }

    /// Upserts the task and replaces its complete assignee set atomically.
    pub fn upsert_task(&self, task: &Task) -> Result<()> {
        validate_id("task", &task.id)?;
        validate_text("task", "title", &task.title)?;
        if task.updated_at_unix_ms < task.created_at_unix_ms {
            return Err(invalid("task", "updated_at precedes created_at"));
        }
        let created_at = to_i64(task.created_at_unix_ms, "created_at_unix_ms")?;
        let updated_at = to_i64(task.updated_at_unix_ms, "updated_at_unix_ms")?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_task_dependents_match_project(&transaction, &task.id, &task.project_id)?;
        transaction.execute(
            "INSERT INTO tasks (id, project_id, title, detail, state, created_at_unix_ms, updated_at_unix_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT(id) DO UPDATE SET \
             project_id=excluded.project_id, title=excluded.title, detail=excluded.detail, \
             state=excluded.state, updated_at_unix_ms=excluded.updated_at_unix_ms",
            params![
                task.id,
                task.project_id,
                task.title,
                task.detail,
                task.state.db_value(),
                created_at,
                updated_at
            ],
        )?;
        replace_task_assignees(&transaction, &task.id, &task.assignee_agent_ids)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn assign_task_agents(
        &self,
        task_id: &str,
        agent_ids: &[String],
        updated_at_unix_ms: u64,
    ) -> Result<Task> {
        let updated_at = to_i64(updated_at_unix_ms, "updated_at_unix_ms")?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "UPDATE tasks SET updated_at_unix_ms = ?2 WHERE id = ?1 AND created_at_unix_ms <= ?2",
            params![task_id, updated_at],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound {
                entity: "task",
                id: task_id.to_owned(),
            });
        }
        replace_task_assignees(&transaction, task_id, agent_ids)?;
        transaction.commit()?;
        drop(connection);
        self.get_task(task_id)?.ok_or_else(|| StoreError::NotFound {
            entity: "task",
            id: task_id.to_owned(),
        })
    }

    pub fn get_task(&self, id: &str) -> Result<Option<Task>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, project_id, title, detail, state, created_at_unix_ms, updated_at_unix_ms \
             FROM tasks WHERE id = ?1",
        )?;
        let mut rows = statement.query([id])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let mut task = decode_task_without_assignees(row)?;
        drop(rows);
        drop(statement);
        task.assignee_agent_ids = query_task_assignees(&connection, id)?;
        Ok(Some(task))
    }

    pub fn list_tasks(&self, project_id: Option<&str>) -> Result<Vec<Task>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, project_id, title, detail, state, created_at_unix_ms, updated_at_unix_ms \
             FROM tasks WHERE (?1 IS NULL OR project_id = ?1) \
             ORDER BY CASE state WHEN 'running' THEN 0 WHEN 'blocked' THEN 1 WHEN 'waiting' THEN 2 \
                 WHEN 'queued' THEN 3 WHEN 'draft' THEN 4 ELSE 5 END, updated_at_unix_ms DESC, id",
        )?;
        let mut rows = statement.query([project_id])?;
        let mut tasks = Vec::new();
        while let Some(row) = rows.next()? {
            tasks.push(decode_task_without_assignees(row)?);
        }
        drop(rows);
        drop(statement);
        for task in &mut tasks {
            task.assignee_agent_ids = query_task_assignees(&connection, &task.id)?;
        }
        Ok(tasks)
    }

    pub fn delete_task(&self, id: &str) -> Result<bool> {
        let connection = self.connection()?;
        delete_by_id(&connection, "tasks", id)
    }

    pub fn upsert_session(&self, session: &Session) -> Result<()> {
        validate_id("session", &session.id)?;
        let started_at = to_i64(session.started_at_unix_ms, "started_at_unix_ms")?;
        let last_observed =
            optional_to_i64(session.last_observed_at_unix_ms, "last_observed_at_unix_ms")?;
        if session
            .last_observed_at_unix_ms
            .is_some_and(|observed| observed < session.started_at_unix_ms)
        {
            return Err(invalid("session", "last_observed_at precedes started_at"));
        }
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_session_scope(&transaction, session)?;
        ensure_session_dependents_match_scope(&transaction, session)?;
        transaction.execute(
            "INSERT INTO sessions (id, project_id, task_id, agent_id, provider_session_id, state, \
                 started_at_unix_ms, last_observed_at_unix_ms, title_hint) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
             ON CONFLICT(id) DO UPDATE SET project_id=excluded.project_id, task_id=excluded.task_id, \
                 agent_id=excluded.agent_id, provider_session_id=excluded.provider_session_id, \
                 state=excluded.state, last_observed_at_unix_ms=excluded.last_observed_at_unix_ms, \
                 title_hint=COALESCE(excluded.title_hint, title_hint)",
            params![
                session.id,
                session.project_id,
                session.task_id,
                session.agent_id,
                session.provider_session_id,
                session.state.db_value(),
                started_at,
                last_observed,
                session.title_hint
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_session(&self, id: &str) -> Result<Option<Session>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, project_id, task_id, agent_id, provider_session_id, state, \
             started_at_unix_ms, last_observed_at_unix_ms, title_hint FROM sessions WHERE id = ?1",
        )?;
        let mut rows = statement.query([id])?;
        rows.next()?.map(decode_session).transpose()
    }

    pub fn list_sessions(&self, project_id: Option<&str>) -> Result<Vec<Session>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, project_id, task_id, agent_id, provider_session_id, state, \
             started_at_unix_ms, last_observed_at_unix_ms, title_hint FROM sessions \
             WHERE (?1 IS NULL OR project_id = ?1) ORDER BY started_at_unix_ms DESC, id",
        )?;
        collect_rows(statement.query([project_id])?, decode_session)
    }

    pub fn delete_session(&self, id: &str) -> Result<bool> {
        let connection = self.connection()?;
        delete_by_id(&connection, "sessions", id)
    }

    /// Appends one chat message with the next local sequence for its session.
    pub fn append_message(&self, new: NewMessage) -> Result<Message> {
        validate_id("message", &new.id)?;
        validate_text("message", "body", &new.body)?;
        validate_evidence_source("message", new.evidence, &new.source)?;
        validate_message_author(new.role, new.author_agent_id.as_deref())?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_message_author_matches_session(
            &transaction,
            &new.session_id,
            new.role,
            new.author_agent_id.as_deref(),
        )?;
        let sequence = next_sequence(&transaction, "messages", &new.session_id)?;
        let message = Message {
            id: new.id,
            session_id: new.session_id,
            sequence,
            role: new.role,
            author_agent_id: new.author_agent_id,
            body: new.body,
            sent_at_unix_ms: new.sent_at_unix_ms,
            ingested_at_unix_ms: new.ingested_at_unix_ms,
            evidence: new.evidence,
            source: new.source,
            correlation_id: new.correlation_id,
        };
        insert_message_on(&transaction, &message)?;
        transaction.commit()?;
        Ok(message)
    }

    /// Atomically records an owner's chat direction, the provider control
    /// request, and its initial receipt. This prevents UI-visible intent from
    /// being committed without the corresponding delivery truth record.
    pub fn record_owner_direction(
        &self,
        new: NewMessage,
        request: ControlRequest,
        receipt: ControlReceipt,
    ) -> Result<RecordedDirection> {
        validate_id("message", &new.id)?;
        validate_text("message", "body", &new.body)?;
        validate_evidence_source("message", new.evidence, &new.source)?;
        validate_message_author(new.role, new.author_agent_id.as_deref())?;
        validate_control_request(&request)?;
        validate_control_receipt(&receipt)?;
        if new.role != MessageRole::Owner || new.evidence != EvidenceKind::Observed {
            return Err(invalid(
                "owner direction",
                "message must be an observed owner message",
            ));
        }
        if request.action != ControlAction::Direct || !request.requested_by_owner {
            return Err(invalid(
                "owner direction",
                "control request must be an owner direct action",
            ));
        }
        if new.session_id != request.session_id {
            return Err(invalid(
                "owner direction",
                "message and control request must target the same session",
            ));
        }
        if request.instruction.as_deref() != Some(new.body.as_str()) {
            return Err(invalid(
                "owner direction",
                "control instruction must match the owner message",
            ));
        }
        if receipt.request_id != request.id {
            return Err(invalid(
                "owner direction",
                "receipt must reference the control request",
            ));
        }
        if receipt.received_at_unix_ms < request.requested_at_unix_ms {
            return Err(invalid(
                "owner direction",
                "receipt cannot precede the control request",
            ));
        }

        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let sequence = next_sequence(&transaction, "messages", &new.session_id)?;
        let message = Message {
            id: new.id,
            session_id: new.session_id,
            sequence,
            role: new.role,
            author_agent_id: new.author_agent_id,
            body: new.body,
            sent_at_unix_ms: new.sent_at_unix_ms,
            ingested_at_unix_ms: new.ingested_at_unix_ms,
            evidence: new.evidence,
            source: new.source,
            correlation_id: new.correlation_id,
        };
        insert_message_on(&transaction, &message)?;
        upsert_control_request_on(&transaction, &request)?;
        upsert_control_receipt_on(&transaction, &receipt)?;
        transaction.commit()?;
        Ok(RecordedDirection {
            message,
            request,
            receipt,
        })
    }

    /// Imports a message that already has a normalized local sequence.
    pub fn insert_message(&self, message: &Message) -> Result<()> {
        validate_message(message)?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_message_author_matches_session(
            &transaction,
            &message.session_id,
            message.role,
            message.author_agent_id.as_deref(),
        )?;
        insert_message_on(&transaction, message)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_message(&self, id: &str) -> Result<Option<Message>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, session_id, sequence, role, author_agent_id, body, sent_at_unix_ms, \
             ingested_at_unix_ms, evidence, source, correlation_id FROM messages WHERE id = ?1",
        )?;
        let mut rows = statement.query([id])?;
        rows.next()?.map(decode_message).transpose()
    }

    pub fn list_messages(&self, session_id: &str, query: StreamQuery) -> Result<Vec<Message>> {
        let connection = self.connection()?;
        let after = optional_to_i64(query.after_sequence, "after_sequence")?.unwrap_or(0);
        let limit = i64::from(query.limit.clamp(1, 1_000));
        let mut statement = connection.prepare(
            "SELECT id, session_id, sequence, role, author_agent_id, body, sent_at_unix_ms, \
             ingested_at_unix_ms, evidence, source, correlation_id FROM messages \
             WHERE session_id = ?1 AND sequence > ?2 ORDER BY sequence ASC LIMIT ?3",
        )?;
        collect_rows(
            statement.query(params![session_id, after, limit])?,
            decode_message,
        )
    }

    pub fn delete_message(&self, id: &str) -> Result<bool> {
        let connection = self.connection()?;
        delete_by_id(&connection, "messages", id)
    }

    /// Appends a normalized event with the next local sequence. Provider event
    /// IDs are idempotent within a session and source so connector replay is
    /// safe without collapsing distinct repeated text.
    pub fn append_event(&self, new: NewSessionEvent) -> Result<SessionEvent> {
        validate_id("event", &new.id)?;
        validate_text("event", "summary", &new.summary)?;
        validate_evidence_source("event", new.evidence, &new.source)?;
        let mut connection = self.connection()?;

        if let Some(provider_event_id) = new.provider_event_id.as_deref() {
            let existing = {
                let mut statement = connection.prepare(
                    "SELECT id, session_id, sequence, occurred_at_unix_ms, kind, summary, detail, evidence, \
                     source, ingested_at_unix_ms, provider_event_id, correlation_id FROM session_events \
                     WHERE session_id = ?1 AND source = ?2 AND provider_event_id = ?3",
                )?;
                let mut rows =
                    statement.query(params![new.session_id, new.source, provider_event_id])?;
                rows.next()?.map(decode_event).transpose()?
            };
            if let Some(existing) = existing {
                return Ok(existing);
            }
        }

        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let sequence = next_sequence(&transaction, "session_events", &new.session_id)?;
        let event = SessionEvent {
            id: new.id,
            session_id: new.session_id,
            sequence,
            occurred_at_unix_ms: new.occurred_at_unix_ms,
            kind: new.kind,
            summary: new.summary,
            detail: new.detail,
            evidence: new.evidence,
            source: new.source,
            ingested_at_unix_ms: new.ingested_at_unix_ms,
            provider_event_id: new.provider_event_id,
            correlation_id: new.correlation_id,
        };
        insert_event_on(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn insert_event(&self, event: &SessionEvent) -> Result<()> {
        validate_event(event)?;
        let connection = self.connection()?;
        insert_event_on(&connection, event)
    }

    pub fn get_event(&self, id: &str) -> Result<Option<SessionEvent>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, session_id, sequence, occurred_at_unix_ms, kind, summary, detail, evidence, \
             source, ingested_at_unix_ms, provider_event_id, correlation_id FROM session_events WHERE id = ?1",
        )?;
        let mut rows = statement.query([id])?;
        rows.next()?.map(decode_event).transpose()
    }

    pub fn list_events(&self, session_id: &str, query: StreamQuery) -> Result<Vec<SessionEvent>> {
        let connection = self.connection()?;
        let after = optional_to_i64(query.after_sequence, "after_sequence")?.unwrap_or(0);
        let limit = i64::from(query.limit.clamp(1, 5_000));
        let mut statement = connection.prepare(
            "SELECT id, session_id, sequence, occurred_at_unix_ms, kind, summary, detail, evidence, \
             source, ingested_at_unix_ms, provider_event_id, correlation_id FROM session_events \
             WHERE session_id = ?1 AND sequence > ?2 ORDER BY sequence ASC LIMIT ?3",
        )?;
        collect_rows(
            statement.query(params![session_id, after, limit])?,
            decode_event,
        )
    }

    pub fn list_logs(&self, session_id: &str, limit: u32) -> Result<Vec<SessionEvent>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, session_id, sequence, occurred_at_unix_ms, kind, summary, detail, evidence, \
             source, ingested_at_unix_ms, provider_event_id, correlation_id FROM session_events \
             WHERE session_id = ?1 AND kind = 'log' ORDER BY sequence DESC LIMIT ?2",
        )?;
        let mut logs = collect_rows(
            statement.query(params![session_id, i64::from(limit.clamp(1, 5_000))])?,
            decode_event,
        )?;
        logs.reverse();
        Ok(logs)
    }

    pub fn delete_event(&self, id: &str) -> Result<bool> {
        let connection = self.connection()?;
        delete_by_id(&connection, "session_events", id)
    }

    pub fn upsert_file_change(&self, change: &FileChange) -> Result<()> {
        validate_id("file change", &change.id)?;
        validate_text("file change", "path", &change.path)?;
        validate_evidence_source("file change", change.evidence, &change.source)?;
        if change.kind == FileChangeKind::Renamed && change.previous_path.is_none() {
            return Err(invalid(
                "file change",
                "renamed files require previous_path",
            ));
        }
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(event_id) = change.event_id.as_deref() {
            let event_session_id = transaction
                .query_row(
                    "SELECT session_id FROM session_events WHERE id = ?1",
                    [event_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if event_session_id.as_deref() != Some(&change.session_id) {
                return Err(invalid(
                    "file change",
                    "event must belong to the file change session",
                ));
            }
        }
        transaction.execute(
            "INSERT INTO file_changes (id, session_id, event_id, path, previous_path, kind, additions, deletions, \
             occurred_at_unix_ms, evidence, source) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) \
             ON CONFLICT(id) DO UPDATE SET session_id=excluded.session_id, event_id=excluded.event_id, \
             path=excluded.path, previous_path=excluded.previous_path, kind=excluded.kind, additions=excluded.additions, \
             deletions=excluded.deletions, occurred_at_unix_ms=excluded.occurred_at_unix_ms, \
             evidence=excluded.evidence, source=excluded.source",
            params![
                change.id,
                change.session_id,
                change.event_id,
                change.path,
                change.previous_path,
                change.kind.db_value(),
                optional_to_i64(change.additions, "additions")?,
                optional_to_i64(change.deletions, "deletions")?,
                to_i64(change.occurred_at_unix_ms, "occurred_at_unix_ms")?,
                change.evidence.db_value(),
                change.source,
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_file_change(&self, id: &str) -> Result<Option<FileChange>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, session_id, event_id, path, previous_path, kind, additions, deletions, \
             occurred_at_unix_ms, evidence, source FROM file_changes WHERE id = ?1",
        )?;
        let mut rows = statement.query([id])?;
        rows.next()?.map(decode_file_change).transpose()
    }

    pub fn list_file_changes(
        &self,
        session_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<FileChange>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, session_id, event_id, path, previous_path, kind, additions, deletions, \
             occurred_at_unix_ms, evidence, source FROM file_changes \
             WHERE (?1 IS NULL OR session_id = ?1) ORDER BY occurred_at_unix_ms DESC, id LIMIT ?2",
        )?;
        collect_rows(
            statement.query(params![session_id, i64::from(limit.clamp(1, 5_000))])?,
            decode_file_change,
        )
    }

    pub fn delete_file_change(&self, id: &str) -> Result<bool> {
        let connection = self.connection()?;
        delete_by_id(&connection, "file_changes", id)
    }

    pub fn upsert_cost(&self, cost: &CostRecord) -> Result<()> {
        validate_cost(cost)?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_cost_scope(&transaction, cost)?;
        transaction.execute(
            "INSERT INTO cost_records (id, project_id, task_id, session_id, agent_id, currency, amount_micros, \
             confidence, occurred_at_unix_ms, ingested_at_unix_ms, evidence, source, note) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13) \
             ON CONFLICT(id) DO UPDATE SET project_id=excluded.project_id, task_id=excluded.task_id, \
             session_id=excluded.session_id, agent_id=excluded.agent_id, currency=excluded.currency, \
             amount_micros=excluded.amount_micros, confidence=excluded.confidence, \
             occurred_at_unix_ms=excluded.occurred_at_unix_ms, ingested_at_unix_ms=excluded.ingested_at_unix_ms, \
             evidence=excluded.evidence, source=excluded.source, note=excluded.note",
            params![
                cost.id,
                cost.project_id,
                cost.task_id,
                cost.session_id,
                cost.agent_id,
                cost.amount.currency,
                optional_to_i64(cost.amount.micros, "amount_micros")?,
                cost.amount.confidence.db_value(),
                to_i64(cost.occurred_at_unix_ms, "occurred_at_unix_ms")?,
                to_i64(cost.ingested_at_unix_ms, "ingested_at_unix_ms")?,
                cost.evidence.db_value(),
                cost.source,
                cost.note,
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_cost(&self, id: &str) -> Result<Option<CostRecord>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, project_id, task_id, session_id, agent_id, currency, amount_micros, confidence, \
             occurred_at_unix_ms, ingested_at_unix_ms, evidence, source, note FROM cost_records WHERE id = ?1",
        )?;
        let mut rows = statement.query([id])?;
        rows.next()?.map(decode_cost).transpose()
    }

    pub fn list_costs(&self, query: &CostQuery) -> Result<Vec<CostRecord>> {
        let connection = self.connection()?;
        let currency = query
            .currency
            .as_ref()
            .map(|value| value.to_ascii_uppercase());
        let limit = i64::from(query.limit.unwrap_or(1_000).clamp(1, 50_000));
        let mut statement = connection.prepare(
            "SELECT id, project_id, task_id, session_id, agent_id, currency, amount_micros, confidence, \
             occurred_at_unix_ms, ingested_at_unix_ms, evidence, source, note FROM cost_records \
             WHERE (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR task_id = ?2) \
               AND (?3 IS NULL OR session_id = ?3) AND (?4 IS NULL OR agent_id = ?4) \
               AND (?5 IS NULL OR currency = ?5) ORDER BY occurred_at_unix_ms DESC, id LIMIT ?6",
        )?;
        collect_rows(
            statement.query(params![
                query.project_id,
                query.task_id,
                query.session_id,
                query.agent_id,
                currency,
                limit
            ])?,
            decode_cost,
        )
    }

    pub fn cost_summary(&self, project_id: &str, currency: &str) -> Result<CostSummary> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let summary = cost_summary_on(&transaction, project_id, currency)?;
        transaction.commit()?;
        Ok(summary)
    }

    pub fn delete_cost(&self, id: &str) -> Result<bool> {
        let connection = self.connection()?;
        delete_by_id(&connection, "cost_records", id)
    }

    pub fn upsert_attention(&self, finding: &AttentionRecord) -> Result<()> {
        validate_attention(finding)?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let finding = normalized_attention_scope(&transaction, finding)?;
        transaction.execute(
            "INSERT INTO attention_findings (id, project_id, task_id, session_id, agent_id, integration_id, \
             severity, state, title, detail, recovery, detected_at_unix_ms, updated_at_unix_ms, evidence, source) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15) \
             ON CONFLICT(id) DO UPDATE SET project_id=excluded.project_id, task_id=excluded.task_id, \
             session_id=excluded.session_id, agent_id=excluded.agent_id, integration_id=excluded.integration_id, \
             severity=excluded.severity, state=excluded.state, title=excluded.title, detail=excluded.detail, \
             recovery=excluded.recovery, updated_at_unix_ms=excluded.updated_at_unix_ms, \
             evidence=excluded.evidence, source=excluded.source",
            params![
                finding.id,
                finding.project_id,
                finding.task_id,
                finding.session_id,
                finding.agent_id,
                finding.integration_id,
                finding.severity.db_value(),
                finding.state.db_value(),
                finding.title,
                finding.detail,
                finding.recovery,
                to_i64(finding.detected_at_unix_ms, "detected_at_unix_ms")?,
                to_i64(finding.updated_at_unix_ms, "updated_at_unix_ms")?,
                finding.evidence.db_value(),
                finding.source,
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_attention(&self, id: &str) -> Result<Option<AttentionRecord>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, project_id, task_id, session_id, agent_id, integration_id, severity, state, title, \
             detail, recovery, detected_at_unix_ms, updated_at_unix_ms, evidence, source \
             FROM attention_findings WHERE id = ?1",
        )?;
        let mut rows = statement.query([id])?;
        rows.next()?.map(decode_attention).transpose()
    }

    pub fn list_attention(&self, query: &AttentionQuery) -> Result<Vec<AttentionRecord>> {
        let connection = self.connection()?;
        let state = query.state.map(DbEnum::db_value);
        let limit = i64::from(query.limit.unwrap_or(1_000).clamp(1, 10_000));
        let mut statement = connection.prepare(
            "SELECT id, project_id, task_id, session_id, agent_id, integration_id, severity, state, title, \
             detail, recovery, detected_at_unix_ms, updated_at_unix_ms, evidence, source \
             FROM attention_findings WHERE (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR state = ?2) \
             ORDER BY CASE severity WHEN 'blocked' THEN 0 WHEN 'needs_attention' THEN 1 \
                 WHEN 'unknown' THEN 2 ELSE 3 END, updated_at_unix_ms DESC, id LIMIT ?3",
        )?;
        let mut findings = collect_rows(
            statement.query(params![query.project_id, state, limit])?,
            decode_attention,
        )?;
        if let Some(minimum) = query.minimum_severity {
            let minimum = severity_rank(minimum);
            findings.retain(|finding| severity_rank(finding.severity) >= minimum);
        }
        Ok(findings)
    }

    pub fn set_attention_state(
        &self,
        id: &str,
        state: AttentionState,
        updated_at_unix_ms: u64,
    ) -> Result<AttentionRecord> {
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE attention_findings SET state = ?2, updated_at_unix_ms = ?3 \
             WHERE id = ?1 AND detected_at_unix_ms <= ?3",
            params![
                id,
                state.db_value(),
                to_i64(updated_at_unix_ms, "updated_at_unix_ms")?
            ],
        )?;
        drop(connection);
        if changed == 0 {
            return Err(StoreError::NotFound {
                entity: "attention finding",
                id: id.to_owned(),
            });
        }
        self.get_attention(id)?.ok_or_else(|| StoreError::NotFound {
            entity: "attention finding",
            id: id.to_owned(),
        })
    }

    pub fn delete_attention(&self, id: &str) -> Result<bool> {
        let connection = self.connection()?;
        delete_by_id(&connection, "attention_findings", id)
    }

    pub fn upsert_handoff(&self, handoff: &AgentHandoff) -> Result<()> {
        validate_handoff(handoff)?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let task_project: Option<String> = transaction
            .query_row(
                "SELECT project_id FROM tasks WHERE id = ?1",
                [&handoff.task_id],
                |row| row.get(0),
            )
            .optional()?;
        if task_project.as_deref() != Some(&handoff.project_id) {
            return Err(invalid(
                "handoff",
                "task must belong to the handoff project",
            ));
        }
        if let Some(resulting_session_id) = handoff.resulting_session_id.as_deref() {
            let resulting_scope = session_scope(&transaction, resulting_session_id)?;
            let Some((project_id, task_id, agent_id)) = resulting_scope else {
                return Err(invalid("handoff", "resulting session was not found"));
            };
            if project_id != handoff.project_id
                || task_id.as_deref() != Some(&handoff.task_id)
                || agent_id != handoff.to_agent_id
            {
                return Err(invalid(
                    "handoff",
                    "resulting session must match the handoff project, task, and destination agent",
                ));
            }
        }
        transaction.execute(
            "INSERT INTO handoffs (id, project_id, task_id, from_agent_id, to_agent_id, instruction, \
             created_at_unix_ms, approved_by_owner, state, delivered_at_unix_ms, delivery_evidence, source, \
             resulting_session_id, correlation_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14) \
             ON CONFLICT(id) DO UPDATE SET project_id=excluded.project_id, task_id=excluded.task_id, \
             from_agent_id=excluded.from_agent_id, to_agent_id=excluded.to_agent_id, instruction=excluded.instruction, \
             approved_by_owner=excluded.approved_by_owner, state=excluded.state, \
             delivered_at_unix_ms=excluded.delivered_at_unix_ms, delivery_evidence=excluded.delivery_evidence, \
             source=excluded.source, resulting_session_id=excluded.resulting_session_id, correlation_id=excluded.correlation_id",
            params![
                handoff.id,
                handoff.project_id,
                handoff.task_id,
                handoff.from_agent_id,
                handoff.to_agent_id,
                handoff.instruction,
                to_i64(handoff.created_at_unix_ms, "created_at_unix_ms")?,
                bool_i64(handoff.approved_by_owner),
                handoff.state.db_value(),
                optional_to_i64(handoff.delivered_at_unix_ms, "delivered_at_unix_ms")?,
                handoff.delivery_evidence.db_value(),
                handoff.source,
                handoff.resulting_session_id,
                handoff.correlation_id,
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_handoff(&self, id: &str) -> Result<Option<AgentHandoff>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, project_id, task_id, from_agent_id, to_agent_id, instruction, created_at_unix_ms, \
             approved_by_owner, state, delivered_at_unix_ms, delivery_evidence, source, resulting_session_id, correlation_id \
             FROM handoffs WHERE id = ?1",
        )?;
        let mut rows = statement.query([id])?;
        rows.next()?.map(decode_handoff).transpose()
    }

    pub fn list_handoffs(&self, project_id: Option<&str>) -> Result<Vec<AgentHandoff>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, project_id, task_id, from_agent_id, to_agent_id, instruction, created_at_unix_ms, \
             approved_by_owner, state, delivered_at_unix_ms, delivery_evidence, source, resulting_session_id, correlation_id \
             FROM handoffs WHERE (?1 IS NULL OR project_id = ?1) ORDER BY created_at_unix_ms DESC, id",
        )?;
        collect_rows(statement.query([project_id])?, decode_handoff)
    }

    pub fn delete_handoff(&self, id: &str) -> Result<bool> {
        let connection = self.connection()?;
        delete_by_id(&connection, "handoffs", id)
    }

    pub fn upsert_control_request(&self, request: &ControlRequest) -> Result<()> {
        validate_control_request(request)?;
        let connection = self.connection()?;
        upsert_control_request_on(&connection, request)
    }

    /// Atomically records a control request and its initial delivery receipt.
    pub fn record_control(
        &self,
        request: ControlRequest,
        receipt: ControlReceipt,
    ) -> Result<RecordedControl> {
        validate_control_request(&request)?;
        validate_control_receipt(&receipt)?;
        if receipt.request_id != request.id {
            return Err(invalid(
                "control",
                "receipt must reference the control request",
            ));
        }
        if receipt.received_at_unix_ms < request.requested_at_unix_ms {
            return Err(invalid("control", "receipt cannot precede the request"));
        }
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        upsert_control_request_on(&transaction, &request)?;
        upsert_control_receipt_on(&transaction, &receipt)?;
        transaction.commit()?;
        Ok(RecordedControl { request, receipt })
    }

    pub fn get_control_request(&self, id: &str) -> Result<Option<ControlRequest>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, session_id, action, instruction, requested_at_unix_ms, requested_by_owner \
             FROM control_requests WHERE id = ?1",
        )?;
        let mut rows = statement.query([id])?;
        rows.next()?.map(decode_control_request).transpose()
    }

    pub fn list_control_requests(&self, session_id: &str) -> Result<Vec<ControlRequest>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, session_id, action, instruction, requested_at_unix_ms, requested_by_owner \
             FROM control_requests WHERE session_id = ?1 ORDER BY requested_at_unix_ms DESC, id",
        )?;
        collect_rows(statement.query([session_id])?, decode_control_request)
    }

    pub fn delete_control_request(&self, id: &str) -> Result<bool> {
        let connection = self.connection()?;
        delete_by_id(&connection, "control_requests", id)
    }

    pub fn upsert_control_receipt(&self, receipt: &ControlReceipt) -> Result<()> {
        validate_control_receipt(receipt)?;
        let connection = self.connection()?;
        upsert_control_receipt_on(&connection, receipt)
    }

    pub fn get_control_receipt(&self, id: &str) -> Result<Option<ControlReceipt>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, request_id, outcome, received_at_unix_ms, evidence, source, message, provider_receipt_id \
             FROM control_receipts WHERE id = ?1",
        )?;
        let mut rows = statement.query([id])?;
        rows.next()?.map(decode_control_receipt).transpose()
    }

    pub fn list_control_receipts(&self, request_id: &str) -> Result<Vec<ControlReceipt>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, request_id, outcome, received_at_unix_ms, evidence, source, message, provider_receipt_id \
             FROM control_receipts WHERE request_id = ?1 ORDER BY received_at_unix_ms DESC, id",
        )?;
        collect_rows(statement.query([request_id])?, decode_control_receipt)
    }

    pub fn delete_control_receipt(&self, id: &str) -> Result<bool> {
        let connection = self.connection()?;
        delete_by_id(&connection, "control_receipts", id)
    }

    pub fn search(&self, query: &SearchQuery) -> Result<Vec<SearchHit>> {
        let text = query.text.trim();
        if text.is_empty() {
            return Ok(Vec::new());
        }
        let pattern = format!("%{}%", escape_like(text));
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            r#"
            SELECT entity_kind, id, project_id, session_id, occurred_at_unix_ms, title, excerpt, evidence
            FROM (
                SELECT 'message' AS entity_kind, m.id, s.project_id, m.session_id,
                       m.sent_at_unix_ms AS occurred_at_unix_ms,
                       CASE m.role WHEN 'owner' THEN 'Owner message' WHEN 'agent' THEN 'Agent message' ELSE 'System message' END AS title,
                       substr(m.body, 1, 320) AS excerpt, m.evidence,
                       m.body AS haystack
                FROM messages m JOIN sessions s ON s.id = m.session_id
                UNION ALL
                SELECT 'event', e.id, s.project_id, e.session_id, e.occurred_at_unix_ms,
                       e.summary, substr(COALESCE(e.detail, e.summary), 1, 320), e.evidence,
                       e.summary || ' ' || COALESCE(e.detail, '')
                FROM session_events e JOIN sessions s ON s.id = e.session_id
                UNION ALL
                SELECT 'file_change', f.id, s.project_id, f.session_id, f.occurred_at_unix_ms,
                       f.path, CASE WHEN f.previous_path IS NULL THEN f.path ELSE f.previous_path || ' -> ' || f.path END,
                       f.evidence, f.path || ' ' || COALESCE(f.previous_path, '')
                FROM file_changes f JOIN sessions s ON s.id = f.session_id
                UNION ALL
                SELECT 'task', t.id, t.project_id, NULL, t.updated_at_unix_ms,
                       t.title, substr(t.detail, 1, 320), 'observed', t.title || ' ' || t.detail
                FROM tasks t
            ) searchable
            WHERE haystack LIKE ?1 ESCAPE '\'
              AND (?2 IS NULL OR project_id = ?2)
              AND (?3 IS NULL OR session_id = ?3)
            ORDER BY occurred_at_unix_ms DESC, entity_kind, id
            LIMIT ?4
            "#,
        )?;
        let mut rows = statement.query(params![
            pattern,
            query.project_id,
            query.session_id,
            i64::from(query.limit.clamp(1, 1_000))
        ])?;
        let mut hits = Vec::new();
        while let Some(row) = rows.next()? {
            hits.push(decode_search_hit(row)?);
        }
        Ok(hits)
    }
}

fn read_workspace_projection_on(
    connection: &Connection,
    scope: WorkspaceScope,
    currency: &str,
    after_projects: impl FnOnce(),
) -> Result<WorkspaceProjection> {
    let project_id = match &scope {
        WorkspaceScope::Global => None,
        WorkspaceScope::Project(project_id) => Some(project_id.as_str()),
    };
    let health = store_health_on(connection)?;
    let projects = {
        let mut statement = connection.prepare(
            "SELECT id, name, root_path, state, created_at_unix_ms FROM projects \
             WHERE (?1 IS NULL OR id = ?1) \
             ORDER BY CASE state WHEN 'active' THEN 0 WHEN 'paused' THEN 1 ELSE 2 END, \
                      name COLLATE NOCASE, id",
        )?;
        collect_rows(statement.query([project_id])?, decode_project)?
    };
    if let WorkspaceScope::Project(requested) = &scope
        && projects.is_empty()
    {
        return Err(StoreError::NotFound {
            entity: "project",
            id: requested.clone(),
        });
    }
    after_projects();

    let providers = {
        let mut statement = connection.prepare(
            "SELECT id, display_name, kind FROM providers \
             ORDER BY display_name COLLATE NOCASE, id",
        )?;
        collect_rows(statement.query([])?, decode_provider)?
    };
    let integrations = {
        let sql = format!(
            "SELECT id, provider_id, connector_key, display_name, kind, state, auth, evidence, \
             checked_at_unix_ms, problem, {CAPABILITY_COLUMNS} FROM integrations \
             ORDER BY display_name COLLATE NOCASE, id"
        );
        let mut statement = connection.prepare(&sql)?;
        collect_rows(statement.query([])?, decode_integration)?
    };
    let agents = {
        let sql = format!(
            "SELECT id, provider_id, connector_id, display_name, model, {CAPABILITY_COLUMNS} \
             FROM agents ORDER BY display_name COLLATE NOCASE, id"
        );
        let mut statement = connection.prepare(&sql)?;
        collect_rows(statement.query([])?, decode_agent)?
    };
    let mut tasks = {
        let mut statement = connection.prepare(
            "SELECT id, project_id, title, detail, state, created_at_unix_ms, updated_at_unix_ms \
             FROM tasks WHERE (?1 IS NULL OR project_id = ?1) \
             ORDER BY CASE state WHEN 'running' THEN 0 WHEN 'blocked' THEN 1 WHEN 'waiting' THEN 2 \
                 WHEN 'queued' THEN 3 WHEN 'draft' THEN 4 ELSE 5 END, updated_at_unix_ms DESC, id",
        )?;
        collect_rows(
            statement.query([project_id])?,
            decode_task_without_assignees,
        )?
    };
    for task in &mut tasks {
        task.assignee_agent_ids = query_task_assignees(connection, &task.id)?;
    }
    let sessions = {
        let mut statement = connection.prepare(
            "SELECT id, project_id, task_id, agent_id, provider_session_id, state, \
             started_at_unix_ms, last_observed_at_unix_ms, title_hint FROM sessions \
             WHERE (?1 IS NULL OR project_id = ?1) ORDER BY started_at_unix_ms DESC, id",
        )?;
        collect_rows(statement.query([project_id])?, decode_session)?
    };
    let attention = {
        let mut statement = connection.prepare(
            "SELECT id, project_id, task_id, session_id, agent_id, integration_id, severity, state, title, \
             detail, recovery, detected_at_unix_ms, updated_at_unix_ms, evidence, source \
             FROM attention_findings WHERE (?1 IS NULL OR project_id = ?1) \
             ORDER BY CASE severity WHEN 'blocked' THEN 0 WHEN 'needs_attention' THEN 1 \
                 WHEN 'unknown' THEN 2 ELSE 3 END, updated_at_unix_ms DESC, id",
        )?;
        collect_rows(statement.query([project_id])?, decode_attention)?
    };
    let handoffs = {
        let mut statement = connection.prepare(
            "SELECT id, project_id, task_id, from_agent_id, to_agent_id, instruction, created_at_unix_ms, \
             approved_by_owner, state, delivered_at_unix_ms, delivery_evidence, source, resulting_session_id, \
             correlation_id FROM handoffs WHERE (?1 IS NULL OR project_id = ?1) \
             ORDER BY created_at_unix_ms DESC, id",
        )?;
        collect_rows(statement.query([project_id])?, decode_handoff)?
    };
    let costs = projects
        .iter()
        .map(|project| {
            Ok(ProjectCostProjection {
                project_id: project.id.clone(),
                summary: cost_summary_on(connection, &project.id, currency)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(WorkspaceProjection {
        scope,
        health,
        providers,
        integrations,
        agents,
        projects,
        tasks,
        sessions,
        attention,
        handoffs,
        costs,
    })
}

fn read_session_projection_on(
    connection: &Connection,
    session_id: &str,
    message_query: StreamQuery,
    event_query: StreamQuery,
    related_limit: u32,
) -> Result<Option<SessionProjection>> {
    let session = {
        let mut statement = connection.prepare(
            "SELECT id, project_id, task_id, agent_id, provider_session_id, state, \
             started_at_unix_ms, last_observed_at_unix_ms, title_hint FROM sessions WHERE id = ?1",
        )?;
        let mut rows = statement.query([session_id])?;
        rows.next()?.map(decode_session).transpose()?
    };
    let Some(session) = session else {
        return Ok(None);
    };
    let message_after =
        optional_to_i64(message_query.after_sequence, "after_sequence")?.unwrap_or(0);
    let messages = {
        let mut statement = connection.prepare(
            "SELECT id, session_id, sequence, role, author_agent_id, body, sent_at_unix_ms, \
             ingested_at_unix_ms, evidence, source, correlation_id FROM messages \
             WHERE session_id = ?1 AND sequence > ?2 ORDER BY sequence ASC LIMIT ?3",
        )?;
        collect_rows(
            statement.query(params![
                session_id,
                message_after,
                i64::from(message_query.limit.clamp(1, 1_000))
            ])?,
            decode_message,
        )?
    };
    let event_after = optional_to_i64(event_query.after_sequence, "after_sequence")?.unwrap_or(0);
    let events = {
        let mut statement = connection.prepare(
            "SELECT id, session_id, sequence, occurred_at_unix_ms, kind, summary, detail, evidence, \
             source, ingested_at_unix_ms, provider_event_id, correlation_id FROM session_events \
             WHERE session_id = ?1 AND sequence > ?2 ORDER BY sequence ASC LIMIT ?3",
        )?;
        collect_rows(
            statement.query(params![
                session_id,
                event_after,
                i64::from(event_query.limit.clamp(1, 5_000))
            ])?,
            decode_event,
        )?
    };
    let related_limit = i64::from(related_limit.clamp(1, 5_000));
    let file_changes = {
        let mut statement = connection.prepare(
            "SELECT id, session_id, event_id, path, previous_path, kind, additions, deletions, \
             occurred_at_unix_ms, evidence, source FROM file_changes WHERE session_id = ?1 \
             ORDER BY occurred_at_unix_ms DESC, id LIMIT ?2",
        )?;
        collect_rows(
            statement.query(params![session_id, related_limit])?,
            decode_file_change,
        )?
    };
    let costs = {
        let mut statement = connection.prepare(
            "SELECT id, project_id, task_id, session_id, agent_id, currency, amount_micros, confidence, \
             occurred_at_unix_ms, ingested_at_unix_ms, evidence, source, note FROM cost_records \
             WHERE session_id = ?1 ORDER BY occurred_at_unix_ms DESC, id LIMIT ?2",
        )?;
        collect_rows(
            statement.query(params![session_id, related_limit])?,
            decode_cost,
        )?
    };
    let control_requests = {
        let mut statement = connection.prepare(
            "SELECT id, session_id, action, instruction, requested_at_unix_ms, requested_by_owner \
             FROM control_requests WHERE session_id = ?1 \
             ORDER BY requested_at_unix_ms DESC, id LIMIT ?2",
        )?;
        collect_rows(
            statement.query(params![session_id, related_limit])?,
            decode_control_request,
        )?
    };
    let control_receipts = {
        let mut statement = connection.prepare(
            "SELECT r.id, r.request_id, r.outcome, r.received_at_unix_ms, r.evidence, r.source, \
             r.message, r.provider_receipt_id FROM control_receipts r \
             JOIN control_requests q ON q.id = r.request_id WHERE q.session_id = ?1 \
             ORDER BY r.received_at_unix_ms DESC, r.id LIMIT ?2",
        )?;
        collect_rows(
            statement.query(params![session_id, related_limit])?,
            decode_control_receipt,
        )?
    };

    Ok(Some(SessionProjection {
        session,
        messages,
        events,
        file_changes,
        costs,
        control_requests,
        control_receipts,
    }))
}

fn store_health_on(connection: &Connection) -> Result<crate::StoreHealth> {
    let schema_version = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let foreign_keys: i64 =
        connection.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    let integrity: String = connection.pragma_query_value(None, "quick_check", |row| row.get(0))?;
    Ok(crate::StoreHealth {
        schema_version,
        latest_supported_schema_version: Store::latest_supported_schema_version(),
        integrity_ok: integrity == "ok",
        foreign_keys_enabled: foreign_keys == 1,
    })
}

fn cost_summary_on(
    connection: &Connection,
    project_id: &str,
    currency: &str,
) -> Result<CostSummary> {
    let currency = currency.to_ascii_uppercase();
    // Aggregate in bounded rowid pages rather than materializing records.
    // SQLite SUM is signed i64, so 32-bit limbs preserve the full u64 domain.
    let mut statement = connection.prepare(
        "SELECT MAX(rowid), COUNT(*), COUNT(amount_micros), \
                COALESCE(SUM(amount_micros / 4294967296), 0), \
                COALESCE(SUM(amount_micros % 4294967296), 0), \
                COALESCE(SUM(CASE WHEN amount_micros IS NULL THEN 1 ELSE 0 END), 0), \
                COALESCE(MAX(CASE WHEN amount_micros IS NOT NULL AND confidence = 'estimated' \
                                  THEN 1 ELSE 0 END), 0), \
                COALESCE(MAX(CASE WHEN amount_micros IS NOT NULL AND confidence = 'partial' \
                                  THEN 1 ELSE 0 END), 0) \
         FROM (SELECT rowid, amount_micros, confidence FROM cost_records \
               WHERE project_id = ?1 AND currency = ?2 AND (?3 IS NULL OR rowid > ?3) \
               ORDER BY rowid LIMIT ?4)",
    )?;
    let mut known_micros = 0_u64;
    let mut known_records = 0_u64;
    let mut unknown_records = 0_u64;
    let mut saw_estimate = false;
    let mut saw_partial = false;
    let mut after_rowid: Option<i64> = None;
    loop {
        let page = statement.query_row(
            params![project_id, currency, after_rowid, COST_SUMMARY_PAGE_ROWS],
            |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                ))
            },
        )?;
        let page_rows = to_u64(page.1, "cost summary page records")?;
        if page_rows == 0 {
            break;
        }
        let page_micros = u128::from(to_u64(page.3, "cost summary high limb")?)
            .checked_mul(COST_SUMMARY_LIMB_BASE)
            .and_then(|value| {
                value.checked_add(u128::from(to_u64(page.4, "cost summary low limb").ok()?))
            })
            .and_then(|value| u64::try_from(value).ok())
            .ok_or_else(|| invalid("cost summary", "sum exceeds unsigned 64-bit micros"))?;
        known_micros = known_micros
            .checked_add(page_micros)
            .ok_or_else(|| invalid("cost summary", "sum exceeds unsigned 64-bit micros"))?;
        known_records = known_records
            .checked_add(to_u64(page.2, "cost summary known records")?)
            .ok_or_else(|| invalid("cost summary", "known record count overflow"))?;
        unknown_records = unknown_records
            .checked_add(to_u64(page.5, "cost summary unknown records")?)
            .ok_or_else(|| invalid("cost summary", "unknown record count overflow"))?;
        saw_estimate |= page.6 != 0;
        saw_partial |= page.7 != 0;
        let last_rowid = page
            .0
            .ok_or_else(|| invalid("cost summary", "non-empty page had no rowid"))?;
        if after_rowid.is_some_and(|previous| last_rowid <= previous) {
            return Err(invalid("cost summary", "rowid cursor did not advance"));
        }
        after_rowid = Some(last_rowid);
    }
    let confidence = if known_records == 0 {
        CostConfidence::Unknown
    } else if unknown_records > 0 || saw_partial {
        CostConfidence::Partial
    } else if saw_estimate {
        CostConfidence::Estimated
    } else {
        CostConfidence::Exact
    };
    Ok(CostSummary {
        currency,
        known_micros,
        known_records,
        unknown_records,
        confidence,
    })
}

fn decode_provider(row: &Row<'_>) -> Result<Provider> {
    Ok(Provider {
        id: row.get(0)?,
        display_name: row.get(1)?,
        kind: ProviderKind::from_db(&row.get::<_, String>(2)?)?,
    })
}

fn decode_integration(row: &Row<'_>) -> Result<Integration> {
    Ok(Integration {
        id: row.get(0)?,
        provider_id: row.get(1)?,
        connector_key: row.get(2)?,
        display_name: row.get(3)?,
        kind: ProviderKind::from_db(&row.get::<_, String>(4)?)?,
        state: IntegrationState::from_db(&row.get::<_, String>(5)?)?,
        auth: AuthState::from_db(&row.get::<_, String>(6)?)?,
        evidence: EvidenceKind::from_db(&row.get::<_, String>(7)?)?,
        checked_at_unix_ms: optional_to_u64(row.get(8)?, "checked_at_unix_ms")?,
        problem: row.get(9)?,
        capabilities: read_capabilities(row, 10)?,
    })
}

fn decode_project(row: &Row<'_>) -> Result<Project> {
    Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        root_path: row.get(2)?,
        state: ProjectState::from_db(&row.get::<_, String>(3)?)?,
        created_at_unix_ms: to_u64(row.get(4)?, "created_at_unix_ms")?,
    })
}

fn decode_agent(row: &Row<'_>) -> Result<Agent> {
    Ok(Agent {
        id: row.get(0)?,
        provider_id: row.get(1)?,
        connector_id: row.get(2)?,
        display_name: row.get(3)?,
        model: row.get(4)?,
        capabilities: read_capabilities(row, 5)?,
    })
}

fn decode_task_without_assignees(row: &Row<'_>) -> Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        detail: row.get(3)?,
        state: TaskState::from_db(&row.get::<_, String>(4)?)?,
        assignee_agent_ids: Vec::new(),
        created_at_unix_ms: to_u64(row.get(5)?, "created_at_unix_ms")?,
        updated_at_unix_ms: to_u64(row.get(6)?, "updated_at_unix_ms")?,
    })
}

fn decode_session(row: &Row<'_>) -> Result<Session> {
    Ok(Session {
        id: row.get(0)?,
        project_id: row.get(1)?,
        task_id: row.get(2)?,
        agent_id: row.get(3)?,
        provider_session_id: row.get(4)?,
        state: AgentState::from_db(&row.get::<_, String>(5)?)?,
        started_at_unix_ms: to_u64(row.get(6)?, "started_at_unix_ms")?,
        last_observed_at_unix_ms: optional_to_u64(row.get(7)?, "last_observed_at_unix_ms")?,
        title_hint: row.get(8)?,
    })
}

fn decode_message(row: &Row<'_>) -> Result<Message> {
    Ok(Message {
        id: row.get(0)?,
        session_id: row.get(1)?,
        sequence: to_u64(row.get(2)?, "message sequence")?,
        role: MessageRole::from_db(&row.get::<_, String>(3)?)?,
        author_agent_id: row.get(4)?,
        body: row.get(5)?,
        sent_at_unix_ms: to_u64(row.get(6)?, "sent_at_unix_ms")?,
        ingested_at_unix_ms: to_u64(row.get(7)?, "ingested_at_unix_ms")?,
        evidence: EvidenceKind::from_db(&row.get::<_, String>(8)?)?,
        source: row.get(9)?,
        correlation_id: row.get(10)?,
    })
}

fn decode_event(row: &Row<'_>) -> Result<SessionEvent> {
    Ok(SessionEvent {
        id: row.get(0)?,
        session_id: row.get(1)?,
        sequence: to_u64(row.get(2)?, "event sequence")?,
        occurred_at_unix_ms: to_u64(row.get(3)?, "occurred_at_unix_ms")?,
        kind: EventKind::from_db(&row.get::<_, String>(4)?)?,
        summary: row.get(5)?,
        detail: row.get(6)?,
        evidence: EvidenceKind::from_db(&row.get::<_, String>(7)?)?,
        source: row.get(8)?,
        ingested_at_unix_ms: to_u64(row.get(9)?, "ingested_at_unix_ms")?,
        provider_event_id: row.get(10)?,
        correlation_id: row.get(11)?,
    })
}

fn decode_file_change(row: &Row<'_>) -> Result<FileChange> {
    Ok(FileChange {
        id: row.get(0)?,
        session_id: row.get(1)?,
        event_id: row.get(2)?,
        path: row.get(3)?,
        previous_path: row.get(4)?,
        kind: FileChangeKind::from_db(&row.get::<_, String>(5)?)?,
        additions: optional_to_u64(row.get(6)?, "additions")?,
        deletions: optional_to_u64(row.get(7)?, "deletions")?,
        occurred_at_unix_ms: to_u64(row.get(8)?, "occurred_at_unix_ms")?,
        evidence: EvidenceKind::from_db(&row.get::<_, String>(9)?)?,
        source: row.get(10)?,
    })
}

fn decode_cost(row: &Row<'_>) -> Result<CostRecord> {
    let currency: String = row.get(5)?;
    let micros = optional_to_u64(row.get(6)?, "amount_micros")?;
    let confidence = CostConfidence::from_db(&row.get::<_, String>(7)?)?;
    let amount = CostAmount::new(currency, micros, confidence)
        .map_err(|error| invalid("cost", error.to_string()))?;
    Ok(CostRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        task_id: row.get(2)?,
        session_id: row.get(3)?,
        agent_id: row.get(4)?,
        amount,
        occurred_at_unix_ms: to_u64(row.get(8)?, "occurred_at_unix_ms")?,
        ingested_at_unix_ms: to_u64(row.get(9)?, "ingested_at_unix_ms")?,
        evidence: EvidenceKind::from_db(&row.get::<_, String>(10)?)?,
        source: row.get(11)?,
        note: row.get(12)?,
    })
}

fn decode_attention(row: &Row<'_>) -> Result<AttentionRecord> {
    Ok(AttentionRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        task_id: row.get(2)?,
        session_id: row.get(3)?,
        agent_id: row.get(4)?,
        integration_id: row.get(5)?,
        severity: Severity::from_db(&row.get::<_, String>(6)?)?,
        state: AttentionState::from_db(&row.get::<_, String>(7)?)?,
        title: row.get(8)?,
        detail: row.get(9)?,
        recovery: row.get(10)?,
        detected_at_unix_ms: to_u64(row.get(11)?, "detected_at_unix_ms")?,
        updated_at_unix_ms: to_u64(row.get(12)?, "updated_at_unix_ms")?,
        evidence: EvidenceKind::from_db(&row.get::<_, String>(13)?)?,
        source: row.get(14)?,
    })
}

fn decode_handoff(row: &Row<'_>) -> Result<AgentHandoff> {
    Ok(AgentHandoff {
        id: row.get(0)?,
        project_id: row.get(1)?,
        task_id: row.get(2)?,
        from_agent_id: row.get(3)?,
        to_agent_id: row.get(4)?,
        instruction: row.get(5)?,
        created_at_unix_ms: to_u64(row.get(6)?, "created_at_unix_ms")?,
        approved_by_owner: read_bool(row, 7)?,
        state: HandoffState::from_db(&row.get::<_, String>(8)?)?,
        delivered_at_unix_ms: optional_to_u64(row.get(9)?, "delivered_at_unix_ms")?,
        delivery_evidence: EvidenceKind::from_db(&row.get::<_, String>(10)?)?,
        source: row.get(11)?,
        resulting_session_id: row.get(12)?,
        correlation_id: row.get(13)?,
    })
}

fn decode_control_request(row: &Row<'_>) -> Result<ControlRequest> {
    Ok(ControlRequest {
        id: row.get(0)?,
        session_id: row.get(1)?,
        action: ControlAction::from_db(&row.get::<_, String>(2)?)?,
        instruction: row.get(3)?,
        requested_at_unix_ms: to_u64(row.get(4)?, "requested_at_unix_ms")?,
        requested_by_owner: read_bool(row, 5)?,
    })
}

fn decode_control_receipt(row: &Row<'_>) -> Result<ControlReceipt> {
    Ok(ControlReceipt {
        id: row.get(0)?,
        request_id: row.get(1)?,
        outcome: ControlOutcome::from_db(&row.get::<_, String>(2)?)?,
        received_at_unix_ms: to_u64(row.get(3)?, "received_at_unix_ms")?,
        evidence: EvidenceKind::from_db(&row.get::<_, String>(4)?)?,
        source: row.get(5)?,
        message: row.get(6)?,
        provider_receipt_id: row.get(7)?,
    })
}

fn decode_search_hit(row: &Row<'_>) -> Result<SearchHit> {
    let entity = match row.get::<_, String>(0)?.as_str() {
        "message" => SearchEntityKind::Message,
        "event" => SearchEntityKind::Event,
        "file_change" => SearchEntityKind::FileChange,
        "task" => SearchEntityKind::Task,
        other => {
            return Err(StoreError::InvalidEnum {
                kind: "search entity kind",
                value: other.to_owned(),
            });
        }
    };
    Ok(SearchHit {
        entity,
        id: row.get(1)?,
        project_id: row.get(2)?,
        session_id: row.get(3)?,
        occurred_at_unix_ms: to_u64(row.get(4)?, "occurred_at_unix_ms")?,
        title: row.get(5)?,
        excerpt: row.get(6)?,
        evidence: EvidenceKind::from_db(&row.get::<_, String>(7)?)?,
    })
}

fn escape_like(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn capabilities_are_subset(
    candidate: ConnectorCapabilities,
    boundary: ConnectorCapabilities,
) -> bool {
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

fn ensure_integration_agent_compatibility(
    connection: &Connection,
    integration: &Integration,
) -> Result<()> {
    let c = integration.capabilities;
    let incompatible = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM agents WHERE connector_id = ?1 AND (\
             ?2 IS NULL OR provider_id <> ?2 \
             OR (can_observe = 1 AND ?3 = 0) \
             OR (can_auth_probe = 1 AND ?4 = 0) \
             OR (can_direct = 1 AND ?5 = 0) \
             OR (can_pause = 1 AND ?6 = 0) \
             OR (can_resume = 1 AND ?7 = 0) \
             OR (can_stop = 1 AND ?8 = 0) \
             OR (can_logs = 1 AND ?9 = 0) \
             OR (can_costs = 1 AND ?10 = 0) \
             OR (can_agent_messages = 1 AND ?11 = 0)))",
        params![
            integration.id,
            integration.provider_id,
            bool_i64(c.observe),
            bool_i64(c.auth_probe),
            bool_i64(c.direct),
            bool_i64(c.pause),
            bool_i64(c.resume),
            bool_i64(c.stop),
            bool_i64(c.logs),
            bool_i64(c.costs),
            bool_i64(c.agent_messages),
        ],
        |row| read_bool(row, 0),
    )?;
    if incompatible {
        return Err(invalid(
            "integration",
            "provider and capabilities cannot invalidate an existing agent",
        ));
    }
    Ok(())
}

fn ensure_task_dependents_match_project(
    connection: &Connection,
    task_id: &str,
    project_id: &str,
) -> Result<()> {
    let incompatible = connection.query_row(
        "SELECT EXISTS(\
             SELECT 1 FROM sessions WHERE task_id = ?1 AND project_id <> ?2 \
             UNION ALL \
             SELECT 1 FROM cost_records WHERE task_id = ?1 AND project_id <> ?2 \
             UNION ALL \
             SELECT 1 FROM handoffs WHERE task_id = ?1 AND project_id <> ?2 \
             UNION ALL \
             SELECT 1 FROM attention_findings WHERE task_id = ?1 \
                 AND (project_id IS NULL OR project_id <> ?2))",
        params![task_id, project_id],
        |row| read_bool(row, 0),
    )?;
    if incompatible {
        return Err(invalid(
            "task",
            "project change would invalidate related sessions, costs, or handoffs",
        ));
    }
    Ok(())
}

fn session_scope(
    connection: &Connection,
    session_id: &str,
) -> Result<Option<(String, Option<String>, String)>> {
    connection
        .query_row(
            "SELECT project_id, task_id, agent_id FROM sessions WHERE id = ?1",
            [session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(StoreError::from)
}

fn ensure_session_scope(connection: &Connection, session: &Session) -> Result<()> {
    if let Some(task_id) = session.task_id.as_deref() {
        let task_project = connection
            .query_row(
                "SELECT project_id FROM tasks WHERE id = ?1",
                [task_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if task_project.as_deref() != Some(&session.project_id) {
            return Err(invalid(
                "session",
                "task must belong to the session project",
            ));
        }
    }
    Ok(())
}

fn ensure_session_dependents_match_scope(connection: &Connection, session: &Session) -> Result<()> {
    let incompatible_message = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM messages WHERE session_id = ?1 AND role = 'agent' \
             AND author_agent_id <> ?2)",
        params![session.id, session.agent_id],
        |row| read_bool(row, 0),
    )?;
    let incompatible_cost = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM cost_records WHERE session_id = ?1 AND (\
             project_id <> ?2 \
             OR (task_id IS NOT NULL AND (?3 IS NULL OR task_id <> ?3)) \
             OR (agent_id IS NOT NULL AND agent_id <> ?4)))",
        params![
            session.id,
            session.project_id,
            session.task_id,
            session.agent_id
        ],
        |row| read_bool(row, 0),
    )?;
    let incompatible_handoff = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM handoffs WHERE resulting_session_id = ?1 AND (\
             project_id <> ?2 OR ?3 IS NULL OR task_id <> ?3 OR to_agent_id <> ?4))",
        params![
            session.id,
            session.project_id,
            session.task_id,
            session.agent_id
        ],
        |row| read_bool(row, 0),
    )?;
    let incompatible_attention = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM attention_findings WHERE session_id = ?1 AND (\
             project_id IS NULL OR project_id <> ?2 \
             OR (task_id IS NOT NULL AND (?3 IS NULL OR task_id <> ?3)) \
             OR (agent_id IS NOT NULL AND agent_id <> ?4)))",
        params![
            session.id,
            session.project_id,
            session.task_id,
            session.agent_id
        ],
        |row| read_bool(row, 0),
    )?;
    if incompatible_message || incompatible_cost || incompatible_handoff || incompatible_attention {
        return Err(invalid(
            "session",
            "scope change would invalidate related messages, costs, handoffs, or attention",
        ));
    }
    Ok(())
}

fn ensure_message_author_matches_session(
    connection: &Connection,
    session_id: &str,
    role: MessageRole,
    author_agent_id: Option<&str>,
) -> Result<()> {
    if role != MessageRole::Agent {
        return Ok(());
    }
    let session_agent_id = connection
        .query_row(
            "SELECT agent_id FROM sessions WHERE id = ?1",
            [session_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let Some(session_agent_id) = session_agent_id else {
        return Err(invalid("message", "session was not found"));
    };
    if author_agent_id != Some(session_agent_id.as_str()) {
        return Err(invalid(
            "message",
            "agent author must match the target session agent",
        ));
    }
    Ok(())
}

fn normalized_attention_scope(
    connection: &Connection,
    finding: &AttentionRecord,
) -> Result<AttentionRecord> {
    let mut normalized = finding.clone();

    if let Some(session_id) = finding.session_id.as_deref() {
        let Some((project_id, task_id, agent_id)) = session_scope(connection, session_id)? else {
            return Err(invalid("attention finding", "session was not found"));
        };
        if normalized
            .project_id
            .as_deref()
            .is_some_and(|value| value != project_id)
        {
            return Err(invalid(
                "attention finding",
                "session does not belong to the finding project",
            ));
        }
        if normalized
            .task_id
            .as_deref()
            .is_some_and(|value| task_id.as_deref() != Some(value))
        {
            return Err(invalid(
                "attention finding",
                "session does not belong to the finding task",
            ));
        }
        if normalized
            .agent_id
            .as_deref()
            .is_some_and(|value| value != agent_id)
        {
            return Err(invalid(
                "attention finding",
                "session does not belong to the finding agent",
            ));
        }
        normalized.project_id.get_or_insert(project_id);
        if normalized.task_id.is_none() {
            normalized.task_id = task_id;
        }
        normalized.agent_id.get_or_insert(agent_id);
    }
    if let Some(task_id) = normalized.task_id.as_deref() {
        let task_project = connection
            .query_row(
                "SELECT project_id FROM tasks WHERE id = ?1",
                [task_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(task_project) = task_project else {
            return Err(invalid("attention finding", "task was not found"));
        };
        if normalized
            .project_id
            .as_deref()
            .is_some_and(|value| value != task_project)
        {
            return Err(invalid(
                "attention finding",
                "task does not belong to the finding project",
            ));
        }
        normalized.project_id.get_or_insert(task_project);
    }
    if let Some(integration_id) = finding.integration_id.as_deref() {
        let integration_provider = connection
            .query_row(
                "SELECT provider_id FROM integrations WHERE id = ?1",
                [integration_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?;
        let Some(integration_provider) = integration_provider else {
            return Err(invalid("attention finding", "integration was not found"));
        };
        if let Some(agent_id) = normalized.agent_id.as_deref() {
            let agent_connector = connection
                .query_row(
                    "SELECT connector_id, provider_id FROM agents WHERE id = ?1",
                    [agent_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?;
            let Some((connector_id, provider_id)) = agent_connector else {
                return Err(invalid("attention finding", "agent was not found"));
            };
            if connector_id != integration_id
                || integration_provider.as_deref() != Some(&provider_id)
            {
                return Err(invalid(
                    "attention finding",
                    "agent does not belong to the finding integration",
                ));
            }
        }
    } else if let Some(agent_id) = normalized.agent_id.as_deref() {
        let exists = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM agents WHERE id = ?1)",
            [agent_id],
            |row| read_bool(row, 0),
        )?;
        if !exists {
            return Err(invalid("attention finding", "agent was not found"));
        }
    }
    Ok(normalized)
}

fn ensure_cost_scope(connection: &Connection, cost: &CostRecord) -> Result<()> {
    if let Some(task_id) = cost.task_id.as_deref() {
        let task_project = connection
            .query_row(
                "SELECT project_id FROM tasks WHERE id = ?1",
                [task_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if task_project.as_deref() != Some(&cost.project_id) {
            return Err(invalid("cost", "task must belong to the cost project"));
        }
    }
    if let Some(session_id) = cost.session_id.as_deref() {
        let Some((project_id, task_id, agent_id)) = session_scope(connection, session_id)? else {
            return Err(invalid("cost", "session was not found"));
        };
        if project_id != cost.project_id {
            return Err(invalid("cost", "session must belong to the cost project"));
        }
        if cost
            .task_id
            .as_deref()
            .is_some_and(|cost_task| task_id.as_deref() != Some(cost_task))
        {
            return Err(invalid(
                "cost",
                "task must match the attributed session task",
            ));
        }
        if cost
            .agent_id
            .as_deref()
            .is_some_and(|cost_agent| agent_id != cost_agent)
        {
            return Err(invalid(
                "cost",
                "agent must match the attributed session agent",
            ));
        }
    }
    Ok(())
}

fn validate_attention(finding: &AttentionRecord) -> Result<()> {
    validate_id("attention finding", &finding.id)?;
    validate_text("attention finding", "title", &finding.title)?;
    validate_evidence_source("attention finding", finding.evidence, &finding.source)?;
    if finding.updated_at_unix_ms < finding.detected_at_unix_ms {
        return Err(invalid(
            "attention finding",
            "updated_at precedes detected_at",
        ));
    }
    if finding.severity == Severity::Healthy && finding.state == AttentionState::Open {
        return Err(invalid(
            "attention finding",
            "healthy status cannot remain an open attention item",
        ));
    }
    if finding.severity == Severity::Healthy && finding.evidence != EvidenceKind::Observed {
        return Err(invalid(
            "attention finding",
            "healthy status requires observed evidence",
        ));
    }
    Ok(())
}

fn store_error_to_sql(error: StoreError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

const fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Healthy => 0,
        Severity::Unknown => 1,
        Severity::NeedsAttention => 2,
        Severity::Blocked => 3,
    }
}

fn validate_control_request(request: &ControlRequest) -> Result<()> {
    validate_id("control request", &request.id)?;
    if request.action == ControlAction::Direct
        && request
            .instruction
            .as_deref()
            .is_none_or(|text| text.trim().is_empty())
    {
        return Err(invalid(
            "control request",
            "direct action requires an instruction",
        ));
    }
    Ok(())
}

fn upsert_control_request_on(connection: &Connection, request: &ControlRequest) -> Result<()> {
    let changed = connection.execute(
        "INSERT INTO control_requests (id, session_id, action, instruction, requested_at_unix_ms, requested_by_owner) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(id) DO NOTHING",
        params![
            request.id,
            request.session_id,
            request.action.db_value(),
            request.instruction,
            to_i64(request.requested_at_unix_ms, "requested_at_unix_ms")?,
            bool_i64(request.requested_by_owner),
        ],
    )?;
    if changed == 0 {
        let mut statement = connection.prepare(
            "SELECT id, session_id, action, instruction, requested_at_unix_ms, requested_by_owner \
             FROM control_requests WHERE id = ?1",
        )?;
        let existing = statement.query_row([&request.id], |row| {
            decode_control_request(row).map_err(store_error_to_sql)
        })?;
        if existing != *request {
            return Err(invalid(
                "control request",
                "an immutable request ID was reused with different content",
            ));
        }
    }
    Ok(())
}

fn validate_control_receipt(receipt: &ControlReceipt) -> Result<()> {
    validate_id("control receipt", &receipt.id)?;
    validate_evidence_source("control receipt", receipt.evidence, &receipt.source)?;
    if receipt.outcome == ControlOutcome::Acknowledged && receipt.evidence != EvidenceKind::Observed
    {
        return Err(invalid(
            "control receipt",
            "acknowledged outcome requires observed evidence",
        ));
    }
    Ok(())
}

fn upsert_control_receipt_on(connection: &Connection, receipt: &ControlReceipt) -> Result<()> {
    let changed = connection.execute(
        "INSERT INTO control_receipts (id, request_id, outcome, received_at_unix_ms, evidence, source, \
         message, provider_receipt_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
         ON CONFLICT(id) DO NOTHING",
        params![
            receipt.id,
            receipt.request_id,
            receipt.outcome.db_value(),
            to_i64(receipt.received_at_unix_ms, "received_at_unix_ms")?,
            receipt.evidence.db_value(),
            receipt.source,
            receipt.message,
            receipt.provider_receipt_id,
        ],
    )?;
    if changed == 0 {
        let mut statement = connection.prepare(
            "SELECT id, request_id, outcome, received_at_unix_ms, evidence, source, message, provider_receipt_id \
             FROM control_receipts WHERE id = ?1",
        )?;
        let existing = statement.query_row([&receipt.id], |row| {
            decode_control_receipt(row).map_err(store_error_to_sql)
        })?;
        if existing != *receipt {
            return Err(invalid(
                "control receipt",
                "an immutable receipt ID was reused with different content",
            ));
        }
    }
    Ok(())
}

fn validate_handoff(handoff: &AgentHandoff) -> Result<()> {
    validate_id("handoff", &handoff.id)?;
    validate_text("handoff", "instruction", &handoff.instruction)?;
    validate_evidence_source("handoff", handoff.delivery_evidence, &handoff.source)?;
    if handoff.from_agent_id == handoff.to_agent_id {
        return Err(invalid(
            "handoff",
            "sender and recipient must be different agents",
        ));
    }
    if matches!(
        handoff.state,
        HandoffState::Approved | HandoffState::Delivered
    ) && !handoff.approved_by_owner
    {
        return Err(invalid(
            "handoff",
            "approved or delivered handoff requires owner approval",
        ));
    }
    if handoff.state == HandoffState::Delivered
        && (handoff.delivered_at_unix_ms.is_none()
            || handoff.delivery_evidence != EvidenceKind::Observed)
    {
        return Err(invalid(
            "handoff",
            "delivered handoff requires a timestamp and observed delivery evidence",
        ));
    }
    if handoff
        .delivered_at_unix_ms
        .is_some_and(|value| value < handoff.created_at_unix_ms)
    {
        return Err(invalid("handoff", "delivery precedes creation"));
    }
    Ok(())
}

fn validate_cost(cost: &CostRecord) -> Result<()> {
    validate_id("cost", &cost.id)?;
    validate_evidence_source("cost", cost.evidence, &cost.source)?;
    cost.amount
        .validate()
        .map_err(|error| invalid("cost", error.to_string()))?;
    if cost.amount.currency != cost.amount.currency.to_ascii_uppercase() {
        return Err(invalid("cost", "currency must be uppercase"));
    }
    if cost.amount.confidence == CostConfidence::Exact && cost.evidence != EvidenceKind::Observed {
        return Err(invalid("cost", "exact cost requires observed evidence"));
    }
    if cost.evidence == EvidenceKind::Unsupported && cost.amount.micros.is_some() {
        return Err(invalid(
            "cost",
            "unsupported cost evidence cannot contain an amount",
        ));
    }
    Ok(())
}

fn validate_message(message: &Message) -> Result<()> {
    validate_id("message", &message.id)?;
    validate_text("message", "body", &message.body)?;
    validate_evidence_source("message", message.evidence, &message.source)?;
    validate_message_author(message.role, message.author_agent_id.as_deref())?;
    if message.sequence == 0 {
        return Err(invalid("message", "sequence must start at one"));
    }
    Ok(())
}

fn validate_message_author(role: MessageRole, author_agent_id: Option<&str>) -> Result<()> {
    match (role, author_agent_id) {
        (MessageRole::Agent, None) => Err(invalid(
            "message",
            "agent messages require an author_agent_id",
        )),
        (MessageRole::Owner | MessageRole::System, Some(_)) => Err(invalid(
            "message",
            "only agent messages may contain author_agent_id",
        )),
        _ => Ok(()),
    }
}

fn insert_message_on(connection: &Connection, message: &Message) -> Result<()> {
    validate_message(message)?;
    connection.execute(
        "INSERT INTO messages (id, session_id, sequence, role, author_agent_id, body, sent_at_unix_ms, \
         ingested_at_unix_ms, evidence, source, correlation_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            message.id,
            message.session_id,
            to_i64(message.sequence, "message sequence")?,
            message.role.db_value(),
            message.author_agent_id,
            message.body,
            to_i64(message.sent_at_unix_ms, "sent_at_unix_ms")?,
            to_i64(message.ingested_at_unix_ms, "ingested_at_unix_ms")?,
            message.evidence.db_value(),
            message.source,
            message.correlation_id,
        ],
    )?;
    Ok(())
}

fn validate_event(event: &SessionEvent) -> Result<()> {
    validate_id("event", &event.id)?;
    validate_text("event", "summary", &event.summary)?;
    validate_evidence_source("event", event.evidence, &event.source)?;
    if event.sequence == 0 {
        return Err(invalid("event", "sequence must start at one"));
    }
    Ok(())
}

fn insert_event_on(connection: &Connection, event: &SessionEvent) -> Result<()> {
    validate_event(event)?;
    connection.execute(
        "INSERT INTO session_events (id, session_id, sequence, occurred_at_unix_ms, ingested_at_unix_ms, \
         kind, summary, detail, evidence, source, provider_event_id, correlation_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            event.id,
            event.session_id,
            to_i64(event.sequence, "event sequence")?,
            to_i64(event.occurred_at_unix_ms, "occurred_at_unix_ms")?,
            to_i64(event.ingested_at_unix_ms, "ingested_at_unix_ms")?,
            event.kind.db_value(),
            event.summary,
            event.detail,
            event.evidence.db_value(),
            event.source,
            event.provider_event_id,
            event.correlation_id,
        ],
    )?;
    Ok(())
}

fn next_sequence(connection: &Connection, table: &'static str, session_id: &str) -> Result<u64> {
    debug_assert!(matches!(table, "messages" | "session_events"));
    let current: i64 = connection.query_row(
        &format!("SELECT COALESCE(MAX(sequence), 0) FROM {table} WHERE session_id = ?1"),
        [session_id],
        |row| row.get(0),
    )?;
    to_u64(current, "stream sequence")?
        .checked_add(1)
        .ok_or_else(|| invalid("stream", "sequence exhausted"))
}

fn replace_task_assignees(
    connection: &Connection,
    task_id: &str,
    agent_ids: &[String],
) -> Result<()> {
    connection.execute("DELETE FROM task_assignees WHERE task_id = ?1", [task_id])?;
    let mut unique_ids = agent_ids.to_vec();
    unique_ids.sort();
    unique_ids.dedup();
    let mut statement =
        connection.prepare("INSERT INTO task_assignees (task_id, agent_id) VALUES (?1, ?2)")?;
    for agent_id in unique_ids {
        validate_id("agent assignment", &agent_id)?;
        statement.execute(params![task_id, agent_id])?;
    }
    Ok(())
}

fn query_task_assignees(connection: &Connection, task_id: &str) -> Result<Vec<String>> {
    let mut statement = connection
        .prepare("SELECT agent_id FROM task_assignees WHERE task_id = ?1 ORDER BY agent_id")?;
    let mut rows = statement.query([task_id])?;
    let mut ids = Vec::new();
    while let Some(row) = rows.next()? {
        ids.push(row.get(0)?);
    }
    Ok(ids)
}

fn validate_id(entity: &'static str, id: &str) -> Result<()> {
    validate_text(entity, "id", id)
}

fn validate_text(entity: &'static str, field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(invalid(entity, format!("{field} cannot be empty")));
    }
    Ok(())
}

fn validate_evidence_source(
    entity: &'static str,
    _evidence: EvidenceKind,
    source: &str,
) -> Result<()> {
    validate_text(entity, "evidence source", source)
}

fn invalid(entity: &'static str, reason: impl Into<String>) -> StoreError {
    StoreError::InvalidRecord {
        entity,
        reason: reason.into(),
    }
}

fn collect_rows<T>(
    mut rows: rusqlite::Rows<'_>,
    decode: fn(&Row<'_>) -> Result<T>,
) -> Result<Vec<T>> {
    let mut values = Vec::new();
    while let Some(row) = rows.next()? {
        values.push(decode(row)?);
    }
    Ok(values)
}

fn delete_by_id(connection: &Connection, table: &'static str, id: &str) -> Result<bool> {
    let allowed = [
        "providers",
        "integrations",
        "projects",
        "tasks",
        "agents",
        "sessions",
        "messages",
        "session_events",
        "file_changes",
        "cost_records",
        "attention_findings",
        "handoffs",
        "control_requests",
        "control_receipts",
    ];
    debug_assert!(allowed.contains(&table));
    let changed = connection.execute(&format!("DELETE FROM {table} WHERE id = ?1"), [id])?;
    Ok(changed > 0)
}

#[cfg(test)]
mod tests {
    use std::{sync::mpsc, thread, time::Duration};

    use super::*;

    fn store_with_project() -> Store {
        let store = Store::open_in_memory().expect("in-memory store");
        store
            .upsert_project(&Project {
                id: "project".into(),
                name: "Cost aggregation".into(),
                root_path: None,
                state: ProjectState::Active,
                created_at_unix_ms: 1,
            })
            .expect("project");
        store
    }

    #[test]
    fn integration_agent_activation_rolls_back_ready_state_when_agent_write_fails() {
        let store = Store::open_in_memory().expect("store");
        store
            .upsert_provider(&Provider {
                id: "provider".into(),
                display_name: "Provider".into(),
                kind: ProviderKind::LocalCli,
            })
            .expect("provider");
        {
            let connection = store.connection().expect("connection");
            connection
                .execute_batch(
                    "CREATE TRIGGER reject_activation_agent \
                     BEFORE INSERT ON agents WHEN NEW.id = 'agent' \
                     BEGIN SELECT RAISE(ABORT, 'hostile agent failure'); END;",
                )
                .expect("hostile trigger");
        }
        let capabilities = ConnectorCapabilities {
            observe: true,
            auth_probe: true,
            direct: true,
            agent_messages: true,
            ..ConnectorCapabilities::default()
        };
        let integration = Integration {
            id: "integration".into(),
            provider_id: Some("provider".into()),
            connector_key: "integration".into(),
            display_name: "Integration".into(),
            kind: ProviderKind::LocalCli,
            state: IntegrationState::Ready,
            auth: AuthState::Confirmed,
            evidence: EvidenceKind::Observed,
            checked_at_unix_ms: Some(1),
            problem: None,
            capabilities,
        };
        let agent = Agent {
            id: "agent".into(),
            provider_id: "provider".into(),
            connector_id: "integration".into(),
            display_name: "Agent".into(),
            model: None,
            capabilities,
        };

        assert!(
            store
                .activate_integration_agent(&integration, &agent)
                .is_err()
        );
        assert!(
            store
                .get_integration("integration")
                .expect("integration read")
                .is_none()
        );
        assert!(store.get_agent("agent").expect("agent read").is_none());
    }

    #[test]
    fn cost_summary_aggregates_every_row_beyond_former_list_limit() {
        let store = store_with_project();
        {
            let mut connection = store.connection().expect("connection");
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .expect("transaction");
            let mut insert = transaction
                .prepare(
                    "INSERT INTO cost_records \
                     (id, project_id, currency, amount_micros, confidence, occurred_at_unix_ms, \
                      ingested_at_unix_ms, evidence, source) \
                     VALUES (?1, 'project', 'USD', ?2, ?3, ?4, ?4, 'observed', 'boundary-test')",
                )
                .expect("insert statement");
            for index in 0_i64..=50_001 {
                let is_unknown = index == 50_001;
                let amount = (!is_unknown).then_some(1_i64);
                let confidence = if is_unknown {
                    "unknown"
                } else if index == 4_097 {
                    "estimated"
                } else {
                    "exact"
                };
                insert
                    .execute(params![format!("bulk-{index}"), amount, confidence, index])
                    .expect("insert cost");
            }
            drop(insert);
            transaction.commit().expect("commit costs");
        }

        let partial = store.cost_summary("project", "usd").expect("summary");
        assert_eq!(partial.known_micros, 50_001);
        assert_eq!(partial.known_records, 50_001);
        assert_eq!(partial.unknown_records, 1);
        assert_eq!(partial.confidence, CostConfidence::Partial);

        assert!(store.delete_cost("bulk-50001").expect("delete unknown"));
        let estimated = store.cost_summary("project", "USD").expect("summary");
        assert_eq!(estimated.known_micros, 50_001);
        assert_eq!(estimated.known_records, 50_001);
        assert_eq!(estimated.unknown_records, 0);
        assert_eq!(estimated.confidence, CostConfidence::Estimated);
    }

    #[test]
    fn cost_summary_preserves_full_u64_range_and_rejects_overflow() {
        let store = store_with_project();
        let maximum_signed = i64::MAX;
        {
            let connection = store.connection().expect("connection");
            for (id, amount) in [("max-a", maximum_signed), ("max-b", maximum_signed)] {
                connection
                    .execute(
                        "INSERT INTO cost_records \
                         (id, project_id, currency, amount_micros, confidence, occurred_at_unix_ms, \
                          ingested_at_unix_ms, evidence, source) \
                         VALUES (?1, 'project', 'USD', ?2, 'exact', 1, 1, 'observed', 'overflow-test')",
                        params![id, amount],
                    )
                    .expect("insert maximum cost");
            }
        }

        let maximum = store.cost_summary("project", "USD").expect("summary");
        assert_eq!(maximum.known_micros, u64::MAX - 1);
        assert_eq!(maximum.confidence, CostConfidence::Exact);

        {
            let connection = store.connection().expect("connection");
            connection
                .execute(
                    "INSERT INTO cost_records \
                     (id, project_id, currency, amount_micros, confidence, occurred_at_unix_ms, \
                      ingested_at_unix_ms, evidence, source) \
                     VALUES ('overflow', 'project', 'USD', 2, 'exact', 1, 1, 'observed', 'overflow-test')",
                    [],
                )
                .expect("insert overflowing cost");
        }
        let error = store.cost_summary("project", "USD").unwrap_err();
        assert!(matches!(
            error,
            StoreError::InvalidRecord {
                entity: "cost summary",
                ..
            }
        ));
    }

    #[test]
    fn workspace_projection_holds_one_snapshot_across_concurrent_commit() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("projection.sqlite3");
        let reader = Store::open(&path).expect("reader store");
        let writer = Store::open(&path).expect("writer store");
        reader
            .upsert_project(&Project {
                id: "before".into(),
                name: "Before".into(),
                root_path: None,
                state: ProjectState::Active,
                created_at_unix_ms: 1,
            })
            .expect("initial project");
        let (start_write, write_started) = mpsc::channel();
        let (written, write_done) = mpsc::channel();
        let writer_thread = thread::spawn(move || {
            write_started.recv().expect("start signal");
            writer
                .upsert_project(&Project {
                    id: "during".into(),
                    name: "During".into(),
                    root_path: None,
                    state: ProjectState::Active,
                    created_at_unix_ms: 2,
                })
                .expect("concurrent project");
            written.send(()).expect("done signal");
        });

        let projection = reader
            .read_workspace_projection_with_hook(WorkspaceScope::Global, "USD", || {
                start_write.send(()).expect("start writer");
                write_done
                    .recv_timeout(Duration::from_secs(2))
                    .expect("writer commits while read transaction is open");
            })
            .expect("projection");
        writer_thread.join().expect("writer thread");

        assert_eq!(
            projection
                .projects
                .iter()
                .map(|project| project.id.as_str())
                .collect::<Vec<_>>(),
            vec!["before"]
        );
        assert_eq!(reader.list_projects().expect("fresh read").len(), 2);
    }
}
