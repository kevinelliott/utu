use leptos::prelude::*;

use crate::{
    components::{
        AgentAvatar, AppMarkGlyph, DemoBadge, ICON_COMMAND, ICON_COST, ICON_HOME, ICON_NODES,
        ICON_PLUG, ICON_SEARCH, ICON_SETTINGS, Icon, StatusDot, ViewSwitch,
    },
    views::{
        attention::{AttentionInspector, AttentionView},
        fleet::{FleetInspector, FleetView},
        projects::{ProjectInspector, ProjectsView},
    },
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AppView {
    #[default]
    Attention,
    Projects,
    Fleet,
}

impl AppView {
    const fn label(self) -> &'static str {
        match self {
            Self::Attention => "Attention",
            Self::Projects => "Projects",
            Self::Fleet => "Fleet",
        }
    }
}

#[component]
pub fn App() -> impl IntoView {
    let active_view = RwSignal::new(AppView::Attention);
    let inspector_open = RwSignal::new(true);
    let context_open = RwSignal::new(false);
    let notice = RwSignal::new(None::<String>);
    let read_only = is_read_only_web();

    view! {
        <div class="app-frame">
            <header class="native-titlebar" data-tauri-drag-region="">
                <div class="titlebar-leading" data-tauri-drag-region="">
                    <span class="titlebar-product" data-tauri-drag-region="">"Utu"</span>
                    <span class="titlebar-owner" data-tauri-drag-region="">"Local owner"</span>
                </div>
                <span class="titlebar-view" data-tauri-drag-region="">{move || active_view.get().label()}</span>
                <DemoBadge web=read_only />
            </header>

            <div
                class=move || if inspector_open.get() { "app-shell inspector-is-open" } else { "app-shell" }
                class:context-is-open=move || context_open.get()
            >
                <UtilityRail active_view context_open />
                <ContextRail active_view read_only />

                <section class="work-surface" aria-label="Utu workspace">
                    <div class="mobile-workspace-switch">
                        <button
                            class="icon-button"
                            type="button"
                            aria-label="Open context"
                            on:click=move |_| context_open.update(|open| *open = !*open)
                        ><Icon path=ICON_COMMAND /></button>
                        <ViewSwitch active=active_view />
                    </div>

                    <Show when=move || active_view.get() == AppView::Attention>
                        <AttentionView inspector_open read_only notice />
                    </Show>
                    <Show when=move || active_view.get() == AppView::Projects>
                        <ProjectsView inspector_open read_only notice />
                    </Show>
                    <Show when=move || active_view.get() == AppView::Fleet>
                        <FleetView inspector_open read_only notice />
                    </Show>
                </section>

                <Show when=move || inspector_open.get()>
                    <aside class="inspector" aria-label="Selection details">
                        <Show when=move || active_view.get() == AppView::Attention>
                            <AttentionInspector inspector_open read_only notice />
                        </Show>
                        <Show when=move || active_view.get() == AppView::Projects>
                            <ProjectInspector inspector_open read_only notice />
                        </Show>
                        <Show when=move || active_view.get() == AppView::Fleet>
                            <FleetInspector inspector_open read_only notice />
                        </Show>
                    </aside>
                </Show>

                <button
                    class="context-scrim"
                    type="button"
                    aria-label="Close context"
                    on:click=move |_| context_open.set(false)
                ></button>
            </div>

            <Show when=move || notice.get().is_some()>
                <div class="toast" role="status">
                    <span>{move || notice.get().unwrap_or_default()}</span>
                    <button type="button" on:click=move |_| notice.set(None)>"Dismiss"</button>
                </div>
            </Show>
        </div>
    }
}

#[component]
fn UtilityRail(active_view: RwSignal<AppView>, context_open: RwSignal<bool>) -> impl IntoView {
    view! {
        <nav class="utility-rail" aria-label="Application">
            <div class="utility-primary">
                <button class="app-mark" type="button" aria-label="Utu home" on:click=move |_| active_view.set(AppView::Attention)>
                    <AppMarkGlyph />
                </button>
                <button class="rail-button is-active" type="button" aria-label="Workspace" on:click=move |_| context_open.set(true)><Icon path=ICON_HOME /></button>
                <button class="rail-button" type="button" aria-label="Search"><Icon path=ICON_SEARCH /></button>
                <button class="rail-button" type="button" aria-label="Coordination"><Icon path=ICON_NODES /></button>
                <button class="rail-button" type="button" aria-label="Connectors"><Icon path=ICON_PLUG /><span class="rail-alert"></span></button>
                <button class="rail-button" type="button" aria-label="Costs"><Icon path=ICON_COST /></button>
            </div>
            <div class="utility-secondary">
                <button class="rail-button" type="button" aria-label="Settings"><Icon path=ICON_SETTINGS /></button>
                <button class="owner-avatar" type="button" aria-label="Owner profile">"K"<StatusDot /></button>
            </div>
        </nav>
    }
}

#[component]
fn ContextRail(active_view: RwSignal<AppView>, read_only: bool) -> impl IntoView {
    view! {
        <aside class="context-rail" aria-label="Workspace context">
            <div class="context-header">
                <ViewSwitch active=active_view />
                <p class="context-subtitle">{if read_only { "Read-only status" } else { "Owner workspace" }}</p>
            </div>

            <div class="context-content">
                <Show when=move || active_view.get() == AppView::Attention>
                    <AttentionContext />
                </Show>
                <Show when=move || active_view.get() == AppView::Projects>
                    <ProjectContext />
                </Show>
                <Show when=move || active_view.get() == AppView::Fleet>
                    <FleetContext />
                </Show>
            </div>
            <div class="context-footer">
                <span><StatusDot />"Local service"</span>
                <span class="footer-state">"Ready"</span>
            </div>
        </aside>
    }
}

#[component]
fn AttentionContext() -> impl IntoView {
    view! {
        <div class="context-section">
            <div class="section-label"><span>"Needs you"</span><span class="count count-attention">"3"</span></div>
            <button class="context-row is-selected" type="button">
                <AgentAvatar initials="CL" tone="coral" size="sm" />
                <span class="row-copy"><strong>"Claude"</strong><small>"HomeTender · approval"</small></span>
                <span class="row-time">"18s"</span>
            </button>
            <button class="context-row" type="button">
                <AgentAvatar initials="CO" tone="blue" size="sm" />
                <span class="row-copy"><strong>"Codex"</strong><small>"NOCTIVOX · quiet"</small></span>
                <span class="row-time">"12m"</span>
            </button>
            <button class="context-row" type="button">
                <AgentAvatar initials="GE" tone="lime" size="sm" />
                <span class="row-copy"><strong>"Gemma"</strong><small>"MediaServer · input"</small></span>
                <span class="row-time">"27m"</span>
            </button>
        </div>
        <div class="context-section">
            <div class="section-label"><span>"Working"</span><span class="count count-healthy">"6"</span></div>
            <AgentContextRow initials="QW" tone="violet" name="Qwen" project="HomeTender" detail="Running tests" time="5m" />
            <AgentContextRow initials="LM" tone="rose" name="Llama" project="NOCTIVOX" detail="Indexing library" time="9m" />
            <AgentContextRow initials="MI" tone="amber" name="Mistral" project="MediaServer" detail="Transcoding" time="14m" />
            <button class="text-button small" type="button">"View all 6"</button>
        </div>
        <div class="context-section quiet-section">
            <div class="section-label"><span>"Quiet"</span><span class="count">"8"</span></div>
            <AgentContextRow initials="PH" tone="teal" name="Phi" project="HomeTender" detail="Idle" time="1h" />
            <AgentContextRow initials="YI" tone="sand" name="Yi" project="NOCTIVOX" detail="Idle" time="2h" />
        </div>
    }
}

#[component]
fn ProjectContext() -> impl IntoView {
    view! {
        <div class="context-section">
            <div class="section-label"><span>"Projects"</span><button class="bare-plus" type="button" aria-label="Add project">"+"</button></div>
            <ProjectContextRow initials="HT" tone="teal" name="HomeTender" running="5" waiting="2" selected=true />
            <ProjectContextRow initials="NV" tone="violet" name="NOCTIVOX" running="3" waiting="1" />
            <ProjectContextRow initials="MS" tone="blue" name="MediaServer" running="4" waiting="0" />
            <ProjectContextRow initials="MW" tone="sand" name="Minimal Wiki" running="1" waiting="3" />
            <ProjectContextRow initials="PH" tone="aqua" name="Property History" running="2" waiting="1" />
        </div>
        <div class="context-section compact-status">
            <div class="section-label"><span>"Recent"</span></div>
            <button class="plain-context-link" type="button"><StatusDot tone="attention" />"Samurai Sushi"<span>"waiting"</span></button>
            <button class="plain-context-link" type="button"><StatusDot />"Utu"<span>"active"</span></button>
        </div>
    }
}

#[component]
fn FleetContext() -> impl IntoView {
    view! {
        <div class="context-section">
            <div class="section-label"><span><StatusDot />"Running"</span><span class="count">"6"</span></div>
            <AgentFleetRow initials="CO" tone="teal" name="Codex" provider="OpenAI · GPT-5" project="Utu" time="18m" selected=true />
            <AgentFleetRow initials="CL" tone="amber" name="Claude" provider="Anthropic · Sonnet" project="Docs revamp" time="14m" />
            <AgentFleetRow initials="AG" tone="navy" name="Antigravity" provider="Local · Llama" project="Infra migration" time="9m" />
            <AgentFleetRow initials="CU" tone="ink" name="Cursor" provider="Cursor · Auto" project="CLI refactor" time="6m" />
            <AgentFleetRow initials="GR" tone="purple" name="Grok" provider="xAI · Grok" project="Market scan" time="4m" />
        </div>
        <div class="context-section">
            <div class="section-label"><span><StatusDot tone="attention" />"Waiting"</span><span class="count">"3"</span></div>
            <AgentFleetRow initials="NV" tone="violet" name="NOCTIVOX" provider="Local · Mistral" project="Voice Notes" time="2m" />
            <AgentFleetRow initials="MS" tone="blue" name="MediaServer" provider="Local · Llama" project="Transcode" time="1m" />
        </div>
        <div class="context-section collapsed-section"><div class="section-label"><span><StatusDot tone="quiet" />"Idle"</span><span class="count">"8"</span></div></div>
        <div class="context-section collapsed-section"><div class="section-label"><span><StatusDot tone="problem" />"Problems"</span><span class="count">"2"</span></div></div>
    }
}

#[component]
fn AgentContextRow(
    initials: &'static str,
    tone: &'static str,
    name: &'static str,
    project: &'static str,
    detail: &'static str,
    time: &'static str,
) -> impl IntoView {
    view! {
        <button class="context-row" type="button">
            <AgentAvatar initials tone size="sm" />
            <span class="row-copy"><strong>{name}</strong><small>{project}" · "{detail}</small></span>
            <span class="row-time">{time}</span>
        </button>
    }
}

#[component]
fn ProjectContextRow(
    initials: &'static str,
    tone: &'static str,
    name: &'static str,
    running: &'static str,
    waiting: &'static str,
    #[prop(default = false)] selected: bool,
) -> impl IntoView {
    view! {
        <button class=if selected { "project-context-row is-selected" } else { "project-context-row" } type="button">
            <AgentAvatar initials tone size="md" />
            <span><strong>{name}</strong><small><StatusDot />{running}" running"<span class="context-waiting">{waiting}" waiting"</span></small></span>
        </button>
    }
}

#[component]
fn AgentFleetRow(
    initials: &'static str,
    tone: &'static str,
    name: &'static str,
    provider: &'static str,
    project: &'static str,
    time: &'static str,
    #[prop(default = false)] selected: bool,
) -> impl IntoView {
    view! {
        <button class=if selected { "fleet-context-row is-selected" } else { "fleet-context-row" } type="button">
            <AgentAvatar initials tone size="sm" />
            <span class="row-copy"><strong>{name}</strong><small>{provider}</small></span>
            <span class="fleet-row-meta"><small>{project}</small><span>{time}</span></span>
        </button>
    }
}

fn is_read_only_web() -> bool {
    web_sys::window()
        .and_then(|window| window.location().search().ok())
        .is_some_and(|search| search.contains("surface=web") || search.contains("readonly=1"))
}
