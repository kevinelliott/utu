use leptos::prelude::*;

use crate::{
    app::AppView,
    components::{ICON_PLUG, Icon},
    theme::{ThemeController, ThemePreference},
    workspace_data::{LiveStatus, WorkspaceAction, WorkspaceActionSink},
};

#[component]
pub fn SettingsView() -> impl IntoView {
    let live = expect_context::<LiveStatus>();
    let actions = expect_context::<WorkspaceActionSink>();
    let active_view = expect_context::<RwSignal<AppView>>();
    let theme = expect_context::<ThemeController>();
    let is_web = !live.is_desktop();

    view! {
        <div class="workspace-layout settings-layout">
            <header class="workspace-toolbar">
                <div class="toolbar-leading">
                    <div><h1>"Settings"</h1><p>"Local owner configuration"</p></div>
                </div>
            </header>
            <div class="settings-content">
                <section class="settings-group">
                    <h2>"Appearance"</h2>
                    <div class="settings-rows">
                        <div class="settings-row settings-row-action">
                            <span class="settings-row-label">"Theme"</span>
                            <span class="settings-row-value">
                                {move || match theme.preference.get() {
                                    ThemePreference::System => "System follows this device",
                                    ThemePreference::Light => "Light",
                                    ThemePreference::Dark => "Dark",
                                }}
                            </span>
                            <div class="theme-switch" role="radiogroup" aria-label="Theme">
                                <button
                                    class="theme-option"
                                    class:is-active=move || theme.preference.get() == ThemePreference::System
                                    type="button"
                                    role="radio"
                                    aria-checked=move || theme.preference.get() == ThemePreference::System
                                    on:click=move |_| theme.set(ThemePreference::System)
                                >
                                    "System"
                                </button>
                                <button
                                    class="theme-option"
                                    class:is-active=move || theme.preference.get() == ThemePreference::Light
                                    type="button"
                                    role="radio"
                                    aria-checked=move || theme.preference.get() == ThemePreference::Light
                                    on:click=move |_| theme.set(ThemePreference::Light)
                                >
                                    "Light"
                                </button>
                                <button
                                    class="theme-option"
                                    class:is-active=move || theme.preference.get() == ThemePreference::Dark
                                    type="button"
                                    role="radio"
                                    aria-checked=move || theme.preference.get() == ThemePreference::Dark
                                    on:click=move |_| theme.set(ThemePreference::Dark)
                                >
                                    "Dark"
                                </button>
                            </div>
                        </div>
                        <Show when=move || is_web>
                            <div class="settings-row">
                                <span class="settings-row-label">"This browser"</span>
                                <span class="settings-row-value">"Remembers the choice on this device. It is not synced."</span>
                            </div>
                        </Show>
                    </div>
                </section>

                <section class="settings-group">
                    <h2>"Workspace"</h2>
                    <div class="settings-rows">
                        <div class="settings-row">
                            <span class="settings-row-label">"Device type"</span>
                            <span class="settings-row-value">"Owner device · local-first"</span>
                        </div>
                        <div class="settings-row">
                            <span class="settings-row-label">"Projects"</span>
                            <span class="settings-row-value">
                                {move || live.snapshot.get()
                                    .map(|s| format!("{} stored", s.projects.len()))
                                    .unwrap_or_else(|| "—".into())}
                            </span>
                        </div>
                        <div class="settings-row">
                            <span class="settings-row-label">"Sessions"</span>
                            <span class="settings-row-value">
                                {move || live.snapshot.get()
                                    .map(|s| format!("{} observed", s.sessions.len()))
                                    .unwrap_or_else(|| "—".into())}
                            </span>
                        </div>
                    </div>
                </section>

                <section class="settings-group">
                    <h2>"Agents & Integrations"</h2>
                    <div class="settings-rows">
                        <div class="settings-row settings-row-action">
                            <span class="settings-row-label">"Connected CLIs"</span>
                            <span class="settings-row-value">
                                {move || live.diagnostics.get()
                                    .map(|d| format!("{} connectors observed", d.connectors.len()))
                                    .unwrap_or_else(|| "Checking…".into())}
                            </span>
                            <button
                                class="secondary-button settings-action-button"
                                type="button"
                                on:click=move |_| {
                                    actions.dispatch(WorkspaceAction::SelectView("integrations"));
                                    active_view.set(AppView::Integrations);
                                }
                            >
                                <Icon path=ICON_PLUG />
                                "Manage connectors"
                            </button>
                        </div>
                    </div>
                </section>

                <section class="settings-group">
                    <h2>"Data"</h2>
                    <div class="settings-rows">
                        <div class="settings-row">
                            <span class="settings-row-label">"Store schema"</span>
                            <span class="settings-row-value">
                                {move || live.snapshot.get()
                                    .map(|s| format!("v{}", s.store.schema_version))
                                    .unwrap_or_else(|| "—".into())}
                            </span>
                        </div>
                        <div class="settings-row">
                            <span class="settings-row-label">"Integrity"</span>
                            <span class="settings-row-value">
                                {move || live.snapshot.get()
                                    .map(|s| if s.store.integrity_ok { "Healthy" } else { "Needs attention" })
                                    .unwrap_or("—")}
                            </span>
                        </div>
                        <div class="settings-row">
                            <span class="settings-row-label">"Foreign keys"</span>
                            <span class="settings-row-value">
                                {move || live.snapshot.get()
                                    .map(|s| if s.store.foreign_keys_enabled { "Enabled" } else { "Disabled" })
                                    .unwrap_or("—")}
                            </span>
                        </div>
                        <div class="settings-row">
                            <span class="settings-row-label">"Transcripts"</span>
                            <span class="settings-row-value">"Observed only · not imported"</span>
                        </div>
                    </div>
                </section>

                <section class="settings-group">
                    <h2>"About"</h2>
                    <div class="settings-rows">
                        <div class="settings-row">
                            <span class="settings-row-label">"Utu"</span>
                            <span class="settings-row-value">"Owner-device, local-first agent workspace"</span>
                        </div>
                        <div class="settings-row">
                            <span class="settings-row-label">"Data residency"</span>
                            <span class="settings-row-value">"All data stays on this device. No cloud sync."</span>
                        </div>
                    </div>
                </section>
            </div>
        </div>
    }
}
