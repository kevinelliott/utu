use leptos::prelude::*;

use crate::components::{
    AgentAvatar, AgentCliIcon, Composer, EmptyInspectorButton, ICON_CHECK, ICON_CLOSE, ICON_FILE,
    ICON_MORE, ICON_PAUSE, ICON_PLUS, ICON_SEND, ICON_SHIELD, ICON_STOP, Icon, StatusDot,
};

#[component]
pub fn ProjectsView(
    inspector_open: RwSignal<bool>,
    read_only: bool,
    notice: RwSignal<Option<String>>,
) -> impl IntoView {
    view! {
        <div class="workspace-layout project-layout">
            <header class="workspace-toolbar project-toolbar">
                <div class="toolbar-leading">
                    <div class="project-heading">
                        <h1>"HomeTender"</h1>
                        <p>"~/Projects/hometender"</p>
                    </div>
                </div>
                <div class="toolbar-actions project-toolbar-actions">
                    <div class="avatar-stack" aria-label="5 active agents">
                        <AgentCliIcon connector_id="codex" size="sm" />
                        <AgentCliIcon connector_id="claude" size="sm" />
                        <AgentCliIcon connector_id="aider" size="sm" />
                        <span class="stack-more">"+2"</span>
                    </div>
                    <button class="icon-button" type="button" aria-label="Project actions"><Icon path=ICON_MORE /></button>
                    <button class="primary-button compact-action" type="button" disabled=read_only on:click=move |_| announce(read_only, notice, "A project task draft is ready to configure.")><Icon path=ICON_PLUS />"New task"</button>
                    <Show when=move || !inspector_open.get()><EmptyInspectorButton inspector_open /></Show>
                </div>
            </header>

            <div class="project-workspace">
                <nav class="work-groups" aria-label="Task groups">
                    <div class="work-group">
                        <p>"Today"<span>"6"</span></p>
                        <button class="is-active" type="button">"All"<span>"6"</span></button>
                        <button type="button">"Working"<span>"4"</span></button>
                        <button type="button">"Waiting for you"<span>"2"</span></button>
                    </div>
                    <div class="work-group">
                        <p>"In progress"<span>"4"</span></p>
                        <button type="button"><StatusDot />"Running"<span>"4"</span></button>
                    </div>
                    <div class="work-group">
                        <p>"Waiting"<span>"2"</span></p>
                        <button type="button"><StatusDot tone="problem" />"Blocked"<span>"1"</span></button>
                        <button type="button"><StatusDot tone="attention" />"External"<span>"1"</span></button>
                    </div>
                    <div class="work-group">
                        <p>"Done"<span>"18"</span></p>
                        <button type="button"><StatusDot tone="quiet" />"Completed"<span>"17"</span></button>
                        <button type="button"><StatusDot tone="quiet" />"Canceled"<span>"1"</span></button>
                    </div>
                </nav>

                <div class="task-stream">
                    <TaskBand title="Ingest appraisal PDFs" agents="Codex → Embedder" time="Observed 28s ago" state="Working" tone="healthy" />
                    <TaskBand title="Normalize address variants" agents="Claude → Validator" time="Observed 2m ago" state="Working" tone="healthy" />

                    <article class="task-band expanded-task" on:click=move |_| inspector_open.set(true)>
                        <div class="task-band-summary">
                            <span class="task-kind"><Icon path=ICON_FILE /></span>
                            <div class="task-copy"><strong>"Review exact candidate"</strong><small><AgentCliIcon connector_id="codex" size="xs" />"Codex"<span>"→"</span><AgentCliIcon connector_id="claude" size="xs" />"Claude"</small></div>
                            <span class="task-freshness">"Observed 3m ago"</span>
                            <span class="state-label healthy"><StatusDot />"Working"</span>
                        </div>
                        <div class="expanded-content">
                            <div class="current-action">
                                <h3>"Current action"</h3>
                                <p>"Codex is analyzing candidate matches and preparing a ranked short list."</p>
                            </div>
                            <div class="handoff-grid">
                                <div>
                                    <h3>"Handoff"</h3>
                                    <p class="handoff-line"><AgentCliIcon connector_id="codex" size="sm" /><strong>"Codex"</strong><span>"→"</span><AgentCliIcon connector_id="claude" size="sm" /><strong>"Claude"</strong></p>
                                    <small>"Will hand off when analysis is complete."</small>
                                </div>
                                <div>
                                    <h3>"Last verified artifact"</h3>
                                    <button class="verified-artifact" type="button"><Icon path=ICON_FILE /><span><strong>"candidates_ranked.json"</strong><small>"Verified 4m ago · 148 KB"</small></span><Icon path=ICON_CHECK /></button>
                                </div>
                            </div>
                            <div class="recent-log">
                                <h3>"Recent log"</h3>
                                <div class="log-preview compact-log"><span>"12:41:02  Parsed 28 appraisal PDFs"</span><span>"12:41:18  Extracted 4,176 comparable records"</span><span>"12:41:24  Scored candidates and built initial ranking"</span></div>
                            </div>
                            <div class="direction-history">
                                <h3>"Owner direction history"</h3>
                                <div><AgentAvatar initials="YO" tone="ink" size="sm" /><p><strong>"You"<span>" · 12:19 PM"</span></strong>"Focus on exact matches first. Defer fuzzy matches. Added filters: distance ≤ 0.25 mi, sold within 12 months."</p></div>
                            </div>
                        </div>
                    </article>

                    <TaskBand title="Reminder downstream authorization" agents="Claude → Notifier" time="Observed 6m ago" state="Waiting for you" tone="attention" />
                    <TaskBand title="Generate CMA summary" agents="Writer → Reviewer" time="Observed 12m ago" state="Working" tone="healthy" />
                    <TaskBand title="Update listing status" agents="Claude → API Client" time="Observed 18m ago" state="External" tone="attention" />
                </div>
            </div>

            <Composer placeholder="Message this project or assign work…" context="HomeTender" agent="Codex + Claude" read_only notice />
        </div>
    }
}

#[component]
fn TaskBand(
    title: &'static str,
    agents: &'static str,
    time: &'static str,
    state: &'static str,
    tone: &'static str,
) -> impl IntoView {
    view! {
        <button class="task-band" type="button">
            <span class=format!("task-kind kind-{tone}")><Icon path=ICON_FILE /></span>
            <span class="task-copy"><strong>{title}</strong><small>{agents}</small></span>
            <span class="task-freshness">{time}</span>
            <span class=format!("state-label {tone}")><StatusDot tone />{state}</span>
        </button>
    }
}

#[component]
pub fn ProjectInspector(
    inspector_open: RwSignal<bool>,
    read_only: bool,
    notice: RwSignal<Option<String>>,
) -> impl IntoView {
    view! {
        <div class="inspector-content">
            <header class="inspector-header simple-inspector-header">
                <div><h2>"Review exact candidate"</h2><p>"Task · Created 10:31 AM"</p></div>
                <button class="icon-button" type="button" aria-label="Close details" on:click=move |_| inspector_open.set(false)><Icon path=ICON_CLOSE /></button>
            </header>

            <section class="inspector-section">
                <h3>"Assignees"</h3>
                <div class="assignee-handoff">
                    <div><AgentCliIcon connector_id="codex" size="md" /><span><strong>"Codex"</strong><small>"Analyzer"</small></span></div>
                    <span>"→"</span>
                    <div><AgentCliIcon connector_id="claude" size="md" /><span><strong>"Claude"</strong><small>"Reviewer"</small></span></div>
                </div>
            </section>

            <section class="inspector-section">
                <h3>"Active session"</h3>
                <div class="surface-row"><Icon path=ICON_SHIELD /><span><strong>"Session 7f3a…c2d9"</strong><small>"Started 11:02 AM · 3m ago"</small></span><span class="state-label healthy"><StatusDot />"Running"</span></div>
            </section>

            <section class="inspector-section">
                <h3>"Sandbox"</h3>
                <div class="surface-row"><Icon path=ICON_SHIELD /><span><strong>"Local VM"</strong><small>"2 vCPU · 4 GB RAM"</small></span><span class="state-label healthy"><StatusDot />"Healthy"</span></div>
            </section>

            <section class="inspector-section">
                <h3>"Estimated cost"</h3>
                <div class="cost-emphasis"><strong>"~$0.014"</strong><small>"≈ 142K tokens · partial evidence"</small></div>
            </section>

            <section class="inspector-section">
                <h3>"Files changed"</h3>
                <button class="artifact-row" type="button"><span>"candidates_ranked.json"</span><small>"148 KB"</small></button>
                <button class="artifact-row" type="button"><span>"filters.json"</span><small>"632 B"</small></button>
                <button class="artifact-row" type="button"><span>"analysis.log"</span><small>"3.1 KB"</small></button>
            </section>

            <section class="inspector-section stacked-controls">
                <h3>"Owner controls"</h3>
                <button class="primary-outline" type="button" disabled=read_only on:click=move |_| announce(read_only, notice, "Direction composer focused.")><Icon path=ICON_SEND />"Send direction"</button>
                <button type="button" disabled=read_only on:click=move |_| announce(read_only, notice, "Pause requires confirmation in a live session.")><Icon path=ICON_PAUSE />"Pause"</button>
                <button class="danger-control" type="button" disabled=read_only on:click=move |_| announce(read_only, notice, "Stop requires confirmation in a live session.")><Icon path=ICON_STOP />"Stop"</button>
            </section>
        </div>
    }
}

fn announce(read_only: bool, notice: RwSignal<Option<String>>, message: &str) {
    notice.set(Some(if read_only {
        "The web dashboard is read-only. Use the owner desktop app for controls.".into()
    } else {
        message.into()
    }));
}
