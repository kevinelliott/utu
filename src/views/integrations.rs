use leptos::prelude::*;

use crate::{
    components::{
        EvidenceTag, ICON_CHECK, ICON_CLOSE, ICON_CLOUD, ICON_COMMAND, ICON_LOCK, ICON_MORE,
        ICON_PLUG, ICON_REFRESH, ICON_SHIELD, ICON_TERMINAL, Icon, StatusDot, WorkspaceNav,
    },
    workspace_data::{
        ConnectorSummary, LiveStatus, WorkspaceAction, WorkspaceActionSink, WorkspaceModel,
    },
};

#[component]
pub fn IntegrationsView(
    inspector_open: RwSignal<bool>,
    read_only: bool,
    notice: RwSignal<Option<String>>,
) -> impl IntoView {
    let model = expect_context::<WorkspaceModel>();
    let actions = expect_context::<WorkspaceActionSink>();
    let live = expect_context::<LiveStatus>();
    let refresh_all = move |_| {
        actions.dispatch(WorkspaceAction::RefreshConnector("all connectors".into()));
    };

    view! {
        <div class="workspace-layout integrations-layout">
            <header class="workspace-toolbar integrations-toolbar">
                <div class="toolbar-leading">
                    <WorkspaceNav />
                    <div><h1>"Integrations"</h1><p>"Readiness, authentication, and supported controls"</p></div>
                </div>
                <div class="toolbar-actions">
                    <button class="primary-outline" type="button" disabled=move || read_only || live.phase.get() == crate::workspace_data::LoadPhase::Error || live.selected_project_id.get().is_none() || live.codex_syncing.get() on:click=move |_| {
                        if let Some(project_id) = live.selected_project_id.get_untracked() {
                            actions.dispatch(WorkspaceAction::SyncCodexProject(project_id));
                        }
                    }>{move || if live.codex_syncing.get() { "Syncing Codex metadata…" } else { "Sync Codex for project" }}</button>
                    <button class="secondary-button" type="button" disabled=move || read_only || live.phase.get() == crate::workspace_data::LoadPhase::Error on:click=refresh_all><Icon path=ICON_REFRESH />"Refresh checks"</button>
                    <button class="icon-button" type="button" aria-label="Integration actions"><Icon path=ICON_MORE /></button>
                </div>
            </header>

            <div class=move || if live.is_desktop() { "truth-banner live-truth-banner" } else { "truth-banner" } role="note">
                <Icon path=ICON_SHIELD />
                <span><strong>{move || if live.is_desktop() { "Local diagnostics" } else { "Demonstration inventory" }}</strong>{move || if live.is_desktop() { "Utu runs bounded executable, version, and supported authentication probes. Codex metadata sync is opt-in, selected-project scoped, and never imports transcripts." } else { "These rows show intended connector states. Utu has not contacted a CLI, provider, account, or browser session." }}</span>
            </div>

            <div class="integration-list" class:is-checking=move || live.connector_refreshing.get()>
                <Show when=move || live.connector_refreshing.get()>
                    <div class="integration-checking" role="status"><span class="spinner"></span><span><strong>"Checking local connectors"</strong><small>"Bounded probes run outside the UI thread"</small></span></div>
                </Show>

                <Show when=move || live.diagnostics.get().is_some() fallback=move || view! { <div class="integration-live-empty"><span class=if live.is_desktop() { "spinner" } else { "status-dot status-quiet" }></span><strong>{move || if live.is_desktop() { "Loading connector diagnostics" } else { "No live diagnostics on the web surface" }}</strong><small>{move || live.error.get().unwrap_or_else(|| "Waiting for the local connector service.".into())}</small></div> }>
                    <section class="connector-group live-connector-group">
                        <header><span><h2>"Local agent CLIs"</h2><p>"Observed executable, version, authentication, and recovery evidence"</p></span><span class="connector-group-count">{move || live.diagnostics.get().map(|report| report.connectors.len()).unwrap_or_default()}</span></header>
                        <div class="connector-rows">
                            {move || live.diagnostics.get().map(|report| report.connectors.iter().enumerate().map(|(index, connector)| {
                                let tone = diagnostic_tone(&connector.readiness);
                                let detail = diagnostic_detail(connector);
                                let version = connector.version.value.clone().unwrap_or_else(|| connector.version.status.clone());
                                let name = connector.descriptor.display_name.clone();
                                let connector_id = connector.descriptor.id.clone();
                                let inspect_connector_id = connector_id.clone();
                                view! { <article class=format!("connector-row connector-{tone}")><button class="connector-main" type="button" on:click=move |_| { inspector_open.set(true); actions.dispatch(WorkspaceAction::ConfigureConnector(inspect_connector_id.clone())); }><span class="connector-icon"><Icon path=ICON_TERMINAL /></span><span class="connector-identity"><strong>{name}<span class="live-label">"Live"</span></strong><small>{detail}</small></span><span class=format!("connector-status state-label {tone}")><StatusDot tone />{readiness_label(&connector.readiness)}</span><span class="connector-evidence">{connector.installation.kind.clone()}</span></button><div class="connector-capabilities"><span>{version}</span><span>{connector.auth.state.clone()}</span></div><button class="secondary-button connector-action" type="button" disabled=move || live.phase.get() == crate::workspace_data::LoadPhase::Error on:click=move |_| actions.dispatch(WorkspaceAction::RefreshConnector(connector_id.clone()))>{if index == 0 { "Run check" } else { "Recheck" }}</button></article> }
                            }).collect_view())}
                        </div>
                    </section>
                </Show>

                <Show when=move || !live.is_desktop()>
                    <ConnectorGroup title="Local agent CLIs" detail="Executable, version, account, session, and capability checks" family="Local CLI" model inspector_open read_only />
                    <ConnectorGroup title="Cloud workspaces" detail="Provider APIs first; permissioned browser mediation only when required" family="Cloud" model inspector_open read_only />
                </Show>

                <section class="integration-empty" aria-label="Custom connectors empty state">
                    <span class="empty-icon"><Icon path=ICON_PLUG /></span>
                    <span><strong>"No custom connectors yet"</strong><small>"A connector SDK will let local tools join the same capability and evidence model."</small></span>
                    <button class="secondary-button" type="button" disabled=read_only on:click=move |_| notice.set(Some("Custom connector setup is planned; no SDK is wired in this prototype.".into()))>"Learn about connectors"</button>
                </section>
            </div>
        </div>
    }
}

fn diagnostic_tone(readiness: &str) -> &'static str {
    match readiness {
        "ready" => "healthy",
        "installed_unverified" | "needs_attention" => "attention",
        _ => "problem",
    }
}

fn readiness_label(readiness: &str) -> &'static str {
    match readiness {
        "ready" => "Ready",
        "installed_unverified" => "Installed · unverified",
        "needs_attention" => "Needs attention",
        _ => "Unavailable",
    }
}

fn diagnostic_detail(connector: &crate::ipc::ConnectorDiagnostic) -> String {
    connector
        .problems
        .first()
        .map(|problem| problem.summary.clone())
        .or_else(|| connector.auth.detail.clone())
        .or_else(|| connector.installation.detail.clone())
        .unwrap_or_else(|| "No actionable problem reported.".into())
}

#[component]
fn ConnectorGroup(
    title: &'static str,
    detail: &'static str,
    family: &'static str,
    model: WorkspaceModel,
    inspector_open: RwSignal<bool>,
    read_only: bool,
) -> impl IntoView {
    view! {
        <section class="connector-group">
            <header><span><h2>{title}</h2><p>{detail}</p></span><span class="connector-group-count">{model.connectors.iter().filter(|connector| connector.family == family).count()}</span></header>
            <div class="connector-rows">
                {model.connectors.iter().copied().filter(move |connector| connector.family == family).map(|connector| view! {
                    <ConnectorRow connector inspector_open read_only />
                }).collect_view()}
            </div>
        </section>
    }
}

#[component]
fn ConnectorRow(
    connector: ConnectorSummary,
    inspector_open: RwSignal<bool>,
    read_only: bool,
) -> impl IntoView {
    let actions = expect_context::<WorkspaceActionSink>();
    let inspect_actions = actions;
    let button_actions = actions;
    let action_label = match connector.tone {
        "healthy" => "Run check",
        "attention" => "Sign in",
        "problem" => "Locate CLI",
        _ => "View plan",
    };

    view! {
        <article class=format!("connector-row connector-{}", connector.tone)>
            <button class="connector-main" type="button" on:click=move |_| { inspector_open.set(true); inspect_actions.dispatch(WorkspaceAction::ConfigureConnector(connector.id.into())); }>
                <span class="connector-icon"><Icon path=if connector.family == "Cloud" { ICON_CLOUD } else { ICON_TERMINAL } /></span>
                <span class="connector-identity"><strong>{connector.name}<span class="demo-label">"Demo"</span></strong><small>{connector.detail}</small></span>
                <span class=format!("connector-status state-label {}", connector.tone)><StatusDot tone=connector.tone />{connector.status}</span>
                <span class="connector-evidence">{connector.evidence}</span>
            </button>
            <div class="connector-capabilities" aria-label=format!("{} demonstration capabilities", connector.name)>
                {connector.capabilities.iter().map(|capability| view! { <span>{*capability}</span> }).collect_view()}
            </div>
            <button class="secondary-button connector-action" type="button" disabled=read_only on:click=move |_| {
                if connector.tone == "healthy" {
                    button_actions.dispatch(WorkspaceAction::RefreshConnector(connector.id.into()));
                } else {
                    button_actions.dispatch(WorkspaceAction::ConfigureConnector(connector.id.into()));
                }
            }>{action_label}</button>
        </article>
    }
}

#[component]
pub fn IntegrationsInspector(
    inspector_open: RwSignal<bool>,
    read_only: bool,
    notice: RwSignal<Option<String>>,
) -> impl IntoView {
    let model = expect_context::<WorkspaceModel>();
    let connector = model.connectors[0];
    let actions = expect_context::<WorkspaceActionSink>();
    let refresh_actions = actions;
    let configure_actions = actions;

    view! {
        <div class="inspector-content integration-inspector">
            <header class="inspector-header simple-inspector-header">
                <div><h2>{connector.name}</h2><p>"Demonstration connector detail"</p></div>
                <button class="icon-button" type="button" aria-label="Close connector detail" on:click=move |_| inspector_open.set(false)><Icon path=ICON_CLOSE /></button>
            </header>

            <section class="inspector-section connector-readiness">
                <div class="inspector-section-title"><h3>"Readiness"</h3><span class="state-label healthy"><StatusDot />"Demo ready"</span></div>
                <p>"This is the intended healthy layout, not a live check result."</p>
                <button class="primary-outline" type="button" disabled=read_only on:click=move |_| refresh_actions.dispatch(WorkspaceAction::RefreshConnector(connector.id.into()))><Icon path=ICON_REFRESH />"Run demo check"</button>
            </section>

            <section class="inspector-section">
                <h3>"Authentication"</h3>
                <dl class="detail-list">
                    <div><dt>"Account"</dt><dd>"Not queried"</dd></div>
                    <div><dt>"Credential storage"</dt><dd><Icon path=ICON_LOCK />"OS keychain planned"</dd></div>
                    <div><dt>"Evidence"</dt><dd><EvidenceTag kind="Unknown" /></dd></div>
                </dl>
            </section>

            <section class="inspector-section capabilities-list">
                <h3>"Represented capabilities"</h3>
                {connector.capabilities.iter().map(|capability| view! { <span><Icon path=ICON_CHECK />{*capability}</span> }).collect_view()}
            </section>

            <section class="inspector-section">
                <h3>"Isolation defaults"</h3>
                <div class="surface-row"><Icon path=ICON_SHIELD /><span><strong>"Workspace sandbox"</strong><small>"Confirm write, command, and network scopes"</small></span></div>
            </section>

            <section class="inspector-section stacked-controls">
                <h3>"Connector actions"</h3>
                <button type="button" disabled=read_only on:click=move |_| configure_actions.dispatch(WorkspaceAction::ConfigureConnector(connector.id.into()))><Icon path=ICON_COMMAND />"Configure demo"</button>
                <button type="button" disabled=read_only on:click=move |_| notice.set(Some("No live connector logs exist. This prototype only contains demonstration events.".into()))><Icon path=ICON_TERMINAL />"View logs"</button>
            </section>
        </div>
    }
}

#[component]
pub fn LiveIntegrationsInspector(inspector_open: RwSignal<bool>) -> impl IntoView {
    let live = expect_context::<LiveStatus>();
    let actions = expect_context::<WorkspaceActionSink>();

    view! {
        <div class="inspector-content integration-inspector live-integration-inspector">
            <header class="inspector-header simple-inspector-header">
                <div><h2>"Connector diagnostics"</h2><p>"Observed on this owner device"</p></div>
                <button class="icon-button" type="button" aria-label="Close connector detail" on:click=move |_| inspector_open.set(false)><Icon path=ICON_CLOSE /></button>
            </header>
            <Show when=move || live.diagnostics.get().is_some() fallback=move || view! { <div class="integration-live-empty"><span class="spinner"></span><strong>"Waiting for diagnostics"</strong><small>"Utu is running bounded local checks."</small></div> }>
                {move || live.diagnostics.get().and_then(|report| {
                    let connector = live.selected_connector_id.get().as_deref().and_then(|id| report.connectors.iter().find(|connector| connector.descriptor.id == id)).or_else(|| report.connectors.first())?;
                    let id = connector.descriptor.id.clone();
                    let name = connector.descriptor.display_name.clone();
                    let executable = connector.descriptor.executable.clone();
                    let readiness = connector.readiness.clone();
                    let health = connector.health.clone();
                    let version = connector.version.value.clone().unwrap_or_else(|| connector.version.status.clone());
                    let auth = connector.auth.state.clone();
                    let install_detail = connector.installation.detail.clone().unwrap_or_else(|| connector.installation.status.clone());
                    let problems = StoredValue::new(connector.problems.iter().map(|problem| (problem.severity.clone(), problem.summary.clone(), problem.recovery.clone())).collect::<Vec<_>>());
                    Some(view! {
                        <section class="inspector-section connector-readiness">
                            <div class="inspector-section-title"><h3>{name}</h3><span class=format!("state-label {}", diagnostic_tone(&readiness))><StatusDot tone=diagnostic_tone(&readiness) />{readiness_label(&readiness)}</span></div>
                            <p>{install_detail}</p>
                            <button class="primary-outline" type="button" disabled=move || live.phase.get() == crate::workspace_data::LoadPhase::Error on:click=move |_| actions.dispatch(WorkspaceAction::RefreshConnector(id.clone()))><Icon path=ICON_REFRESH />"Run checks again"</button>
                        </section>
                        <section class="inspector-section"><h3>"Evidence"</h3><dl class="detail-list"><div><dt>"Executable"</dt><dd><code>{executable}</code></dd></div><div><dt>"Version"</dt><dd>{version}</dd></div><div><dt>"Authentication"</dt><dd>{auth}</dd></div><div><dt>"Health"</dt><dd>{health}</dd></div></dl></section>
                        <section class="inspector-section"><h3>"Problems and recovery"</h3><Show when=move || !problems.get_value().is_empty() fallback=move || view! { <p class="inspector-note">"No actionable problem was reported by this adapter."</p> }>{move || problems.get_value().into_iter().map(|(severity, summary, recovery)| view! { <div class="diagnostic-problem"><StatusDot tone=if severity == "error" { "problem" } else { "attention" } /><span><strong>{summary}</strong><small>{recovery.unwrap_or_else(|| "No automated recovery is available.".into())}</small></span></div> }).collect_view()}</Show></section>
                    })
                })}
            </Show>
        </div>
    }
}
