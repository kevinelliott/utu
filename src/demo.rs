use utu_core::{
    AgentSnapshot, AgentState, AuthState, ControlCapabilities, EvidenceKind, IsolationMode,
};

pub fn codex_snapshot() -> AgentSnapshot {
    AgentSnapshot {
        id: "demo-codex".into(),
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

#[cfg(test)]
mod tests {
    use utu_core::{Severity, assess_agent};

    use super::*;

    #[test]
    fn demonstration_agent_is_healthy_but_never_live_evidence() {
        let finding = assess_agent(&codex_snapshot());
        assert_eq!(finding.severity, Severity::Healthy);
    }
}
