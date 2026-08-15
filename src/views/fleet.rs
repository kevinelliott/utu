use leptos::prelude::*;

use crate::components::{
    AgentCliIcon, Composer, EmptyInspectorButton, EvidenceTag, ICON_CHECK, ICON_CLOSE,
    ICON_COMMAND, ICON_FILE, ICON_MORE, ICON_PAUSE, ICON_SHIELD, ICON_STOP, Icon, StatusDot,
};

#[component]
pub fn FleetView(
    inspector_open: RwSignal<bool>,
    read_only: bool,
    notice: RwSignal<Option<String>>,
) -> impl IntoView {
    let session_tab = RwSignal::new("Session");

    view! {
        <div class="workspace-layout fleet-layout">
            <header class="workspace-toolbar fleet-toolbar">
                <div class="toolbar-leading">
                    <AgentCliIcon connector_id="codex" size="lg" />
                    <div class="fleet-heading">
                        <h1>"Codex"<span class="live-status"><StatusDot />"Running 18m"</span></h1>
                        <p>"OpenAI · GPT-5"<span>"Utu / app shell"</span></p>
                    </div>
                </div>
                <div class="toolbar-actions">
                    <button class="secondary-button" type="button" disabled=read_only on:click=move |_| announce(read_only, notice, "Pause requires confirmation in a live session.")><Icon path=ICON_PAUSE />"Pause"</button>
                    <button class="icon-button" type="button" aria-label="Agent actions"><Icon path=ICON_MORE /></button>
                    <button class="secondary-button compact-action" type="button">"All agents"</button>
                    <Show when=move || !inspector_open.get()><EmptyInspectorButton inspector_open /></Show>
                </div>
            </header>

            <div class="session-tabs" role="tablist" aria-label="Agent detail">
                {[
                    ("Session", "Session"),
                    ("Trace", "Trace"),
                    ("Files", "Files"),
                    ("Costs", "Costs"),
                ].into_iter().map(|(id, label)| view! {
                    <button
                        type="button"
                        role="tab"
                        aria-selected=move || (session_tab.get() == id).to_string()
                        class=move || if session_tab.get() == id { "is-active" } else { "" }
                        on:click=move |_| session_tab.set(id)
                    >{label}</button>
                }).collect_view()}
            </div>

            <Show when=move || session_tab.get() == "Session">
                <div class="session-stream">
                    <article class="transcript-item owner-message">
                        <div class="transcript-meta"><strong>"You"</strong><span>"18m"</span><EvidenceTag kind="Observed" /></div>
                        <p>"Add optimistic message sending with rollback and retry. Include unit tests."</p>
                    </article>

                    <article class="transcript-item agent-message">
                        <div class="transcript-meta"><strong>"Codex"</strong><span>"18m"</span><EvidenceTag kind="Inferred" /></div>
                        <p>"I’ll implement optimistic updates in the message store, add rollback on failure, and cover success and retry paths."</p>
                    </article>

                    <article class="transcript-item tool-message">
                        <div class="transcript-meta"><strong>"Codex used tools"</strong><span>"18m"</span><EvidenceTag kind="Observed" /></div>
                        <div class="tool-chips">
                            <span><Icon path=ICON_FILE />"read_file"<small>"src/store/messages.rs"</small></span>
                            <span><Icon path=ICON_FILE />"edit_file"<small>"src/store/messages.rs"</small></span>
                            <span><Icon path=ICON_FILE />"write_file"<small>"tests/message_send.rs"</small></span>
                            <span><Icon path=ICON_COMMAND />"run_command"<small>"cargo test"</small></span>
                        </div>
                    </article>

                    <article class="transcript-item diff-message">
                        <div class="transcript-meta"><strong>"Edited"<span>"src/store/messages.rs"</span></strong><span>"18m"</span><EvidenceTag kind="Observed" /></div>
                        <div class="diff-preview" aria-label="Code diff">
                            <div><span>"142"</span><code>"fn add_message(msg: Message) {"</code></div>
                            <div class="diff-removed"><span>"143"</span><code>"- messages.push(msg);"</code></div>
                            <div class="diff-added"><span>"143"</span><code>"+ messages.push(Message { pending: true, ..msg });"</code></div>
                            <div class="diff-added"><span>"144"</span><code>"+ notify();"</code></div>
                        </div>
                    </article>

                    <article class="transcript-item result-message">
                        <div class="transcript-meta"><strong>"Codex ran tests"</strong><span>"16m"</span><EvidenceTag kind="Observed" /></div>
                        <p class="test-success"><Icon path=ICON_CHECK /><span><strong>"cargo test"</strong><small>"32 passed, 0 failed"</small></span></p>
                    </article>

                    <article class="transcript-item agent-message">
                        <div class="transcript-meta"><strong>"Codex"</strong><span>"16m"</span><EvidenceTag kind="Observed" /></div>
                        <p>"All tests passed. Optimistic sending with rollback and retry is in place."</p>
                    </article>
                </div>
            </Show>

            <Show when=move || session_tab.get() != "Session">
                <div class="secondary-surface">
                    <div class="secondary-surface-icon"><Icon path=ICON_NODES_COMPAT /></div>
                    <h2>{move || session_tab.get()}</h2>
                    <p>{move || match session_tab.get() {
                        "Trace" => "The evidence graph will connect tool calls, files, handoffs, and verification events.",
                        "Files" => "Files observed or modified during this session will appear here with provenance.",
                        "Costs" => "Provider-reported usage and configured rates will be reconciled here without overstating precision.",
                        _ => "",
                    }}</p>
                    <span class="state-label unknown">"Planned connector surface"</span>
                </div>
            </Show>

            <div class="live-accessory"><span class="spinner" aria-hidden="true"></span><strong>"Codex is running tests"</strong><span>"16s elapsed"</span><button type="button" disabled=read_only on:click=move |_| announce(read_only, notice, "Stop requires confirmation in a live session.")><Icon path=ICON_STOP />"Stop"</button></div>
            <Composer placeholder="Direct Codex…" context="Utu" agent="Handoff to…" read_only notice />
        </div>
    }
}

const ICON_NODES_COMPAT: &str = "M5 5h5v5H5zm9 0h5v5h-5zM9 14h6v6H9zM10 8h4m3 2-3 4m-7-4 3 4";

#[component]
pub fn FleetInspector(
    inspector_open: RwSignal<bool>,
    read_only: bool,
    notice: RwSignal<Option<String>>,
) -> impl IntoView {
    view! {
        <div class="inspector-content">
            <header class="inspector-header simple-inspector-header">
                <div><h2>"Agent details"</h2><p>"Codex · demo snapshot"</p></div>
                <button class="icon-button" type="button" aria-label="Close details" on:click=move |_| inspector_open.set(false)><Icon path=ICON_CLOSE /></button>
            </header>

            <section class="inspector-section">
                <h3>"Authentication"</h3>
                <div class="auth-confirmed"><Icon path=ICON_CHECK /><span>"Confirmed"</span><small>"2m ago"</small></div>
            </section>

            <section class="inspector-section">
                <h3>"Runtime"</h3>
                <dl class="detail-list">
                    <div><dt>"CLI version"</dt><dd>"0.6.2"</dd></div>
                    <div><dt>"Provider"</dt><dd>"OpenAI"</dd></div>
                    <div><dt>"Model"</dt><dd>"GPT-5"</dd></div>
                    <div><dt>"Context window"</dt><dd>"Unknown"</dd></div>
                    <div><dt>"Working directory"</dt><dd>"~/Projects/Utu"</dd></div>
                    <div><dt>"Evidence"</dt><dd><EvidenceTag kind="Observed" /></dd></div>
                </dl>
            </section>

            <section class="inspector-section">
                <h3>"Local sandbox"</h3>
                <p class="auth-confirmed"><Icon path=ICON_SHIELD /><span>"Boundary active"</span></p>
                <dl class="detail-list">
                    <div><dt>"Filesystem"</dt><dd>"Workspace only"</dd></div>
                    <div><dt>"Network"</dt><dd>"Allowlist"</dd></div>
                    <div><dt>"Execution"</dt><dd>"Local VM"</dd></div>
                </dl>
            </section>

            <section class="inspector-section capabilities-list">
                <h3>"Capabilities"</h3>
                <span><Icon path=ICON_CHECK />"Read files"</span>
                <span><Icon path=ICON_CHECK />"Write files"</span>
                <span><Icon path=ICON_CHECK />"Run commands"</span>
                <span><Icon path=ICON_CHECK />"Network (allowlist)"</span>
                <span><Icon path=ICON_CHECK />"Handoff"</span>
            </section>

            <section class="inspector-section">
                <h3>"Cost (estimated)"</h3>
                <dl class="detail-list">
                    <div><dt>"Session total"</dt><dd>"~$0.09"</dd></div>
                    <div><dt>"Input"</dt><dd>"~$0.06"</dd></div>
                    <div><dt>"Output"</dt><dd>"~$0.03"</dd></div>
                </dl>
            </section>

            <section class="inspector-section stacked-controls">
                <h3>"Controls"</h3>
                <div class="control-row">
                    <button type="button" disabled=read_only on:click=move |_| announce(read_only, notice, "Pause requires confirmation in a live session.")><Icon path=ICON_PAUSE />"Pause"</button>
                    <button type="button" disabled=read_only on:click=move |_| announce(read_only, notice, "Restart requires confirmation in a live session.")><Icon path=ICON_COMMAND />"Restart"</button>
                </div>
                <button class="danger-control" type="button" disabled=read_only on:click=move |_| announce(read_only, notice, "Stop requires confirmation in a live session.")><Icon path=ICON_STOP />"Stop agent"</button>
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
