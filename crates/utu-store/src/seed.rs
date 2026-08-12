use utu_core::{
    Agent, AgentHandoff, AgentState, AttentionRecord, AttentionState, AuthState,
    ConnectorCapabilities, CostAmount, CostRecord, EvidenceKind, FileChange, FileChangeKind,
    HandoffState, Integration, IntegrationState, MessageRole, Project, ProjectState, Provider,
    ProviderKind, Session, Severity, Task, TaskState,
};

use crate::{NewMessage, NewSessionEvent, Result, Store, invalid_seed};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemoSeedReport {
    pub project_id: String,
    pub providers: u32,
    pub agents: u32,
    pub tasks: u32,
    pub sessions: u32,
    pub messages: u32,
    pub events: u32,
}

impl Store {
    /// Seeds a clearly labeled, synthetic workspace. This method is never
    /// called by `open`; callers must opt in and use a database with no
    /// projects so demonstration content cannot overwrite real work.
    pub fn seed_demo(&self) -> Result<DemoSeedReport> {
        if !self.list_projects()?.is_empty() {
            return Err(invalid_seed(
                "demo seeding requires a database with no projects",
            ));
        }

        let observed_at = 1_786_474_800_000_u64;
        let source = "utu.demo";
        self.upsert_provider(&Provider {
            id: "demo-provider-local".into(),
            display_name: "Local CLI (demo)".into(),
            kind: ProviderKind::LocalCli,
        })?;
        self.upsert_provider(&Provider {
            id: "demo-provider-cloud".into(),
            display_name: "Cloud API (demo)".into(),
            kind: ProviderKind::CloudApi,
        })?;
        let demo_capabilities = ConnectorCapabilities {
            observe: true,
            auth_probe: true,
            direct: true,
            pause: true,
            resume: true,
            stop: true,
            logs: true,
            costs: true,
            agent_messages: true,
        };
        for integration in [
            Integration {
                id: "demo-integration-local".into(),
                provider_id: Some("demo-provider-local".into()),
                connector_key: "demo.local".into(),
                display_name: "Local connector (demo)".into(),
                kind: ProviderKind::LocalCli,
                state: IntegrationState::Unknown,
                auth: AuthState::Unknown,
                evidence: EvidenceKind::Inferred,
                checked_at_unix_ms: Some(observed_at),
                problem: Some("Synthetic state; no provider was contacted.".into()),
                capabilities: demo_capabilities,
            },
            Integration {
                id: "demo-integration-cloud".into(),
                provider_id: Some("demo-provider-cloud".into()),
                connector_key: "demo.cloud".into(),
                display_name: "Cloud connector (demo)".into(),
                kind: ProviderKind::CloudApi,
                state: IntegrationState::Unknown,
                auth: AuthState::Unknown,
                evidence: EvidenceKind::Inferred,
                checked_at_unix_ms: Some(observed_at),
                problem: Some("Synthetic state; no provider was contacted.".into()),
                capabilities: demo_capabilities,
            },
        ] {
            self.upsert_integration(&integration)?;
        }
        for agent in [
            Agent {
                id: "demo-agent-builder".into(),
                provider_id: "demo-provider-local".into(),
                connector_id: "demo-integration-local".into(),
                display_name: "Builder (demo)".into(),
                model: Some("local-demo".into()),
                capabilities: demo_capabilities,
            },
            Agent {
                id: "demo-agent-reviewer".into(),
                provider_id: "demo-provider-cloud".into(),
                connector_id: "demo-integration-cloud".into(),
                display_name: "Reviewer (demo)".into(),
                model: Some("cloud-demo".into()),
                capabilities: demo_capabilities,
            },
        ] {
            self.upsert_agent(&agent)?;
        }

        self.upsert_project(&Project {
            id: "demo-project-utu".into(),
            name: "Utu demonstration workspace".into(),
            root_path: Some("~/Projects/Utu-Demo".into()),
            state: ProjectState::Active,
            created_at_unix_ms: observed_at - 60_000,
        })?;
        self.upsert_task(&Task {
            id: "demo-task-chat".into(),
            project_id: "demo-project-utu".into(),
            title: "Prototype provider-neutral chat".into(),
            detail: "Synthetic task used to preview assignment, chat, files, and evidence.".into(),
            state: TaskState::Running,
            assignee_agent_ids: vec!["demo-agent-builder".into(), "demo-agent-reviewer".into()],
            created_at_unix_ms: observed_at - 50_000,
            updated_at_unix_ms: observed_at,
        })?;
        self.upsert_session(&Session {
            id: "demo-session-builder".into(),
            project_id: "demo-project-utu".into(),
            task_id: Some("demo-task-chat".into()),
            agent_id: "demo-agent-builder".into(),
            provider_session_id: Some("synthetic-session".into()),
            state: AgentState::Running,
            started_at_unix_ms: observed_at - 45_000,
            last_observed_at_unix_ms: Some(observed_at),
        })?;

        self.append_message(NewMessage {
            id: "demo-message-owner".into(),
            session_id: "demo-session-builder".into(),
            role: MessageRole::Owner,
            author_agent_id: None,
            body: "Create a provider-neutral chat workspace. This is demonstration data.".into(),
            sent_at_unix_ms: observed_at - 40_000,
            ingested_at_unix_ms: observed_at - 40_000,
            evidence: EvidenceKind::Inferred,
            source: source.into(),
            correlation_id: Some("demo-chat".into()),
        })?;
        self.append_message(NewMessage {
            id: "demo-message-agent".into(),
            session_id: "demo-session-builder".into(),
            role: MessageRole::Agent,
            author_agent_id: Some("demo-agent-builder".into()),
            body: "I prepared the session shell and a file preview. No live agent produced this message.".into(),
            sent_at_unix_ms: observed_at - 30_000,
            ingested_at_unix_ms: observed_at - 30_000,
            evidence: EvidenceKind::Inferred,
            source: source.into(),
            correlation_id: Some("demo-chat".into()),
        })?;
        let event = self.append_event(NewSessionEvent {
            id: "demo-event-file".into(),
            session_id: "demo-session-builder".into(),
            occurred_at_unix_ms: observed_at - 20_000,
            ingested_at_unix_ms: observed_at - 20_000,
            kind: utu_core::EventKind::FileChange,
            summary: "Prepared a synthetic chat panel".into(),
            detail: Some("Demonstration event; no filesystem mutation was observed.".into()),
            evidence: EvidenceKind::Inferred,
            source: source.into(),
            provider_event_id: Some("demo-file-1".into()),
            correlation_id: Some("demo-chat".into()),
        })?;
        self.upsert_file_change(&FileChange {
            id: "demo-file-chat".into(),
            session_id: "demo-session-builder".into(),
            event_id: Some(event.id),
            path: "src/chat.rs".into(),
            previous_path: None,
            kind: FileChangeKind::Modified,
            additions: Some(42),
            deletions: Some(7),
            occurred_at_unix_ms: observed_at - 20_000,
            evidence: EvidenceKind::Inferred,
            source: source.into(),
        })?;
        self.upsert_cost(&CostRecord {
            id: "demo-cost-estimate".into(),
            project_id: "demo-project-utu".into(),
            task_id: Some("demo-task-chat".into()),
            session_id: Some("demo-session-builder".into()),
            agent_id: Some("demo-agent-builder".into()),
            amount: CostAmount::usd_estimate(184_000),
            occurred_at_unix_ms: observed_at - 10_000,
            ingested_at_unix_ms: observed_at - 10_000,
            evidence: EvidenceKind::Inferred,
            source: source.into(),
            note: Some("Synthetic estimate, not provider billing evidence.".into()),
        })?;
        self.upsert_cost(&CostRecord {
            id: "demo-cost-unknown".into(),
            project_id: "demo-project-utu".into(),
            task_id: Some("demo-task-chat".into()),
            session_id: None,
            agent_id: Some("demo-agent-reviewer".into()),
            amount: CostAmount::unknown("USD").expect("USD is a valid currency"),
            occurred_at_unix_ms: observed_at,
            ingested_at_unix_ms: observed_at,
            evidence: EvidenceKind::Unsupported,
            source: source.into(),
            note: Some("This synthetic connector does not report cost.".into()),
        })?;
        self.upsert_attention(&AttentionRecord {
            id: "demo-attention-auth".into(),
            project_id: Some("demo-project-utu".into()),
            task_id: None,
            session_id: None,
            agent_id: None,
            integration_id: Some("demo-integration-cloud".into()),
            severity: Severity::Unknown,
            state: AttentionState::Open,
            title: "Cloud login cannot be confirmed (demo)".into(),
            detail: Some("Demonstration finding; no authentication probe ran.".into()),
            recovery: Some("Connect a supported provider and run an observed probe.".into()),
            detected_at_unix_ms: observed_at,
            updated_at_unix_ms: observed_at,
            evidence: EvidenceKind::Inferred,
            source: source.into(),
        })?;
        self.upsert_handoff(&AgentHandoff {
            id: "demo-handoff-review".into(),
            project_id: "demo-project-utu".into(),
            task_id: "demo-task-chat".into(),
            from_agent_id: "demo-agent-builder".into(),
            to_agent_id: "demo-agent-reviewer".into(),
            instruction: "Review the synthetic chat flow.".into(),
            created_at_unix_ms: observed_at,
            approved_by_owner: true,
            state: HandoffState::Approved,
            delivered_at_unix_ms: None,
            delivery_evidence: EvidenceKind::Inferred,
            source: source.into(),
            resulting_session_id: None,
            correlation_id: Some("demo-handoff".into()),
        })?;

        Ok(DemoSeedReport {
            project_id: "demo-project-utu".into(),
            providers: 2,
            agents: 2,
            tasks: 1,
            sessions: 1,
            messages: 2,
            events: 1,
        })
    }

    /// Explicit convenience for first-run previews. Existing projects leave
    /// the database untouched.
    pub fn seed_demo_if_empty(&self) -> Result<Option<DemoSeedReport>> {
        if self.list_projects()?.is_empty() {
            self.seed_demo().map(Some)
        } else {
            Ok(None)
        }
    }
}
