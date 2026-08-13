use std::f64::consts::PI;

use leptos::prelude::*;

use crate::{
    app::AppView,
    components::{AgentCliIcon, AppMarkGlyph, ICON_NODES, Icon},
    workspace_data::{
        LiveStatus, LoadPhase, WorkspaceAction, WorkspaceActionSink, session_state_tone,
    },
};

/// Orbital visualization of active and waiting agent sessions.
/// Nodes orbit the Utu mark. Clicking a node selects that session and navigates to Workspace.
#[component]
pub fn LiveOverviewView() -> impl IntoView {
    let live = expect_context::<LiveStatus>();
    let actions = expect_context::<WorkspaceActionSink>();
    let active_view = expect_context::<RwSignal<AppView>>();

    view! {
        <div class="overview-surface">
            <Show when=move || live.phase.get() == LoadPhase::Loading>
                <div class="overview-state">
                    <span class="spinner"></span>
                    <p>"Loading session data…"</p>
                </div>
            </Show>
            <Show when=move || live.phase.get() == LoadPhase::Error>
                <div class="overview-state"><p>"Could not load workspace data."</p></div>
            </Show>
            <Show when=move || matches!(live.phase.get(), LoadPhase::Empty | LoadPhase::Ready)>
                {move || {
                    let snapshot = live.snapshot.get();
                    let active_sessions: Vec<_> = snapshot.as_ref().map(|s| {
                        s.sessions.iter()
                            .filter(|session| {
                                matches!(session.state.as_str(), "running" | "waiting" | "problem")
                            })
                            .cloned()
                            .collect()
                    }).unwrap_or_default();

                    if active_sessions.is_empty() {
                        return view! {
                            <div class="overview-canvas overview-empty">
                                <div class="overview-center-glyph">
                                    <AppMarkGlyph />
                                </div>
                                <div class="overview-empty-label">
                                    <Icon path=ICON_NODES />
                                    <span>"No active agent sessions"</span>
                                    <small>"Running and waiting sessions appear here."</small>
                                </div>
                            </div>
                        }.into_any();
                    }

                    let total = active_sessions.len();
                    let radius_pct: f64 = if total <= 3 { 34.0 } else if total <= 6 { 37.0 } else { 40.0 };

                    let nodes: Vec<_> = active_sessions.into_iter().enumerate().map(|(i, session)| {
                        let session_id = session.id.clone();
                        let click_id = session_id.clone();
                        let state = session.state.clone();
                        let tone = session_state_tone(&state);
                        let connector_id = snapshot.as_ref().and_then(|s| {
                            s.agents.iter()
                                .find(|a| a.id == session.agent_id)
                                .map(|a| a.connector_id.clone())
                        }).unwrap_or_else(|| session.agent_id.clone());
                        let project_name = snapshot.as_ref().and_then(|s| {
                            s.projects.iter()
                                .find(|p| p.id == session.project_id)
                                .map(|p| p.name.clone())
                        }).unwrap_or_default();
                        let tooltip = format!("{project_name} · {state}");

                        let angle = (2.0 * PI * i as f64 / total as f64) - PI / 2.0;
                        let cx = 50.0 + radius_pct * angle.cos();
                        let cy = 50.0 + radius_pct * angle.sin();
                        let style = format!("left:{cx:.1}%;top:{cy:.1}%;transform:translate(-50%,-50%)");
                        let node_class = format!("overview-node overview-node-{tone}");

                        view! {
                            <button
                                class=node_class
                                style=style
                                title=tooltip
                                type="button"
                                on:click=move |_| {
                                    actions.dispatch(WorkspaceAction::SelectSession(click_id.clone()));
                                    active_view.set(AppView::Workspace);
                                }
                            >
                                <AgentCliIcon connector_id=connector_id size="sm" />
                                <span
                                    class=format!("overview-node-dot status-dot status-{tone}")
                                    aria-hidden="true"
                                ></span>
                                <Show when=move || live.selected_session_id.get().as_deref() == Some(session_id.as_str())>
                                    <span class="overview-node-ring"></span>
                                </Show>
                            </button>
                        }
                    }).collect();

                    let running_count = move || live.snapshot.get()
                        .map(|s| s.sessions.iter().filter(|ses| ses.state == "running").count())
                        .unwrap_or(0);
                    let waiting_count = move || live.snapshot.get()
                        .map(|s| s.sessions.iter().filter(|ses| ses.state == "waiting").count())
                        .unwrap_or(0);

                    view! {
                        <div class="overview-canvas">
                            <div class="overview-orbit-ring"></div>
                            <div class="overview-center-glyph overview-center-clickable">
                                <AppMarkGlyph />
                            </div>
                            {nodes}
                            <div class="overview-legend">
                                <span class="overview-legend-item">
                                    <span class="status-dot status-healthy" aria-hidden="true"></span>
                                    {running_count}" running"
                                </span>
                                <span class="overview-legend-item">
                                    <span class="status-dot status-attention" aria-hidden="true"></span>
                                    {waiting_count}" waiting"
                                </span>
                            </div>
                        </div>
                    }.into_any()
                }}
            </Show>
        </div>
    }
}
