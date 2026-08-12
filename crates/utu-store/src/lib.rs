//! Durable, local-first persistence for Utu.
//!
//! The store contains normalized operational facts, never credentials. Event
//! and message streams are append-oriented; mutable projections use explicit
//! upsert methods. Every fact that can be uncertain retains its evidence kind
//! and source.

mod codec;
mod migrations;
mod repository;
mod seed;

use std::{
    path::Path,
    sync::{Mutex, MutexGuard},
    time::Duration,
};

use migrations::{LATEST_SCHEMA_VERSION, migrate};
use rusqlite::{Connection, OpenFlags};
use thiserror::Error;

pub use repository::{
    AttentionQuery, CostQuery, CostSummary, NewMessage, NewSessionEvent, ProjectCostProjection,
    RecordedControl, RecordedDirection, SearchQuery, SessionProjection, StreamQuery,
    WorkspaceProjection, WorkspaceScope,
};
pub use seed::DemoSeedReport;

pub type Result<T> = std::result::Result<T, StoreError>;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("SQLite error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("store connection mutex was poisoned")]
    LockPoisoned,
    #[error("database schema {found} is newer than supported schema {supported}")]
    NewerSchema { found: u32, supported: u32 },
    #[error("invalid {kind} database value `{value}`")]
    InvalidEnum { kind: &'static str, value: String },
    #[error("{field} value {value} cannot be represented by SQLite")]
    IntegerOverflow { field: &'static str, value: u64 },
    #[error("stored {field} value {value} was negative")]
    NegativeInteger { field: &'static str, value: i64 },
    #[error("invalid {entity}: {reason}")]
    InvalidRecord {
        entity: &'static str,
        reason: String,
    },
    #[error("{entity} `{id}` was not found")]
    NotFound { entity: &'static str, id: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreHealth {
    pub schema_version: u32,
    pub latest_supported_schema_version: u32,
    pub integrity_ok: bool,
    pub foreign_keys_enabled: bool,
}

/// A thread-safe handle around one SQLite connection. The short critical
/// sections keep transaction boundaries explicit and deterministic.
pub struct Store {
    connection: Mutex<Connection>,
}

pub(crate) fn invalid_seed(reason: impl Into<String>) -> StoreError {
    StoreError::InvalidRecord {
        entity: "demo seed",
        reason: reason.into(),
    }
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let mut connection = Connection::open_with_flags(path, flags)?;
        configure_connection(&connection, true)?;
        migrate(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        let mut connection = Connection::open_in_memory()?;
        configure_connection(&connection, false)?;
        migrate(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn schema_version(&self) -> Result<u32> {
        let connection = self.connection()?;
        Ok(connection.pragma_query_value(None, "user_version", |row| row.get(0))?)
    }

    pub const fn latest_supported_schema_version() -> u32 {
        LATEST_SCHEMA_VERSION
    }

    pub fn health(&self) -> Result<StoreHealth> {
        let connection = self.connection()?;
        let schema_version =
            connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        let foreign_keys: i64 =
            connection.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
        let integrity: String =
            connection.pragma_query_value(None, "quick_check", |row| row.get(0))?;
        Ok(StoreHealth {
            schema_version,
            latest_supported_schema_version: LATEST_SCHEMA_VERSION,
            integrity_ok: integrity == "ok",
            foreign_keys_enabled: foreign_keys == 1,
        })
    }

    pub(crate) fn connection(&self) -> Result<MutexGuard<'_, Connection>> {
        self.connection.lock().map_err(|_| StoreError::LockPoisoned)
    }
}

fn configure_connection(connection: &Connection, persistent: bool) -> Result<()> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    connection.pragma_update(None, "trusted_schema", "OFF")?;
    if persistent {
        let _: String = connection.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
    }
    Ok(())
}
