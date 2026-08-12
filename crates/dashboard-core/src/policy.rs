use crate::{AgentSnapshot, AgentState, AttentionFinding, AuthState, EvidenceKind, Severity};

/// Derives owner attention without overstating weak connector evidence.
pub fn assess_agent(snapshot: &AgentSnapshot) -> AttentionFinding {
    if matches!(snapshot.auth, AuthState::Expired | AuthState::Missing) {
        return AttentionFinding {
            severity: Severity::Blocked,
            title: format!("{} needs authentication", snapshot.name),
            recovery: Some("Reconnect the account and run the connector check again.".into()),
        };
    }

    if snapshot.state == AgentState::Problem {
        return AttentionFinding {
            severity: Severity::Blocked,
            title: format!("{} reported a runtime problem", snapshot.name),
            recovery: Some("Open the session evidence before restarting the agent.".into()),
        };
    }

    if snapshot.state == AgentState::Offline {
        return AttentionFinding {
            severity: Severity::NeedsAttention,
            title: format!("{} is offline", snapshot.name),
            recovery: Some("Check the connector and start or reconnect the agent.".into()),
        };
    }

    if snapshot.evidence != EvidenceKind::Observed
        || matches!(snapshot.auth, AuthState::Unknown | AuthState::Unsupported)
    {
        return AttentionFinding {
            severity: Severity::Unknown,
            title: format!("{} status cannot be confirmed", snapshot.name),
            recovery: Some("Inspect connector support and the last observed event.".into()),
        };
    }

    if snapshot.state == AgentState::Waiting {
        return AttentionFinding {
            severity: Severity::NeedsAttention,
            title: format!("{} is waiting for you", snapshot.name),
            recovery: Some("Review the pending decision or send direction.".into()),
        };
    }

    AttentionFinding {
        severity: Severity::Healthy,
        title: format!("{} is {}", snapshot.name, state_label(snapshot.state)),
        recovery: None,
    }
}

const fn state_label(state: AgentState) -> &'static str {
    match state {
        AgentState::Running => "running",
        AgentState::Waiting => "waiting",
        AgentState::Idle => "idle",
        AgentState::Problem => "unhealthy",
        AgentState::Offline => "offline",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ControlCapabilities, IsolationMode};

    #[test]
    fn missing_truth_defaults_fail_closed() {
        assert_eq!(EvidenceKind::default(), EvidenceKind::Inferred);
        assert_eq!(Severity::default(), Severity::Unknown);
    }
    fn snapshot() -> AgentSnapshot {
        AgentSnapshot {
            id: "codex-1".into(),
            name: "Codex".into(),
            provider: "OpenAI".into(),
            project: Some("Utu".into()),
            state: AgentState::Running,
            auth: AuthState::Confirmed,
            evidence: EvidenceKind::Observed,
            evidence_age_seconds: Some(4),
            isolation: IsolationMode::LocalVm,
            controls: ControlCapabilities::FULL,
        }
    }

    #[test]
    fn expired_auth_blocks_even_when_process_is_running() {
        let finding = assess_agent(&AgentSnapshot {
            auth: AuthState::Expired,
            ..snapshot()
        });
        assert_eq!(finding.severity, Severity::Blocked);
        assert!(finding.title.contains("authentication"));
    }

    #[test]
    fn unsupported_evidence_is_unknown_not_healthy() {
        let finding = assess_agent(&AgentSnapshot {
            evidence: EvidenceKind::Unsupported,
            ..snapshot()
        });
        assert_eq!(finding.severity, Severity::Unknown);
    }

    #[test]
    fn inferred_or_default_evidence_is_unknown_not_healthy() {
        for evidence in [EvidenceKind::Inferred, EvidenceKind::default()] {
            let finding = assess_agent(&AgentSnapshot {
                evidence,
                ..snapshot()
            });
            assert_eq!(finding.severity, Severity::Unknown);
        }
    }

    #[test]
    fn unknown_or_unsupported_auth_is_unknown_not_healthy() {
        for auth in [AuthState::Unknown, AuthState::Unsupported] {
            let finding = assess_agent(&AgentSnapshot { auth, ..snapshot() });
            assert_eq!(finding.severity, Severity::Unknown);
        }
    }

    #[test]
    fn waiting_is_owner_attention() {
        let finding = assess_agent(&AgentSnapshot {
            state: AgentState::Waiting,
            ..snapshot()
        });
        assert_eq!(finding.severity, Severity::NeedsAttention);
    }

    #[test]
    fn offline_is_never_healthy() {
        let finding = assess_agent(&AgentSnapshot {
            state: AgentState::Offline,
            ..snapshot()
        });
        assert_eq!(finding.severity, Severity::NeedsAttention);
    }

    #[test]
    fn estimated_cost_is_labeled() {
        assert_eq!(
            crate::CostAmount::usd_estimate(1_420_000).display(),
            "~$1.42"
        );
    }

    #[test]
    fn cost_display_does_not_hide_or_understate_subcent_values() {
        assert_eq!(crate::CostAmount::usd_exact(1).display(), "<$0.01");
        assert_eq!(crate::CostAmount::usd_estimate(9_999).display(), "~<$0.01");
        assert_eq!(crate::CostAmount::usd_exact(14_999).display(), "$0.01");
        assert_eq!(crate::CostAmount::usd_exact(15_000).display(), "$0.02");
    }
}
