use std::{fs, path::Path, sync::Arc};

use utu_store::Store;

use crate::codex_runtime::CodexRuntime;

/// Native application authority shared by Tauri commands.
///
/// `Store` serializes its own short SQLite critical sections. Wrapping it in an
/// `Arc` lets commands clone the handle before moving blocking work to Tauri's
/// blocking executor instead of holding a webview-thread borrow.
#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Store>,
    pub codex: Arc<CodexRuntime>,
}

impl AppState {
    pub fn open(data_directory: impl AsRef<Path>) -> Result<Self, String> {
        let data_directory = data_directory.as_ref();
        fs::create_dir_all(data_directory).map_err(|error| {
            format!(
                "could not create Utu data directory {}: {error}",
                data_directory.display()
            )
        })?;
        tighten_data_directory_permissions(data_directory)?;
        let database_path = data_directory.join("utu.sqlite3");
        let store = Store::open(&database_path)
            .map_err(|error| format!("could not open the Utu local store: {error}"))?;
        deactivate_volatile_codex_transport(&store)?;
        tighten_database_permissions(&database_path)?;
        Ok(Self {
            store: Arc::new(store),
            codex: Arc::new(CodexRuntime::default()),
        })
    }
}

fn deactivate_volatile_codex_transport(store: &Store) -> Result<(), String> {
    if let Some(mut agent) = store
        .get_agent("codex-app-server")
        .map_err(|error| format!("could not inspect Codex runtime agent: {error}"))?
    {
        agent.capabilities = utu_core::ConnectorCapabilities::default();
        store
            .upsert_agent(&agent)
            .map_err(|error| format!("could not deactivate Codex runtime agent: {error}"))?;
    }
    if let Some(mut integration) = store
        .get_integration("codex-app-server")
        .map_err(|error| format!("could not inspect Codex runtime integration: {error}"))?
    {
        integration.state = utu_core::IntegrationState::Unknown;
        integration.auth = utu_core::AuthState::Unknown;
        integration.evidence = utu_core::EvidenceKind::Stale;
        integration.capabilities = utu_core::ConnectorCapabilities::default();
        integration.problem = Some(
            "The Codex App Server runtime is disconnected; synchronize an explicit project to reactivate it."
                .into(),
        );
        store
            .upsert_integration(&integration)
            .map_err(|error| format!("could not deactivate Codex runtime integration: {error}"))?;
    }
    Ok(())
}

#[cfg(unix)]
fn tighten_data_directory_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("could not secure Utu data directory: {error}"))
}

#[cfg(not(unix))]
fn tighten_data_directory_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn tighten_database_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("could not secure Utu database: {error}"))
}

#[cfg(not(unix))]
fn tighten_database_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use utu_core::{
        Agent, AuthState, ConnectorCapabilities, EvidenceKind, Integration, IntegrationState,
        Provider, ProviderKind,
    };

    struct Fixture(PathBuf);

    use std::path::PathBuf;

    impl Fixture {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            Self(std::env::temp_dir().join(format!("utu-app-state-{}-{nonce}", std::process::id())))
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn open_creates_a_healthy_local_store() {
        let fixture = Fixture::new();
        let state = AppState::open(&fixture.0).expect("state");
        let health = state.store.health().expect("health");
        assert!(health.integrity_ok);
        assert!(health.foreign_keys_enabled);
        assert!(fixture.0.join("utu.sqlite3").is_file());
    }

    #[test]
    fn restart_deactivates_a_persisted_codex_runtime_without_failing_open() {
        let fixture = Fixture::new();
        {
            let state = AppState::open(&fixture.0).expect("initial state");
            state
                .store
                .upsert_provider(&Provider {
                    id: "codex".into(),
                    display_name: "Codex".into(),
                    kind: ProviderKind::LocalCli,
                })
                .expect("provider");
            let capabilities = ConnectorCapabilities {
                observe: true,
                auth_probe: true,
                direct: true,
                agent_messages: true,
                ..ConnectorCapabilities::default()
            };
            state
                .store
                .upsert_integration(&Integration {
                    id: "codex-app-server".into(),
                    provider_id: Some("codex".into()),
                    connector_key: "codex-app-server".into(),
                    display_name: "Codex App Server".into(),
                    kind: ProviderKind::LocalCli,
                    state: IntegrationState::Ready,
                    auth: AuthState::Confirmed,
                    evidence: EvidenceKind::Observed,
                    checked_at_unix_ms: Some(1),
                    problem: None,
                    capabilities,
                })
                .expect("active integration");
            state
                .store
                .upsert_agent(&Agent {
                    id: "codex-app-server".into(),
                    provider_id: "codex".into(),
                    connector_id: "codex-app-server".into(),
                    display_name: "Codex App Server".into(),
                    model: None,
                    capabilities,
                })
                .expect("active agent");
        }

        let reopened = AppState::open(&fixture.0).expect("restart must remain available");
        let integration = reopened
            .store
            .get_integration("codex-app-server")
            .expect("integration read")
            .expect("integration");
        let agent = reopened
            .store
            .get_agent("codex-app-server")
            .expect("agent read")
            .expect("agent");
        assert_eq!(integration.state, IntegrationState::Unknown);
        assert_eq!(integration.auth, AuthState::Unknown);
        assert_eq!(integration.evidence, EvidenceKind::Stale);
        assert!(!integration.capabilities.direct);
        assert!(!agent.capabilities.direct);
    }

    #[cfg(unix)]
    #[test]
    fn local_store_permissions_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = Fixture::new();
        let _state = AppState::open(&fixture.0).expect("state");
        let directory_mode = fs::metadata(&fixture.0)
            .expect("directory metadata")
            .permissions()
            .mode()
            & 0o777;
        let database_mode = fs::metadata(fixture.0.join("utu.sqlite3"))
            .expect("database metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(directory_mode, 0o700);
        assert_eq!(database_mode, 0o600);
    }
}
