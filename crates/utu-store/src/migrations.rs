use rusqlite::{Connection, TransactionBehavior};

use crate::{Result, StoreError};

pub(crate) const LATEST_SCHEMA_VERSION: u32 = 4;

const MIGRATIONS: &[(&str, &str)] = &[
    (
        "initial_local_store",
        r#"
        CREATE TABLE providers (
            id TEXT PRIMARY KEY NOT NULL,
            display_name TEXT NOT NULL,
            kind TEXT NOT NULL CHECK (kind IN ('local_cli', 'cloud_api', 'browser_mediated'))
        );

        CREATE TABLE integrations (
            id TEXT PRIMARY KEY NOT NULL,
            provider_id TEXT REFERENCES providers(id) ON DELETE SET NULL,
            connector_key TEXT NOT NULL,
            display_name TEXT NOT NULL,
            kind TEXT NOT NULL CHECK (kind IN ('local_cli', 'cloud_api', 'browser_mediated')),
            state TEXT NOT NULL CHECK (state IN ('ready', 'degraded', 'disabled', 'unknown')),
            auth TEXT NOT NULL CHECK (auth IN ('confirmed', 'expired', 'missing', 'unknown', 'unsupported')),
            evidence TEXT NOT NULL CHECK (evidence IN ('observed', 'inferred', 'stale', 'unsupported')),
            checked_at_unix_ms INTEGER,
            problem TEXT,
            can_observe INTEGER NOT NULL CHECK (can_observe IN (0, 1)),
            can_auth_probe INTEGER NOT NULL CHECK (can_auth_probe IN (0, 1)),
            can_direct INTEGER NOT NULL CHECK (can_direct IN (0, 1)),
            can_pause INTEGER NOT NULL CHECK (can_pause IN (0, 1)),
            can_resume INTEGER NOT NULL CHECK (can_resume IN (0, 1)),
            can_stop INTEGER NOT NULL CHECK (can_stop IN (0, 1)),
            can_logs INTEGER NOT NULL CHECK (can_logs IN (0, 1)),
            can_costs INTEGER NOT NULL CHECK (can_costs IN (0, 1)),
            can_agent_messages INTEGER NOT NULL CHECK (can_agent_messages IN (0, 1)),
            UNIQUE (connector_key, provider_id)
        );

        CREATE TABLE projects (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            root_path TEXT,
            state TEXT NOT NULL CHECK (state IN ('active', 'paused', 'archived')),
            created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0)
        );

        CREATE TABLE tasks (
            id TEXT PRIMARY KEY NOT NULL,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            detail TEXT NOT NULL,
            state TEXT NOT NULL CHECK (state IN ('draft', 'queued', 'running', 'waiting', 'blocked', 'completed', 'canceled')),
            created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
            updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= created_at_unix_ms)
        );

        CREATE TABLE agents (
            id TEXT PRIMARY KEY NOT NULL,
            provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE RESTRICT,
            connector_id TEXT NOT NULL REFERENCES integrations(id) ON DELETE RESTRICT,
            display_name TEXT NOT NULL,
            model TEXT,
            can_observe INTEGER NOT NULL CHECK (can_observe IN (0, 1)),
            can_auth_probe INTEGER NOT NULL CHECK (can_auth_probe IN (0, 1)),
            can_direct INTEGER NOT NULL CHECK (can_direct IN (0, 1)),
            can_pause INTEGER NOT NULL CHECK (can_pause IN (0, 1)),
            can_resume INTEGER NOT NULL CHECK (can_resume IN (0, 1)),
            can_stop INTEGER NOT NULL CHECK (can_stop IN (0, 1)),
            can_logs INTEGER NOT NULL CHECK (can_logs IN (0, 1)),
            can_costs INTEGER NOT NULL CHECK (can_costs IN (0, 1)),
            can_agent_messages INTEGER NOT NULL CHECK (can_agent_messages IN (0, 1))
        );

        CREATE TABLE task_assignees (
            task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE RESTRICT,
            PRIMARY KEY (task_id, agent_id)
        );

        CREATE TABLE sessions (
            id TEXT PRIMARY KEY NOT NULL,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
            agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE RESTRICT,
            provider_session_id TEXT,
            state TEXT NOT NULL CHECK (state IN ('running', 'waiting', 'idle', 'problem', 'offline')),
            started_at_unix_ms INTEGER NOT NULL CHECK (started_at_unix_ms >= 0),
            last_observed_at_unix_ms INTEGER,
            UNIQUE (agent_id, provider_session_id)
        );

        CREATE TABLE messages (
            id TEXT PRIMARY KEY NOT NULL,
            session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            sequence INTEGER NOT NULL CHECK (sequence >= 1),
            role TEXT NOT NULL CHECK (role IN ('owner', 'agent', 'system')),
            author_agent_id TEXT REFERENCES agents(id) ON DELETE SET NULL,
            body TEXT NOT NULL,
            sent_at_unix_ms INTEGER NOT NULL CHECK (sent_at_unix_ms >= 0),
            ingested_at_unix_ms INTEGER NOT NULL CHECK (ingested_at_unix_ms >= 0),
            evidence TEXT NOT NULL CHECK (evidence IN ('observed', 'inferred', 'stale', 'unsupported')),
            source TEXT NOT NULL,
            correlation_id TEXT,
            UNIQUE (session_id, sequence)
        );

        CREATE TABLE session_events (
            id TEXT PRIMARY KEY NOT NULL,
            session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            sequence INTEGER NOT NULL CHECK (sequence >= 1),
            occurred_at_unix_ms INTEGER NOT NULL CHECK (occurred_at_unix_ms >= 0),
            ingested_at_unix_ms INTEGER NOT NULL CHECK (ingested_at_unix_ms >= 0),
            kind TEXT NOT NULL CHECK (kind IN ('status', 'owner_message', 'agent_message', 'tool_call', 'file_change', 'cost', 'problem', 'handoff', 'log')),
            summary TEXT NOT NULL,
            detail TEXT,
            evidence TEXT NOT NULL CHECK (evidence IN ('observed', 'inferred', 'stale', 'unsupported')),
            source TEXT NOT NULL,
            provider_event_id TEXT,
            correlation_id TEXT,
            UNIQUE (session_id, sequence)
        );

        CREATE UNIQUE INDEX session_events_provider_dedupe
        ON session_events (session_id, source, provider_event_id)
        WHERE provider_event_id IS NOT NULL;

        CREATE TABLE file_changes (
            id TEXT PRIMARY KEY NOT NULL,
            session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            event_id TEXT REFERENCES session_events(id) ON DELETE SET NULL,
            path TEXT NOT NULL,
            previous_path TEXT,
            kind TEXT NOT NULL CHECK (kind IN ('added', 'modified', 'deleted', 'renamed')),
            additions INTEGER CHECK (additions IS NULL OR additions >= 0),
            deletions INTEGER CHECK (deletions IS NULL OR deletions >= 0),
            occurred_at_unix_ms INTEGER NOT NULL CHECK (occurred_at_unix_ms >= 0),
            evidence TEXT NOT NULL CHECK (evidence IN ('observed', 'inferred', 'stale', 'unsupported')),
            source TEXT NOT NULL
        );

        CREATE TABLE cost_records (
            id TEXT PRIMARY KEY NOT NULL,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
            session_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,
            agent_id TEXT REFERENCES agents(id) ON DELETE SET NULL,
            currency TEXT NOT NULL CHECK (length(currency) = 3 AND currency = upper(currency)),
            amount_micros INTEGER CHECK (amount_micros IS NULL OR amount_micros >= 0),
            confidence TEXT NOT NULL CHECK (confidence IN ('exact', 'estimated', 'partial', 'unknown')),
            occurred_at_unix_ms INTEGER NOT NULL CHECK (occurred_at_unix_ms >= 0),
            ingested_at_unix_ms INTEGER NOT NULL CHECK (ingested_at_unix_ms >= 0),
            evidence TEXT NOT NULL CHECK (evidence IN ('observed', 'inferred', 'stale', 'unsupported')),
            source TEXT NOT NULL,
            note TEXT,
            CHECK ((confidence = 'unknown' AND amount_micros IS NULL) OR
                   (confidence <> 'unknown' AND amount_micros IS NOT NULL))
        );

        CREATE TABLE attention_findings (
            id TEXT PRIMARY KEY NOT NULL,
            project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
            task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
            session_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,
            agent_id TEXT REFERENCES agents(id) ON DELETE SET NULL,
            integration_id TEXT REFERENCES integrations(id) ON DELETE SET NULL,
            severity TEXT NOT NULL CHECK (severity IN ('blocked', 'needs_attention', 'healthy', 'unknown')),
            state TEXT NOT NULL CHECK (state IN ('open', 'acknowledged', 'resolved')),
            title TEXT NOT NULL,
            detail TEXT,
            recovery TEXT,
            detected_at_unix_ms INTEGER NOT NULL CHECK (detected_at_unix_ms >= 0),
            updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= detected_at_unix_ms),
            evidence TEXT NOT NULL CHECK (evidence IN ('observed', 'inferred', 'stale', 'unsupported')),
            source TEXT NOT NULL
        );

        CREATE TABLE handoffs (
            id TEXT PRIMARY KEY NOT NULL,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            from_agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE RESTRICT,
            to_agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE RESTRICT,
            instruction TEXT NOT NULL,
            created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
            approved_by_owner INTEGER NOT NULL CHECK (approved_by_owner IN (0, 1)),
            state TEXT NOT NULL CHECK (state IN ('requested', 'approved', 'delivered', 'failed', 'canceled', 'unknown')),
            delivered_at_unix_ms INTEGER,
            delivery_evidence TEXT NOT NULL CHECK (delivery_evidence IN ('observed', 'inferred', 'stale', 'unsupported')),
            source TEXT NOT NULL,
            resulting_session_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,
            correlation_id TEXT,
            CHECK (from_agent_id <> to_agent_id)
        );

        CREATE TABLE control_requests (
            id TEXT PRIMARY KEY NOT NULL,
            session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            action TEXT NOT NULL CHECK (action IN ('direct', 'pause', 'resume', 'stop')),
            instruction TEXT,
            requested_at_unix_ms INTEGER NOT NULL CHECK (requested_at_unix_ms >= 0),
            requested_by_owner INTEGER NOT NULL CHECK (requested_by_owner IN (0, 1))
        );

        CREATE TABLE control_receipts (
            id TEXT PRIMARY KEY NOT NULL,
            request_id TEXT NOT NULL REFERENCES control_requests(id) ON DELETE CASCADE,
            outcome TEXT NOT NULL CHECK (outcome IN ('acknowledged', 'rejected', 'timed_out', 'unsupported', 'unknown')),
            received_at_unix_ms INTEGER NOT NULL CHECK (received_at_unix_ms >= 0),
            evidence TEXT NOT NULL CHECK (evidence IN ('observed', 'inferred', 'stale', 'unsupported')),
            source TEXT NOT NULL,
            message TEXT,
            provider_receipt_id TEXT,
            UNIQUE (request_id, provider_receipt_id)
        );
        "#,
    ),
    (
        "query_indexes",
        r#"
        CREATE INDEX projects_state_name ON projects (state, name COLLATE NOCASE);
        CREATE INDEX tasks_project_state_updated ON tasks (project_id, state, updated_at_unix_ms DESC);
        CREATE INDEX task_assignees_agent ON task_assignees (agent_id, task_id);
        CREATE INDEX sessions_project_started ON sessions (project_id, started_at_unix_ms DESC);
        CREATE INDEX sessions_agent_observed ON sessions (agent_id, last_observed_at_unix_ms DESC);
        CREATE INDEX messages_session_order ON messages (session_id, sequence);
        CREATE INDEX events_session_order ON session_events (session_id, sequence);
        CREATE INDEX events_occurred ON session_events (occurred_at_unix_ms DESC);
        CREATE INDEX files_session_occurred ON file_changes (session_id, occurred_at_unix_ms DESC);
        CREATE INDEX costs_project_occurred ON cost_records (project_id, occurred_at_unix_ms DESC);
        CREATE INDEX attention_state_severity ON attention_findings (state, severity, updated_at_unix_ms DESC);
        CREATE INDEX handoffs_project_created ON handoffs (project_id, created_at_unix_ms DESC);
        CREATE INDEX control_requests_session_created ON control_requests (session_id, requested_at_unix_ms DESC);
        CREATE INDEX control_receipts_request_received ON control_receipts (request_id, received_at_unix_ms DESC);
        "#,
    ),
    (
        "message_author_integrity",
        r#"
        CREATE TRIGGER messages_agent_author_insert
        BEFORE INSERT ON messages
        WHEN NEW.role = 'agent'
        BEGIN
            SELECT CASE WHEN NEW.author_agent_id IS NULL
                          OR NEW.author_agent_id <> (SELECT agent_id FROM sessions WHERE id = NEW.session_id)
                        THEN RAISE(ABORT, 'agent message author must match session agent') END;
        END;

        CREATE TRIGGER messages_agent_author_update
        BEFORE UPDATE OF session_id, role, author_agent_id ON messages
        WHEN NEW.role = 'agent'
        BEGIN
            SELECT CASE WHEN NEW.author_agent_id IS NULL
                          OR NEW.author_agent_id <> (SELECT agent_id FROM sessions WHERE id = NEW.session_id)
                        THEN RAISE(ABORT, 'agent message author must match session agent') END;
        END;

        CREATE TRIGGER sessions_agent_author_update
        BEFORE UPDATE OF agent_id ON sessions
        WHEN NEW.agent_id <> OLD.agent_id
        BEGIN
            SELECT CASE WHEN EXISTS(
                SELECT 1 FROM messages
                WHERE session_id = OLD.id AND role = 'agent' AND author_agent_id <> NEW.agent_id
            ) THEN RAISE(ABORT, 'session agent cannot invalidate authored messages') END;
        END;

        CREATE TRIGGER agents_authored_message_delete
        BEFORE DELETE ON agents
        BEGIN
            SELECT CASE WHEN EXISTS(
                SELECT 1 FROM messages WHERE author_agent_id = OLD.id
            ) THEN RAISE(ABORT, 'authored message agents cannot be deleted') END;
        END;
        "#,
    ),
    (
        "session_title_hint",
        r#"
        ALTER TABLE sessions ADD COLUMN title_hint TEXT;
        "#,
    ),
];

pub(crate) fn migrate(connection: &mut Connection) -> Result<()> {
    let current: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if current > LATEST_SCHEMA_VERSION {
        return Err(StoreError::NewerSchema {
            found: current,
            supported: LATEST_SCHEMA_VERSION,
        });
    }

    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS utu_schema_migrations (\
            version INTEGER PRIMARY KEY NOT NULL, \
            name TEXT NOT NULL UNIQUE\
         );",
    )?;

    for (offset, (name, sql)) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let version = (offset + 1) as u32;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(sql)?;
        transaction.execute(
            "INSERT INTO utu_schema_migrations (version, name) VALUES (?1, ?2)",
            (version, name),
        )?;
        transaction.pragma_update(None, "user_version", version)?;
        transaction.commit()?;
    }

    Ok(())
}

#[cfg(test)]
pub(crate) fn migration_sql(version: u32) -> Option<&'static str> {
    MIGRATIONS
        .get(version.checked_sub(1)? as usize)
        .map(|(_, sql)| *sql)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrades_a_version_one_database_without_replaying_initial_schema() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON; \
                 CREATE TABLE utu_schema_migrations (version INTEGER PRIMARY KEY NOT NULL, name TEXT NOT NULL UNIQUE);",
            )
            .unwrap();
        connection.execute_batch(migration_sql(1).unwrap()).unwrap();
        connection
            .execute(
                "INSERT INTO utu_schema_migrations (version, name) VALUES (1, 'initial_local_store')",
                [],
            )
            .unwrap();
        connection.pragma_update(None, "user_version", 1).unwrap();

        migrate(&mut connection).unwrap();

        let version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        let indexes: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE type = 'index' AND name = 'events_session_order'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, LATEST_SCHEMA_VERSION);
        assert_eq!(indexes, 1);
        let triggers: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE type = 'trigger' \
                 AND name IN ('messages_agent_author_insert', 'messages_agent_author_update', \
                              'sessions_agent_author_update', 'agents_authored_message_delete')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(triggers, 4);
    }
}
