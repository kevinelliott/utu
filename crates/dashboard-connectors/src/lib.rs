use std::{env, path::PathBuf};

use serde::Serialize;
use utu_core::{AuthState, EvidenceKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CliDefinition {
    pub id: &'static str,
    pub display_name: &'static str,
    pub executable: &'static str,
}

pub const KNOWN_LOCAL_CLIS: [CliDefinition; 5] = [
    CliDefinition {
        id: "codex",
        display_name: "Codex",
        executable: "codex",
    },
    CliDefinition {
        id: "claude",
        display_name: "Claude Code",
        executable: "claude",
    },
    CliDefinition {
        id: "cursor",
        display_name: "Cursor Agent",
        executable: "cursor-agent",
    },
    CliDefinition {
        id: "antigravity",
        display_name: "Antigravity",
        executable: "antigravity",
    },
    CliDefinition {
        id: "grok",
        display_name: "Grok Build",
        executable: "grok",
    },
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalCliProbe {
    pub id: &'static str,
    pub display_name: &'static str,
    pub executable: &'static str,
    pub installed_path: Option<PathBuf>,
    /// Binary discovery is not authentication evidence.
    pub auth_state: AuthState,
    pub install_evidence: EvidenceKind,
}

pub trait ExecutableLookup {
    fn find(&self, executable: &str) -> Option<PathBuf>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EnvironmentPath;

impl ExecutableLookup for EnvironmentPath {
    fn find(&self, executable: &str) -> Option<PathBuf> {
        let paths = env::var_os("PATH")?;
        env::split_paths(&paths)
            .map(|dir| dir.join(executable))
            .find(|candidate| candidate.is_file())
    }
}

pub fn probe_known_local_clis(lookup: &impl ExecutableLookup) -> Vec<LocalCliProbe> {
    KNOWN_LOCAL_CLIS
        .iter()
        .map(|definition| {
            let installed_path = lookup.find(definition.executable);
            LocalCliProbe {
                id: definition.id,
                display_name: definition.display_name,
                executable: definition.executable,
                install_evidence: if installed_path.is_some() {
                    EvidenceKind::Observed
                } else {
                    EvidenceKind::Inferred
                },
                installed_path,
                auth_state: AuthState::Unknown,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeLookup;

    impl ExecutableLookup for FakeLookup {
        fn find(&self, executable: &str) -> Option<PathBuf> {
            (executable == "codex").then(|| PathBuf::from("/mock/bin/codex"))
        }
    }

    #[test]
    fn path_discovery_never_claims_authentication() {
        let probes = probe_known_local_clis(&FakeLookup);
        assert!(
            probes
                .iter()
                .all(|probe| probe.auth_state == AuthState::Unknown)
        );
        assert_eq!(probes[0].install_evidence, EvidenceKind::Observed);
        assert_eq!(probes[1].install_evidence, EvidenceKind::Inferred);
    }
}
