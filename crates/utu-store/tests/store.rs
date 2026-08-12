use tempfile::tempdir;
use utu_core::{
    Agent, AgentHandoff, AgentState, AuthState, ConnectorCapabilities, ControlAction,
    ControlOutcome, ControlReceipt, ControlRequest, CostAmount, CostConfidence, CostRecord,
    EventKind, EvidenceKind, FileChange, FileChangeKind, HandoffState, Integration,
    IntegrationState, MessageRole, Project, ProjectState, Provider, ProviderKind, SearchEntityKind,
    Session, Task, TaskState,
};
use utu_store::{CostQuery, NewMessage, NewSessionEvent, SearchQuery, Store, StreamQuery};

const CAPS: ConnectorCapabilities = ConnectorCapabilities {
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

fn populate_dependencies(store: &Store) {
    store
        .upsert_provider(&Provider {
            id: "provider".into(),
            display_name: "Provider".into(),
            kind: ProviderKind::LocalCli,
        })
        .unwrap();
    store
        .upsert_integration(&Integration {
            id: "integration".into(),
            provider_id: Some("provider".into()),
            connector_key: "test.connector".into(),
            display_name: "Test connector".into(),
            kind: ProviderKind::LocalCli,
            state: IntegrationState::Ready,
            auth: AuthState::Confirmed,
            evidence: EvidenceKind::Observed,
            checked_at_unix_ms: Some(10),
            problem: None,
            capabilities: CAPS,
        })
        .unwrap();
    for id in ["agent-a", "agent-b", "agent-c"] {
        store
            .upsert_agent(&Agent {
                id: id.into(),
                provider_id: "provider".into(),
                connector_id: "integration".into(),
                display_name: id.into(),
                model: None,
                capabilities: CAPS,
            })
            .unwrap();
    }
    store
        .upsert_project(&Project {
            id: "project".into(),
            name: "Persistent project".into(),
            root_path: Some("/tmp/project".into()),
            state: ProjectState::Active,
            created_at_unix_ms: 1,
        })
        .unwrap();
    store
        .upsert_task(&Task {
            id: "task".into(),
            project_id: "project".into(),
            title: "Build durable chat".into(),
            detail: "Find every elusive persistence edge case".into(),
            state: TaskState::Running,
            assignee_agent_ids: vec!["agent-a".into()],
            created_at_unix_ms: 2,
            updated_at_unix_ms: 3,
        })
        .unwrap();
    store
        .upsert_session(&Session {
            id: "session".into(),
            project_id: "project".into(),
            task_id: Some("task".into()),
            agent_id: "agent-a".into(),
            provider_session_id: Some("provider-session".into()),
            state: AgentState::Running,
            started_at_unix_ms: 4,
            last_observed_at_unix_ms: Some(5),
        })
        .unwrap();
}

fn populate_second_scope(store: &Store) {
    store
        .upsert_project(&Project {
            id: "other-project".into(),
            name: "Other project".into(),
            root_path: Some("/tmp/other-project".into()),
            state: ProjectState::Active,
            created_at_unix_ms: 20,
        })
        .unwrap();
    store
        .upsert_task(&Task {
            id: "other-task".into(),
            project_id: "other-project".into(),
            title: "Other task".into(),
            detail: String::new(),
            state: TaskState::Running,
            assignee_agent_ids: vec!["agent-b".into()],
            created_at_unix_ms: 21,
            updated_at_unix_ms: 22,
        })
        .unwrap();
    store
        .upsert_session(&Session {
            id: "other-session".into(),
            project_id: "other-project".into(),
            task_id: Some("other-task".into()),
            agent_id: "agent-b".into(),
            provider_session_id: Some("other-provider-session".into()),
            state: AgentState::Running,
            started_at_unix_ms: 23,
            last_observed_at_unix_ms: Some(24),
        })
        .unwrap();
}

#[test]
fn migrations_are_current_and_idempotent() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("utu.sqlite3");
    let store = Store::open(&path).unwrap();
    let first = store.health().unwrap();
    assert_eq!(
        first.schema_version,
        Store::latest_supported_schema_version()
    );
    assert!(first.integrity_ok);
    assert!(first.foreign_keys_enabled);
    drop(store);

    let reopened = Store::open(path).unwrap();
    assert_eq!(reopened.health().unwrap(), first);
}

#[test]
fn newer_schema_is_refused_without_mutation() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("future.sqlite3");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 99).unwrap();
    drop(connection);
    let error = Store::open(&path).err().unwrap().to_string();
    assert!(error.contains("newer than supported"));
    let connection = rusqlite::Connection::open(path).unwrap();
    let migration_tables: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name = 'utu_schema_migrations'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(migration_tables, 0);
}

#[test]
fn project_task_session_and_streams_persist_across_reopen() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("reopen.sqlite3");
    {
        let store = Store::open(&path).unwrap();
        populate_dependencies(&store);
        store
            .append_message(NewMessage {
                id: "message".into(),
                session_id: "session".into(),
                role: MessageRole::Owner,
                author_agent_id: None,
                body: "persist this conversation".into(),
                sent_at_unix_ms: 6,
                ingested_at_unix_ms: 7,
                evidence: EvidenceKind::Observed,
                source: "owner".into(),
                correlation_id: None,
            })
            .unwrap();
    }
    let reopened = Store::open(&path).unwrap();
    assert_eq!(
        reopened.get_project("project").unwrap().unwrap().name,
        "Persistent project"
    );
    assert_eq!(
        reopened
            .get_task("task")
            .unwrap()
            .unwrap()
            .assignee_agent_ids,
        ["agent-a"]
    );
    assert_eq!(
        reopened
            .get_session("session")
            .unwrap()
            .unwrap()
            .provider_session_id
            .as_deref(),
        Some("provider-session")
    );
    assert_eq!(
        reopened
            .list_messages("session", StreamQuery::default())
            .unwrap()[0]
            .body,
        "persist this conversation"
    );
}

#[test]
fn task_assignment_is_replace_all_sorted_and_atomic() {
    let store = Store::open_in_memory().unwrap();
    populate_dependencies(&store);
    let task = store
        .assign_task_agents(
            "task",
            &["agent-c".into(), "agent-b".into(), "agent-b".into()],
            20,
        )
        .unwrap();
    assert_eq!(task.assignee_agent_ids, ["agent-b", "agent-c"]);

    let error = store
        .assign_task_agents("task", &["missing-agent".into()], 21)
        .unwrap_err();
    assert!(error.to_string().contains("FOREIGN KEY"));
    assert_eq!(
        store.get_task("task").unwrap().unwrap().assignee_agent_ids,
        ["agent-b", "agent-c"]
    );
    assert_eq!(
        store.get_task("task").unwrap().unwrap().updated_at_unix_ms,
        20
    );
}

#[test]
fn messages_and_events_use_local_sequence_not_wall_clock_order() {
    let store = Store::open_in_memory().unwrap();
    populate_dependencies(&store);
    for (id, sent_at) in [("late", 900), ("early", 100)] {
        store
            .append_message(NewMessage {
                id: id.into(),
                session_id: "session".into(),
                role: MessageRole::Owner,
                author_agent_id: None,
                body: id.into(),
                sent_at_unix_ms: sent_at,
                ingested_at_unix_ms: sent_at,
                evidence: EvidenceKind::Observed,
                source: "owner".into(),
                correlation_id: None,
            })
            .unwrap();
    }
    assert_eq!(
        store
            .list_messages("session", StreamQuery::default())
            .unwrap()
            .iter()
            .map(|m| (&*m.id, m.sequence))
            .collect::<Vec<_>>(),
        [("late", 1), ("early", 2)]
    );

    for (id, occurred_at, provider_id) in [("e1", 900, "p1"), ("e2", 100, "p2")] {
        store
            .append_event(NewSessionEvent {
                id: id.into(),
                session_id: "session".into(),
                occurred_at_unix_ms: occurred_at,
                ingested_at_unix_ms: occurred_at,
                kind: utu_core::EventKind::Log,
                summary: id.into(),
                detail: None,
                evidence: EvidenceKind::Observed,
                source: "connector".into(),
                provider_event_id: Some(provider_id.into()),
                correlation_id: None,
            })
            .unwrap();
    }
    let replay = store
        .append_event(NewSessionEvent {
            id: "different-local-id".into(),
            session_id: "session".into(),
            occurred_at_unix_ms: 1,
            ingested_at_unix_ms: 1,
            kind: utu_core::EventKind::Log,
            summary: "replayed".into(),
            detail: None,
            evidence: EvidenceKind::Observed,
            source: "connector".into(),
            provider_event_id: Some("p1".into()),
            correlation_id: None,
        })
        .unwrap();
    assert_eq!(replay.id, "e1");
    assert_eq!(
        store
            .list_events("session", StreamQuery::default())
            .unwrap()
            .iter()
            .map(|e| (&*e.id, e.sequence))
            .collect::<Vec<_>>(),
        [("e1", 1), ("e2", 2)]
    );
}

#[test]
fn agent_message_author_must_match_session_agent_and_is_delete_protected() {
    let store = Store::open_in_memory().unwrap();
    populate_dependencies(&store);
    let hostile = NewMessage {
        id: "spoofed-agent-message".into(),
        session_id: "session".into(),
        role: MessageRole::Agent,
        author_agent_id: Some("agent-b".into()),
        body: "I did work in another agent's session".into(),
        sent_at_unix_ms: 80,
        ingested_at_unix_ms: 80,
        evidence: EvidenceKind::Observed,
        source: "hostile-test".into(),
        correlation_id: None,
    };
    assert!(store.append_message(hostile).is_err());
    assert!(
        store
            .get_message("spoofed-agent-message")
            .unwrap()
            .is_none()
    );

    let hostile_import = utu_core::Message {
        id: "spoofed-import".into(),
        session_id: "session".into(),
        sequence: 1,
        role: MessageRole::Agent,
        author_agent_id: Some("agent-b".into()),
        body: "Imported spoof".into(),
        sent_at_unix_ms: 81,
        ingested_at_unix_ms: 81,
        evidence: EvidenceKind::Observed,
        source: "hostile-test".into(),
        correlation_id: None,
    };
    assert!(store.insert_message(&hostile_import).is_err());

    let valid = store
        .append_message(NewMessage {
            id: "valid-agent-message".into(),
            session_id: "session".into(),
            role: MessageRole::Agent,
            author_agent_id: Some("agent-a".into()),
            body: "Valid authored message".into(),
            sent_at_unix_ms: 82,
            ingested_at_unix_ms: 82,
            evidence: EvidenceKind::Observed,
            source: "test".into(),
            correlation_id: None,
        })
        .unwrap();
    assert_eq!(valid.author_agent_id.as_deref(), Some("agent-a"));

    let mut moved = store.get_session("session").unwrap().unwrap();
    moved.agent_id = "agent-b".into();
    assert!(store.upsert_session(&moved).is_err());
    assert_eq!(
        store.get_session("session").unwrap().unwrap().agent_id,
        "agent-a"
    );
    assert!(store.delete_agent("agent-a").is_err());
    assert!(store.get_agent("agent-a").unwrap().is_some());
    assert_eq!(
        store
            .get_message("valid-agent-message")
            .unwrap()
            .unwrap()
            .author_agent_id
            .as_deref(),
        Some("agent-a")
    );
}

#[test]
fn search_spans_tasks_messages_and_events_with_literal_wildcards() {
    let store = Store::open_in_memory().unwrap();
    populate_dependencies(&store);
    store
        .append_message(NewMessage {
            id: "search-message".into(),
            session_id: "session".into(),
            role: MessageRole::Owner,
            author_agent_id: None,
            body: "Need elusive token 100%_literal".into(),
            sent_at_unix_ms: 40,
            ingested_at_unix_ms: 40,
            evidence: EvidenceKind::Observed,
            source: "owner".into(),
            correlation_id: None,
        })
        .unwrap();
    store
        .append_event(NewSessionEvent {
            id: "search-event".into(),
            session_id: "session".into(),
            occurred_at_unix_ms: 41,
            ingested_at_unix_ms: 41,
            kind: utu_core::EventKind::Problem,
            summary: "Elusive connector failure".into(),
            detail: Some("recovery trace".into()),
            evidence: EvidenceKind::Observed,
            source: "connector".into(),
            provider_event_id: None,
            correlation_id: None,
        })
        .unwrap();
    let hits = store.search(&SearchQuery::new("elusive")).unwrap();
    assert!(hits.iter().any(|hit| hit.entity == SearchEntityKind::Task));
    assert!(
        hits.iter()
            .any(|hit| hit.entity == SearchEntityKind::Message)
    );
    assert!(hits.iter().any(|hit| hit.entity == SearchEntityKind::Event));
    let literal = store.search(&SearchQuery::new("100%_literal")).unwrap();
    assert_eq!(literal.len(), 1);
    assert_eq!(literal[0].id, "search-message");
}

#[test]
fn unknown_cost_is_not_zero_and_makes_summary_partial() {
    let store = Store::open_in_memory().unwrap();
    populate_dependencies(&store);
    let exact = CostRecord {
        id: "cost-known".into(),
        project_id: "project".into(),
        task_id: Some("task".into()),
        session_id: Some("session".into()),
        agent_id: Some("agent-a".into()),
        amount: CostAmount::usd_exact(1_500_000),
        occurred_at_unix_ms: 10,
        ingested_at_unix_ms: 10,
        evidence: EvidenceKind::Observed,
        source: "billing-api".into(),
        note: None,
    };
    store.upsert_cost(&exact).unwrap();
    store
        .upsert_cost(&CostRecord {
            id: "cost-unknown".into(),
            amount: CostAmount::unknown("USD").unwrap(),
            evidence: EvidenceKind::Unsupported,
            source: "local-cli".into(),
            ..exact
        })
        .unwrap();
    let summary = store.cost_summary("project", "usd").unwrap();
    assert_eq!(summary.known_micros, 1_500_000);
    assert_eq!(summary.unknown_records, 1);
    assert_eq!(summary.confidence, CostConfidence::Partial);
    assert!(!summary.is_complete());
    assert_eq!(summary.amount().display(), "~$1.50");
    assert_eq!(store.list_costs(&CostQuery::default()).unwrap().len(), 2);

    assert!(CostAmount::new("USD", Some(0), CostConfidence::Unknown).is_err());
    let rejected = CostRecord {
        id: "cost-invalid-unknown".into(),
        project_id: "project".into(),
        task_id: None,
        session_id: None,
        agent_id: None,
        amount: CostAmount {
            currency: "USD".into(),
            micros: Some(0),
            confidence: CostConfidence::Unknown,
        },
        occurred_at_unix_ms: 11,
        ingested_at_unix_ms: 11,
        evidence: EvidenceKind::Unsupported,
        source: "unsupported-connector".into(),
        note: None,
    };
    assert!(store.upsert_cost(&rejected).is_err());
    assert!(store.get_cost("cost-invalid-unknown").unwrap().is_none());
}

#[test]
fn confirmed_auth_requires_observation_not_inference() {
    let store = Store::open_in_memory().unwrap();
    store
        .upsert_provider(&Provider {
            id: "provider".into(),
            display_name: "Provider".into(),
            kind: ProviderKind::CloudApi,
        })
        .unwrap();
    let inferred_confirmation = Integration {
        id: "untrustworthy".into(),
        provider_id: Some("provider".into()),
        connector_key: "inferred.auth".into(),
        display_name: "Inferred auth".into(),
        kind: ProviderKind::CloudApi,
        state: IntegrationState::Ready,
        auth: AuthState::Confirmed,
        evidence: EvidenceKind::Inferred,
        checked_at_unix_ms: Some(10),
        problem: None,
        capabilities: CAPS,
    };
    assert!(store.upsert_integration(&inferred_confirmation).is_err());
    assert!(store.get_integration("untrustworthy").unwrap().is_none());

    store
        .upsert_integration(&Integration {
            id: "unsupported".into(),
            auth: AuthState::Unsupported,
            evidence: EvidenceKind::Unsupported,
            state: IntegrationState::Unknown,
            ..inferred_confirmation
        })
        .unwrap();
    let stored = store.get_integration("unsupported").unwrap().unwrap();
    assert_eq!(stored.auth, AuthState::Unsupported);
    assert_eq!(stored.evidence, EvidenceKind::Unsupported);
}

#[test]
fn ready_integration_requires_confirmed_authentication() {
    let store = Store::open_in_memory().unwrap();
    store
        .upsert_provider(&Provider {
            id: "provider".into(),
            display_name: "Provider".into(),
            kind: ProviderKind::CloudApi,
        })
        .unwrap();
    for auth in [
        AuthState::Missing,
        AuthState::Expired,
        AuthState::Unknown,
        AuthState::Unsupported,
    ] {
        let id = format!("ready-{auth:?}").to_ascii_lowercase();
        let integration = Integration {
            id: id.clone(),
            provider_id: Some("provider".into()),
            connector_key: format!("test.{id}"),
            display_name: format!("Ready with {auth:?}"),
            kind: ProviderKind::CloudApi,
            state: IntegrationState::Ready,
            auth,
            evidence: EvidenceKind::Observed,
            checked_at_unix_ms: Some(10),
            problem: None,
            capabilities: CAPS,
        };
        assert!(
            store.upsert_integration(&integration).is_err(),
            "Ready unexpectedly accepted {auth:?} authentication"
        );
        assert!(store.get_integration(&id).unwrap().is_none());
    }
}

#[test]
fn cross_scope_references_and_parent_mutations_are_rejected_atomically() {
    let store = Store::open_in_memory().unwrap();
    populate_dependencies(&store);
    populate_second_scope(&store);

    let invalid_session = Session {
        id: "cross-session".into(),
        project_id: "project".into(),
        task_id: Some("other-task".into()),
        agent_id: "agent-a".into(),
        provider_session_id: None,
        state: AgentState::Running,
        started_at_unix_ms: 30,
        last_observed_at_unix_ms: None,
    };
    assert!(store.upsert_session(&invalid_session).is_err());
    assert!(store.get_session("cross-session").unwrap().is_none());

    let mut moved_task = store.get_task("task").unwrap().unwrap();
    moved_task.project_id = "other-project".into();
    moved_task.updated_at_unix_ms += 1;
    assert!(store.upsert_task(&moved_task).is_err());
    assert_eq!(
        store.get_task("task").unwrap().unwrap().project_id,
        "project"
    );

    let other_event = store
        .append_event(NewSessionEvent {
            id: "other-event".into(),
            session_id: "other-session".into(),
            occurred_at_unix_ms: 31,
            ingested_at_unix_ms: 31,
            kind: EventKind::FileChange,
            summary: "Other project changed".into(),
            detail: None,
            evidence: EvidenceKind::Observed,
            source: "test".into(),
            provider_event_id: None,
            correlation_id: None,
        })
        .unwrap();
    let cross_file = FileChange {
        id: "cross-file".into(),
        session_id: "session".into(),
        event_id: Some(other_event.id),
        path: "src/private.rs".into(),
        previous_path: None,
        kind: FileChangeKind::Modified,
        additions: Some(1),
        deletions: Some(0),
        occurred_at_unix_ms: 31,
        evidence: EvidenceKind::Observed,
        source: "test".into(),
    };
    assert!(store.upsert_file_change(&cross_file).is_err());
    assert!(store.get_file_change("cross-file").unwrap().is_none());

    let valid_cost = CostRecord {
        id: "scoped-cost".into(),
        project_id: "project".into(),
        task_id: Some("task".into()),
        session_id: Some("session".into()),
        agent_id: Some("agent-a".into()),
        amount: CostAmount::usd_exact(1),
        occurred_at_unix_ms: 32,
        ingested_at_unix_ms: 32,
        evidence: EvidenceKind::Observed,
        source: "test".into(),
        note: None,
    };
    store.upsert_cost(&valid_cost).unwrap();
    for invalid_cost in [
        CostRecord {
            id: "cross-cost-task".into(),
            task_id: Some("other-task".into()),
            session_id: None,
            agent_id: None,
            ..valid_cost.clone()
        },
        CostRecord {
            id: "cross-cost-session".into(),
            session_id: Some("other-session".into()),
            ..valid_cost.clone()
        },
        CostRecord {
            id: "cross-cost-agent".into(),
            agent_id: Some("agent-b".into()),
            ..valid_cost.clone()
        },
    ] {
        let id = invalid_cost.id.clone();
        assert!(store.upsert_cost(&invalid_cost).is_err());
        assert!(store.get_cost(&id).unwrap().is_none());
    }

    let mut invalid_session_update = store.get_session("session").unwrap().unwrap();
    invalid_session_update.agent_id = "agent-b".into();
    assert!(store.upsert_session(&invalid_session_update).is_err());
    assert_eq!(
        store.get_session("session").unwrap().unwrap().agent_id,
        "agent-a"
    );

    let cross_handoff = AgentHandoff {
        id: "cross-handoff".into(),
        project_id: "project".into(),
        task_id: "task".into(),
        from_agent_id: "agent-a".into(),
        to_agent_id: "agent-b".into(),
        instruction: "Continue safely".into(),
        created_at_unix_ms: 33,
        approved_by_owner: true,
        state: HandoffState::Approved,
        delivered_at_unix_ms: None,
        delivery_evidence: EvidenceKind::Inferred,
        source: "test".into(),
        resulting_session_id: Some("other-session".into()),
        correlation_id: None,
    };
    assert!(store.upsert_handoff(&cross_handoff).is_err());
    assert!(store.get_handoff("cross-handoff").unwrap().is_none());
}

#[test]
fn agent_cannot_elevate_or_cross_its_integration_boundary() {
    let store = Store::open_in_memory().unwrap();
    populate_dependencies(&store);
    let restricted = ConnectorCapabilities {
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
    store
        .upsert_integration(&Integration {
            id: "restricted-integration".into(),
            provider_id: Some("provider".into()),
            connector_key: "restricted.connector".into(),
            display_name: "Restricted connector".into(),
            kind: ProviderKind::LocalCli,
            state: IntegrationState::Unknown,
            auth: AuthState::Unknown,
            evidence: EvidenceKind::Observed,
            checked_at_unix_ms: Some(40),
            problem: None,
            capabilities: restricted,
        })
        .unwrap();
    let elevated = Agent {
        id: "elevated-agent".into(),
        provider_id: "provider".into(),
        connector_id: "restricted-integration".into(),
        display_name: "Elevated".into(),
        model: None,
        capabilities: CAPS,
    };
    assert!(store.upsert_agent(&elevated).is_err());
    assert!(store.get_agent("elevated-agent").unwrap().is_none());

    store
        .upsert_provider(&Provider {
            id: "other-provider".into(),
            display_name: "Other provider".into(),
            kind: ProviderKind::CloudApi,
        })
        .unwrap();
    let cross_provider = Agent {
        id: "cross-provider-agent".into(),
        provider_id: "other-provider".into(),
        capabilities: restricted,
        ..elevated
    };
    assert!(store.upsert_agent(&cross_provider).is_err());
    assert!(store.get_agent("cross-provider-agent").unwrap().is_none());

    let mut lowered = store.get_integration("integration").unwrap().unwrap();
    lowered.capabilities.direct = false;
    assert!(store.upsert_integration(&lowered).is_err());
    assert!(
        store
            .get_integration("integration")
            .unwrap()
            .unwrap()
            .capabilities
            .direct
    );

    let changed_provider = Provider {
        id: "provider".into(),
        display_name: "Changed provider".into(),
        kind: ProviderKind::CloudApi,
    };
    assert!(store.upsert_provider(&changed_provider).is_err());
    let provider = store.get_provider("provider").unwrap().unwrap();
    assert_eq!(provider.kind, ProviderKind::LocalCli);
    assert_eq!(provider.display_name, "Provider");
}

#[test]
fn owner_direction_message_request_and_receipt_commit_as_one_record() {
    let store = Store::open_in_memory().unwrap();
    populate_dependencies(&store);
    let message = NewMessage {
        id: "direction-message".into(),
        session_id: "session".into(),
        role: MessageRole::Owner,
        author_agent_id: None,
        body: "Run the focused tests".into(),
        sent_at_unix_ms: 50,
        ingested_at_unix_ms: 50,
        evidence: EvidenceKind::Observed,
        source: "utu.owner".into(),
        correlation_id: None,
    };
    let request = ControlRequest {
        id: "direction-request".into(),
        session_id: "session".into(),
        action: ControlAction::Direct,
        instruction: Some("Run the focused tests".into()),
        requested_at_unix_ms: 50,
        requested_by_owner: true,
    };
    let receipt = ControlReceipt {
        id: "direction-receipt".into(),
        request_id: "direction-request".into(),
        outcome: ControlOutcome::Unsupported,
        received_at_unix_ms: 50,
        evidence: EvidenceKind::Unsupported,
        source: "test.connector".into(),
        message: Some("No live transport".into()),
        provider_receipt_id: None,
    };
    let recorded = store
        .record_owner_direction(message, request.clone(), receipt.clone())
        .unwrap();
    assert_eq!(recorded.request, request);
    assert_eq!(recorded.receipt, receipt);
    assert_eq!(recorded.message.sequence, 1);
    assert!(store.get_message("direction-message").unwrap().is_some());
    assert_eq!(
        store.get_control_request("direction-request").unwrap(),
        Some(request)
    );

    store
        .upsert_control_request(&ControlRequest {
            id: "collision".into(),
            session_id: "session".into(),
            action: ControlAction::Pause,
            instruction: None,
            requested_at_unix_ms: 51,
            requested_by_owner: true,
        })
        .unwrap();
    let rollback = store.record_owner_direction(
        NewMessage {
            id: "rolled-back-message".into(),
            session_id: "session".into(),
            role: MessageRole::Owner,
            author_agent_id: None,
            body: "Do not partially commit".into(),
            sent_at_unix_ms: 52,
            ingested_at_unix_ms: 52,
            evidence: EvidenceKind::Observed,
            source: "utu.owner".into(),
            correlation_id: None,
        },
        ControlRequest {
            id: "collision".into(),
            session_id: "session".into(),
            action: ControlAction::Direct,
            instruction: Some("Do not partially commit".into()),
            requested_at_unix_ms: 52,
            requested_by_owner: true,
        },
        ControlReceipt {
            id: "rolled-back-receipt".into(),
            request_id: "collision".into(),
            outcome: ControlOutcome::Unsupported,
            received_at_unix_ms: 52,
            evidence: EvidenceKind::Unsupported,
            source: "test.connector".into(),
            message: None,
            provider_receipt_id: None,
        },
    );
    assert!(rollback.is_err());
    assert!(store.get_message("rolled-back-message").unwrap().is_none());
    assert!(
        store
            .get_control_receipt("rolled-back-receipt")
            .unwrap()
            .is_none()
    );
}

#[test]
fn control_request_and_initial_receipt_are_atomic() {
    let store = Store::open_in_memory().unwrap();
    populate_dependencies(&store);
    store
        .upsert_control_receipt(&ControlReceipt {
            id: "receipt-collision".into(),
            request_id: "existing-request".into(),
            outcome: ControlOutcome::Unsupported,
            received_at_unix_ms: 60,
            evidence: EvidenceKind::Unsupported,
            source: "test".into(),
            message: None,
            provider_receipt_id: None,
        })
        .expect_err("foreign key rejects missing request");

    let request = ControlRequest {
        id: "atomic-control".into(),
        session_id: "session".into(),
        action: ControlAction::Pause,
        instruction: None,
        requested_at_unix_ms: 61,
        requested_by_owner: true,
    };
    let invalid_receipt = ControlReceipt {
        id: "atomic-receipt".into(),
        request_id: "different-request".into(),
        outcome: ControlOutcome::Unsupported,
        received_at_unix_ms: 61,
        evidence: EvidenceKind::Unsupported,
        source: "test".into(),
        message: None,
        provider_receipt_id: None,
    };
    assert!(
        store
            .record_control(request.clone(), invalid_receipt)
            .is_err()
    );
    assert!(
        store
            .get_control_request("atomic-control")
            .unwrap()
            .is_none()
    );

    let receipt = ControlReceipt {
        id: "atomic-receipt".into(),
        request_id: request.id.clone(),
        outcome: ControlOutcome::Unsupported,
        received_at_unix_ms: 61,
        evidence: EvidenceKind::Unsupported,
        source: "test".into(),
        message: None,
        provider_receipt_id: None,
    };
    let recorded = store
        .record_control(request.clone(), receipt.clone())
        .unwrap();
    assert_eq!(recorded.request, request);
    assert_eq!(recorded.receipt, receipt);
}

#[test]
fn attention_scope_is_normalized_and_cross_scope_links_are_rejected() {
    let store = Store::open_in_memory().unwrap();
    populate_dependencies(&store);
    populate_second_scope(&store);
    let session_only = utu_core::AttentionRecord {
        id: "session-attention".into(),
        project_id: None,
        task_id: None,
        session_id: Some("session".into()),
        agent_id: None,
        integration_id: None,
        severity: utu_core::Severity::NeedsAttention,
        state: utu_core::AttentionState::Open,
        title: "Session needs review".into(),
        detail: None,
        recovery: None,
        detected_at_unix_ms: 70,
        updated_at_unix_ms: 70,
        evidence: EvidenceKind::Observed,
        source: "test".into(),
    };
    store.upsert_attention(&session_only).unwrap();
    let normalized = store.get_attention("session-attention").unwrap().unwrap();
    assert_eq!(normalized.project_id.as_deref(), Some("project"));
    assert_eq!(normalized.task_id.as_deref(), Some("task"));
    assert_eq!(normalized.agent_id.as_deref(), Some("agent-a"));
    store
        .upsert_session(&store.get_session("session").unwrap().unwrap())
        .unwrap();
    store
        .upsert_task(&store.get_task("task").unwrap().unwrap())
        .unwrap();
    let project = store
        .read_workspace_projection(utu_store::WorkspaceScope::Project("project".into()), "USD")
        .unwrap();
    assert!(
        project
            .attention
            .iter()
            .any(|item| item.id == "session-attention")
    );

    let cross_scope = utu_core::AttentionRecord {
        id: "cross-attention".into(),
        project_id: Some("other-project".into()),
        ..session_only.clone()
    };
    assert!(store.upsert_attention(&cross_scope).is_err());
    assert!(store.get_attention("cross-attention").unwrap().is_none());

    let integration_mismatch = utu_core::AttentionRecord {
        id: "integration-mismatch".into(),
        project_id: Some("project".into()),
        task_id: Some("task".into()),
        session_id: None,
        agent_id: Some("agent-a".into()),
        integration_id: Some("missing-integration".into()),
        ..session_only
    };
    assert!(store.upsert_attention(&integration_mismatch).is_err());
}

#[test]
fn demo_seed_is_explicit_labeled_and_refuses_nonempty_workspace() {
    let store = Store::open_in_memory().unwrap();
    assert!(store.list_projects().unwrap().is_empty());
    let report = store.seed_demo().unwrap();
    assert_eq!(report.project_id, "demo-project-utu");
    assert!(
        store.list_projects().unwrap()[0]
            .name
            .contains("demonstration")
    );
    assert!(
        store
            .seed_demo()
            .unwrap_err()
            .to_string()
            .contains("no projects")
    );
}
