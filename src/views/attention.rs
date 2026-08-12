use leptos::prelude::*;

use crate::components::{
    AgentAvatar, Composer, EmptyInspectorButton, EvidenceTag, ICON_CHECK, ICON_CLOSE, ICON_FILE,
    ICON_FILTER, ICON_MORE, ICON_PAUSE, ICON_PLUS, ICON_SEND, ICON_SHIELD, ICON_STOP, Icon,
    StatusDot, WorkspaceNav,
};

#[component]
pub fn AttentionView(
    inspector_open: RwSignal<bool>,
    read_only: bool,
    notice: RwSignal<Option<String>>,
) -> impl IntoView {
    view! {
        <div class="workspace-layout attention-layout">
            <header class="workspace-toolbar">
                <div class="toolbar-leading">
                    <WorkspaceNav />
                    <div>
                        <h1>"Needs your attention"</h1>
                        <p>"Decisions and problems across active work"</p>
                    </div>
                </div>
                <div class="toolbar-actions">
                    <button class="icon-button" type="button" aria-label="Filter attention"><Icon path=ICON_FILTER /></button>
                    <button
                        class="primary-button compact-action"
                        type="button"
                        disabled=read_only
                        on:click=move |_| announce(read_only, notice, "A new task draft is ready to configure.")
                    ><Icon path=ICON_PLUS />"New task"</button>
                    <Show when=move || !inspector_open.get()>
                        <EmptyInspectorButton inspector_open />
                    </Show>
                </div>
            </header>

            <div class="work-stream attention-stream">
                <article class="attention-event is-selected" on:click=move |_| inspector_open.set(true)>
                    <div class="event-avatar"><AgentAvatar initials="CL" tone="coral" size="lg" /><StatusDot tone="attention" /></div>
                    <div class="event-body">
                        <div class="event-meta">
                            <div><strong>"Claude"</strong><span>"Anthropic"</span><span class="meta-divider">"HomeTender / release-1.2"</span></div>
                            <div class="event-evidence"><span>"18s ago"</span><EvidenceTag kind="Observed" /></div>
                        </div>
                        <h2>"Release review is waiting for approval"</h2>
                        <p>"The release candidate checks finished successfully. Review the evidence before allowing the next handoff."</p>
                        <div class="log-preview" aria-label="Latest observed output">
                            <span>"$ make verify"</span>
                            <span>"Running 1,283 tests…"</span>
                            <span class="log-success">"All tests passed (1,283/1,283)"</span>
                            <span>"Coverage: 91.4% (+2.1%)"</span>
                        </div>
                        <div class="inline-actions">
                            <button class="primary-button" type="button" on:click=move |event| { event.stop_propagation(); announce(read_only, notice, "Review opened with demonstration evidence.") }>"Review"</button>
                            <button class="secondary-button" type="button" disabled=read_only on:click=move |event| { event.stop_propagation(); announce(read_only, notice, "Approval requires a live connector and explicit confirmation.") }><Icon path=ICON_CHECK />"Approve"</button>
                            <button class="secondary-button" type="button" disabled=read_only on:click=move |event| { event.stop_propagation(); announce(read_only, notice, "Direction composer focused.") }><Icon path=ICON_SEND />"Send direction"</button>
                            <button class="icon-button mini" type="button" aria-label="More actions"><Icon path=ICON_MORE /></button>
                        </div>
                    </div>
                </article>

                <article class="attention-event" on:click=move |_| inspector_open.set(true)>
                    <div class="event-avatar"><AgentAvatar initials="CO" tone="blue" size="lg" /><StatusDot tone="problem" /></div>
                    <div class="event-body">
                        <div class="event-meta">
                            <div><strong>"Codex"</strong><span>"OpenAI"</span><span class="meta-divider">"NOCTIVOX / ingest-pipeline"</span></div>
                            <div class="event-evidence"><span>"12m ago"</span><EvidenceTag kind="Inferred" /></div>
                        </div>
                        <h2>"Codex has gone quiet"</h2>
                        <p>"No output has been observed for 12 minutes. The last event suggests disk I/O, but the connector cannot confirm the cause."</p>
                        <div class="log-preview compact-log">
                            <span>"10:41:45  ✓ Fetched 23,481 items"</span>
                            <span>"10:41:52  → Parsing catalog (batch 12/45)…"</span>
                            <span>"10:42:01  … waiting on disk I/O"</span>
                        </div>
                        <div class="inline-actions">
                            <button class="secondary-button" type="button" disabled=read_only on:click=move |event| { event.stop_propagation(); announce(read_only, notice, "Reconnect is available when the connector is live.") }>
                                "Reconnect"
                            </button>
                            <button class="secondary-button" type="button" disabled=read_only on:click=move |event| { event.stop_propagation(); announce(read_only, notice, "Direction composer focused.") }><Icon path=ICON_SEND />"Send direction"</button>
                            <button class="icon-button mini" type="button" aria-label="More actions"><Icon path=ICON_MORE /></button>
                        </div>
                    </div>
                </article>

                <article class="attention-event" on:click=move |_| inspector_open.set(true)>
                    <div class="event-avatar"><AgentAvatar initials="GE" tone="lime" size="lg" /><StatusDot tone="attention" /></div>
                    <div class="event-body">
                        <div class="event-meta">
                            <div><strong>"Gemma"</strong><span>"Local"</span><span class="meta-divider">"MediaServer / subtitle-sync"</span></div>
                            <div class="event-evidence"><span>"27m ago"</span><EvidenceTag kind="Observed" /></div>
                        </div>
                        <h2>"Needs your input"</h2>
                        <p>"Three subtitle segments have conflicting sources. Choose the preferred source or provide a project rule."</p>
                        <div class="inline-actions">
                            <button class="primary-button" type="button" on:click=move |event| { event.stop_propagation(); announce(read_only, notice, "Conflict review opened.") }>"Review"</button>
                            <button class="secondary-button" type="button" disabled=read_only on:click=move |event| { event.stop_propagation(); announce(read_only, notice, "Direction composer focused.") }><Icon path=ICON_SEND />"Send direction"</button>
                        </div>
                    </div>
                </article>
            </div>

            <Composer
                placeholder="Direct an agent or assign a task…"
                context="HomeTender"
                agent="Claude"
                read_only
                notice
            />
        </div>
    }
}

#[component]
pub fn AttentionInspector(
    inspector_open: RwSignal<bool>,
    read_only: bool,
    notice: RwSignal<Option<String>>,
) -> impl IntoView {
    view! {
        <div class="inspector-content">
            <header class="inspector-header">
                <div class="inspector-identity">
                    <AgentAvatar initials="CL" tone="coral" size="lg" />
                    <div><h2>"Claude"</h2><p>"HomeTender / release-1.2"</p></div>
                </div>
                <button class="icon-button" type="button" aria-label="Close details" on:click=move |_| inspector_open.set(false)><Icon path=ICON_CLOSE /></button>
            </header>

            <section class="inspector-section">
                <div class="inspector-section-title"><h3>"Session health"</h3><span class="state-label healthy"><StatusDot />"Healthy"</span></div>
                <dl class="detail-list">
                    <div><dt>"Last heartbeat"</dt><dd>"18s ago"</dd></div>
                    <div><dt>"Uptime"</dt><dd>"2h 14m"</dd></div>
                    <div><dt>"Evidence"</dt><dd><EvidenceTag kind="Observed" /></dd></div>
                    <div><dt>"Isolation"</dt><dd><Icon path=ICON_SHIELD />"Local sandbox"</dd></div>
                </dl>
            </section>

            <section class="inspector-section">
                <h3>"Latest evidence"</h3>
                <div class="inspector-log">
                    <span>"$ make verify"</span><span>"Running 1,283 tests…"</span><span>"All tests passed"</span><span>"Lint: 0 errors, 3 warnings"</span>
                </div>
            </section>

            <section class="inspector-section">
                <h3>"Artifacts"</h3>
                <button class="artifact-row" type="button"><Icon path=ICON_FILE /><span>"release-notes.md"</span><small>"18s ago"</small></button>
                <button class="artifact-row" type="button"><Icon path=ICON_FILE /><span>"test-report.html"</span><small>"18s ago"</small></button>
                <button class="artifact-row" type="button"><Icon path=ICON_FILE /><span>"coverage.xml"</span><small>"18s ago"</small></button>
            </section>

            <section class="inspector-section">
                <h3>"Cost estimate"</h3>
                <dl class="detail-list">
                    <div><dt>"This run"</dt><dd>"~$0.03"</dd></div>
                    <div><dt>"Confidence"</dt><dd>"Partial"</dd></div>
                </dl>
                <p class="fine-print">"Based on connector-reported usage and configured pricing."</p>
            </section>

            <section class="inspector-section owner-controls">
                <h3>"Owner controls"</h3>
                <div class="control-row">
                    <button type="button" disabled=read_only on:click=move |_| announce(read_only, notice, "Pause requires confirmation in a live session.")><Icon path=ICON_PAUSE />"Pause"</button>
                    <button class="danger-control" type="button" disabled=read_only on:click=move |_| announce(read_only, notice, "Stop requires confirmation in a live session.")><Icon path=ICON_STOP />"Stop"</button>
                </div>
            </section>
        </div>
    }
}

fn announce(read_only: bool, notice: RwSignal<Option<String>>, message: &str) {
    let message = if read_only {
        "The web dashboard is read-only. Use the owner desktop app for controls."
    } else {
        message
    };
    notice.set(Some(message.into()));
}
