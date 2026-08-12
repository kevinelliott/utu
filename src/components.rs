use leptos::prelude::*;

use crate::app::AppView;

pub const ICON_HOME: &str =
    "M3 10.5 12 3l9 7.5v9a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 19.5zm6 10v-6h6v6";
pub const ICON_SEARCH: &str = "m21 21-4.35-4.35m2.35-5.65a8 8 0 1 1-16 0 8 8 0 0 1 16 0";
pub const ICON_NODES: &str =
    "M8 6h8M8 18h8M6 8v8m12-8v8M6 3a3 3 0 1 0 0 6 3 3 0 0 0 0-6m12 12a3 3 0 1 0 0 6 3 3 0 0 0 0-6";
pub const ICON_PLUG: &str = "M8 3v4m8-4v4M6 7h12v3a6 6 0 0 1-6 6v5m-3 0h6";
pub const ICON_COST: &str = "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20m3 7c-.5-1-1.5-1.5-3-1.5-1.7 0-3 .9-3 2.2 0 3.3 6 1.5 6 4.6 0 1.3-1.3 2.2-3 2.2-1.5 0-2.7-.5-3.4-1.5M12 5v14";
pub const ICON_SETTINGS: &str = "M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7m7.4 3.5 1.3-2.2-2-3.4h-2.6L14.8 4h-4L9.5 6.4H6.9l-2 3.4L6.2 12l-1.3 2.2 2 3.4h2.6l1.3 2.4h4l1.3-2.4h2.6l2-3.4z";
pub const ICON_FILTER: &str = "M4 5h16l-6.5 7.2V19l-3 1v-7.8z";
pub const ICON_PLUS: &str = "M12 5v14M5 12h14";
pub const ICON_SEND: &str = "m4 4 17 8-17 8 3-8zm3 8h14";
pub const ICON_CLOSE: &str = "m6 6 12 12M18 6 6 18";
pub const ICON_PAUSE: &str = "M8 5v14m8-14v14";
pub const ICON_STOP: &str = "M7 7h10v10H7z";
pub const ICON_MORE: &str = "M5 12h.01M12 12h.01M19 12h.01";
pub const ICON_ARROW: &str = "m9 18 6-6-6-6";
pub const ICON_CHECK: &str = "m5 12 4 4L19 6";
pub const ICON_FILE: &str = "M6 2h8l4 4v16H6zm8 0v6h6M9 13h6m-6 4h6";
pub const ICON_SHIELD: &str = "M12 3 20 6v6c0 5-3.5 8-8 10-4.5-2-8-5-8-10V6z";
pub const ICON_BACK: &str = "m15 18-6-6 6-6";
pub const ICON_FORWARD: &str = "m9 18 6-6-6-6";
pub const ICON_COMMAND: &str =
    "M9 6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3z";

#[component]
pub fn AppMarkGlyph() -> impl IntoView {
    view! {
        <svg viewBox="0 0 64 64" aria-hidden="true" focusable="false">
            <rect width="64" height="64" rx="16" fill="#183a31" />
            <path d="M18 19h28v8H18zm0 14h18v12H18zm24 0h4v12h-4z" fill="#eff6f2" />
        </svg>
    }
}

#[component]
pub fn Icon(#[prop(into)] path: String) -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
            <path d=path />
        </svg>
    }
}

#[component]
pub fn AgentAvatar(
    initials: &'static str,
    tone: &'static str,
    #[prop(default = "md")] size: &'static str,
) -> impl IntoView {
    view! {
        <span class=format!("agent-avatar avatar-{tone} avatar-{size}") aria-hidden="true">
            {initials}
        </span>
    }
}

#[component]
pub fn StatusDot(#[prop(default = "healthy")] tone: &'static str) -> impl IntoView {
    view! { <span class=format!("status-dot status-{tone}") aria-hidden="true"></span> }
}

#[component]
pub fn ViewSwitch(active: RwSignal<AppView>) -> impl IntoView {
    view! {
        <div class="view-switch" role="tablist" aria-label="Workspace view">
            <button
                type="button"
                role="tab"
                aria-selected=move || (active.get() == AppView::Attention).to_string()
                class=move || if active.get() == AppView::Attention { "is-active" } else { "" }
                on:click=move |_| active.set(AppView::Attention)
            >"Attention"</button>
            <button
                type="button"
                role="tab"
                aria-selected=move || (active.get() == AppView::Projects).to_string()
                class=move || if active.get() == AppView::Projects { "is-active" } else { "" }
                on:click=move |_| active.set(AppView::Projects)
            >"Projects"</button>
            <button
                type="button"
                role="tab"
                aria-selected=move || (active.get() == AppView::Fleet).to_string()
                class=move || if active.get() == AppView::Fleet { "is-active" } else { "" }
                on:click=move |_| active.set(AppView::Fleet)
            >"Fleet"</button>
        </div>
    }
}

#[component]
pub fn DemoBadge(#[prop(default = false)] web: bool) -> impl IntoView {
    view! {
        <span class="demo-badge">
            <StatusDot tone="attention" />
            {if web { "Web status · Demo" } else { "Demo workspace" }}
        </span>
    }
}

#[component]
pub fn WorkspaceNav() -> impl IntoView {
    view! {
        <div class="workspace-nav" aria-label="History controls">
            <button class="icon-button compact" type="button" aria-label="Back"><Icon path=ICON_BACK /></button>
            <button class="icon-button compact" type="button" aria-label="Forward"><Icon path=ICON_FORWARD /></button>
        </div>
    }
}

#[component]
pub fn Composer(
    placeholder: &'static str,
    context: &'static str,
    agent: &'static str,
    read_only: bool,
    notice: RwSignal<Option<String>>,
) -> impl IntoView {
    let submit = move |_| {
        let message = if read_only {
            "The web status surface is read-only. Open Utu on the owner device to direct an agent."
        } else {
            "Direction staged in this prototype. A live connector will deliver it after explicit confirmation."
        };
        notice.set(Some(message.into()));
    };

    view! {
        <div class="composer" class:is-read-only=read_only>
            <label class="sr-only" for="agent-direction">{placeholder}</label>
            <textarea id="agent-direction" rows="1" placeholder=placeholder readonly=read_only></textarea>
            <div class="composer-toolbar">
                <div class="composer-context">
                    <button type="button" class="context-chip" disabled=read_only>{context}</button>
                    <button type="button" class="context-chip" disabled=read_only>{agent}</button>
                    <button type="button" class="icon-button mini" disabled=read_only aria-label="Add context"><Icon path=ICON_PLUS /></button>
                </div>
                <button type="button" class="send-button" disabled=read_only on:click=submit aria-label="Send direction">
                    <Icon path=ICON_SEND />
                </button>
            </div>
        </div>
    }
}

#[component]
pub fn EvidenceTag(kind: &'static str) -> impl IntoView {
    let tone = match kind {
        "Observed" => "observed",
        "Inferred" => "inferred",
        "Stale" => "stale",
        _ => "unknown",
    };
    view! { <span class=format!("evidence-tag evidence-{tone}")>{kind}</span> }
}

#[component]
pub fn EmptyInspectorButton(inspector_open: RwSignal<bool>) -> impl IntoView {
    view! {
        <button class="icon-button" type="button" aria-label="Open details" on:click=move |_| inspector_open.set(true)>
            <Icon path=ICON_ARROW />
        </button>
    }
}
