use leptos::prelude::*;

use crate::{
    components::{
        AgentAvatar, EvidenceTag, ICON_BRANCH, ICON_CHECK, ICON_CHEVRON_RIGHT, ICON_CLOSE,
        ICON_FILE, ICON_FOLDER, ICON_LOCK, ICON_MORE, ICON_PLUS, ICON_SEND, ICON_SHIELD, ICON_STOP,
        ICON_TERMINAL, Icon, StatusDot, WorkspaceNav,
    },
    workspace_data::{LiveStatus, LoadPhase, WorkspaceAction, WorkspaceActionSink, WorkspaceModel},
};

#[component]
pub fn LiveWorkspaceView(
    inspector_open: RwSignal<bool>,
    notice: RwSignal<Option<String>>,
) -> impl IntoView {
    let live = expect_context::<LiveStatus>();
    let actions = expect_context::<WorkspaceActionSink>();
    let refresh_actions = actions;
    let create_actions = actions;

    view! {
        <div class="workspace-layout conversation-layout live-workspace-layout">
            <header class="workspace-toolbar conversation-toolbar">
                <div class="toolbar-leading">
                    <WorkspaceNav />
                    <div class="conversation-heading">
                        <h1>{move || live.active_project_name().unwrap_or_else(|| "Local workspace".into())}<span class=move || format!("live-status state-{}", live_phase_name(live.phase.get()))><span class=move || format!("status-dot status-{}", live_phase_tone(live.phase.get())) aria-hidden="true"></span>{move || live.phase.get().label()}</span></h1>
                        <p>{move || live_session_label(&live)}</p>
                    </div>
                </div>
                <div class="toolbar-actions conversation-actions">
                    <button class="secondary-button" type="button" on:click=move |_| inspector_open.set(true)><Icon path=ICON_FOLDER />"Files"</button>
                    <button class="icon-button" type="button" aria-label="Workspace actions" title="Workspace actions"><Icon path=ICON_MORE /></button>
                </div>
            </header>

            <div class="session-context-bar live-session-context">
                <span><span class=move || format!("status-dot status-{}", live_phase_tone(live.phase.get())) aria-hidden="true"></span>"Local data from the Utu store"</span>
                <span class="session-context-path"><Icon path=ICON_SHIELD />"Provider-neutral · owner device"</span>
            </div>

            <div class=move || if live.phase.get() == LoadPhase::Ready && live.session_stream.get().is_some_and(|stream| !stream.messages.is_empty()) { "live-workspace-state is-hidden" } else { "live-workspace-state" } role="status">
                <Show when=move || live.phase.get() == LoadPhase::Loading>
                    <span class="spinner live-workspace-spinner"></span>
                    <h2>"Opening your local workspace"</h2>
                    <p>"Reading projects, sessions, agents, and connector state from the owner device."</p>
                </Show>
                <Show when=move || live.phase.get() == LoadPhase::Empty>
                    <span class="live-workspace-glyph"><Icon path=ICON_FOLDER /></span>
                    <h2>"Your local workspace is ready"</h2>
                    <p>"No projects or agent sessions are stored yet. Add a local folder boundary, then create tasks and connect agents when you are ready."</p>
                    <div class="live-empty-actions">
                        <button class="primary-button" type="button" on:click=move |_| create_actions.dispatch(WorkspaceAction::OpenCreateProject)><Icon path=ICON_PLUS />"Add project"</button>
                        <button class="secondary-button" type="button" on:click=move |_| refresh_actions.dispatch(WorkspaceAction::RefreshConnector("all connectors".into()))>"Run connector checks"</button>
                    </div>
                    <small>"No sample project or provider output is shown as live data."</small>
                </Show>
                <Show when=move || live.phase.get() == LoadPhase::Error>
                    <span class="live-workspace-glyph is-problem"><Icon path=ICON_STOP /></span>
                    <h2>"Utu could not open the local workspace"</h2>
                    <p>{move || live.error.get().unwrap_or_else(|| "The native command bridge did not return workspace state.".into())}</p>
                    <button class="secondary-button" type="button" on:click=move |_| live.start()>"Try again"</button>
                </Show>
                <Show when=move || live.phase.get() == LoadPhase::Ready && live.stream_loading.get()>
                    <span class="spinner live-workspace-spinner"></span>
                    <h2>"Loading the stored conversation"</h2>
                    <p>"Messages and normalized activity are read from one coherent native session projection."</p>
                </Show>
                <Show when=move || live.phase.get() == LoadPhase::Ready && !live.stream_loading.get() && live.session_stream.get().is_none_or(|stream| stream.messages.is_empty())>
                    <span class="live-workspace-glyph"><Icon path=ICON_TERMINAL /></span>
                    <h2>{move || if live.recordable_session_id().is_some() { "Stored session selected" } else { "No eligible stored session" }}</h2>
                    <p>{move || if live.selected_session_can_deliver() { "This Codex session can receive an explicitly armed direction. Utu requests provider read-only/no-network policy, does not independently verify enforcement, and never treats acknowledgement as completion." } else if live.recordable_session_id().is_some() { "Utu can record an owner direction locally. This session has no active provider delivery capability." } else { "Select a non-demonstration session to record owner intent locally." }}</p>
                    <button class="text-button" type="button" on:click=move |_| notice.set(Some("Connector capability reports describe adapter support; they do not prove that an active control transport is attached.".into()))>"Review capability evidence"</button>
                </Show>
            </div>

            <Show when=move || live.phase.get() == LoadPhase::Ready && live.session_stream.get().is_some_and(|stream| !stream.messages.is_empty())>
                <LiveConversationStream />
            </Show>

            <Show when=move || live.phase.get() == LoadPhase::Ready>
                <LiveDirectionComposer />
            </Show>
        </div>
    }
}

#[component]
fn LiveConversationStream() -> impl IntoView {
    let live = expect_context::<LiveStatus>();

    view! {
        <div class="conversation-stream live-conversation-stream" aria-label="Stored session conversation" aria-live="polite">
            {move || live.session_stream.get().map(|stream| {
                let agent_name = live.snapshot.get().and_then(|snapshot| {
                    snapshot
                        .agents
                        .iter()
                        .find(|agent| agent.id == stream.session.agent_id)
                        .map(|agent| agent.display_name.clone())
                }).unwrap_or_else(|| "Agent".into());
                stream.messages.iter().cloned().map(|message| {
                    let role = message.role.clone();
                    let (turn_class, avatar_class, avatar, author) = match role.as_str() {
                        "owner" => ("conversation-turn owner-turn live-stored-turn", "turn-avatar owner-avatar-small", "K", "You".to_owned()),
                        "agent" => ("conversation-turn agent-turn live-stored-turn", "turn-avatar agent-avatar-small", "AI", agent_name.clone()),
                        _ => ("conversation-turn system-turn live-stored-turn", "turn-avatar system-avatar-small", "·", "Utu".to_owned()),
                    };
                    let evidence = message.evidence.clone();
                    let source = message.source.clone();
                    view! {
                        <article class=turn_class data-message-id=message.id>
                            <div class=avatar_class aria-hidden="true">{avatar}</div>
                            <div class="turn-content">
                                <div class="turn-meta"><strong>{author}</strong><span>{format!("Message {}", message.sequence)}</span><span class="live-evidence-chip">{evidence}</span></div>
                                <p>{message.body}</p>
                                <small class="live-record-source">{source}</small>
                            </div>
                        </article>
                    }
                }).collect_view()
            })}
            {move || live.session_stream.get().and_then(|stream| (!stream.events.is_empty()).then(|| {
                let event_count = stream.events.len();
                view! {
                    <section class="live-session-events" aria-label="Recent normalized activity">
                        <header><strong>"Recent activity"</strong><span>{event_count}" events"</span></header>
                        {stream.events.iter().rev().take(6).cloned().map(|event| view! {
                            <div class="live-session-event" data-event-id=event.id>
                                <StatusDot tone=if event.kind == "problem" { "problem" } else { "quiet" } />
                                <span><strong>{event.summary}</strong><small>{format!("{} · {} · event {}", event.kind, event.evidence, event.sequence)}</small></span>
                            </div>
                        }).collect_view()}
                    </section>
                }
            }))}
        </div>
    }
}

#[component]
fn LiveDirectionComposer() -> impl IntoView {
    let live = expect_context::<LiveStatus>();
    let actions = expect_context::<WorkspaceActionSink>();
    let draft = RwSignal::new(String::new());
    let deliver_live = RwSignal::new(false);
    Effect::new(move || {
        let _ = live.selected_session_id.get();
        let _ = live.selected_project_id.get();
        let _ = live.selected_session_can_deliver();
        deliver_live.set(false);
    });
    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        let project_id = live.selected_project_id.get_untracked();
        let session_id = live.recordable_session_id();
        if project_id.is_some() && session_id.is_some() && !draft.get_untracked().trim().is_empty()
        {
            actions.dispatch(WorkspaceAction::SubmitPrompt {
                project_id,
                session_id,
                body: draft.get_untracked(),
                allow_provider_delivery: deliver_live.get_untracked()
                    && live.selected_session_can_deliver(),
            });
            deliver_live.set(false);
            draft.set(String::new());
        }
    };

    view! {
        <form class="workspace-composer live-direction-composer" on:submit=submit>
            <label class="sr-only" for="live-workspace-direction">"Direct the selected session"</label>
            <textarea id="live-workspace-direction" rows="2" placeholder=move || if live.selected_session_can_deliver() { "Write a direction for Codex…" } else if live.recordable_session_id().is_some() { "Record an owner direction locally…" } else { "Select a stored session to record owner intent" } prop:value=move || draft.get() on:input=move |event| draft.set(event_target_value(&event))></textarea>
            <div class="live-direction-toolbar">
                <span class="live-direction-target"><span class=move || if live.recordable_session_id().is_some() { "status-dot status-attention" } else { "status-dot status-quiet" } aria-hidden="true"></span>{move || live_direction_target(&live)}</span>
                <Show when=move || live.selected_session_can_deliver()>
                    <label class="live-delivery-arm"><input type="checkbox" prop:checked=move || deliver_live.get() on:change=move |_| deliver_live.update(|armed| *armed = !*armed) /><span>"Send live · request read-only/no-network"</span></label>
                </Show>
                <button class="send-button" type="submit" disabled=move || live.recordable_session_id().is_none() || draft.get().trim().is_empty() || (live.selected_session_can_deliver() && !deliver_live.get()) aria-label=move || if live.selected_session_can_deliver() { "Send explicitly armed direction to Codex" } else { "Record direction in selected local session" }><Icon path=ICON_SEND /></button>
            </div>
        </form>
    }
}

fn live_direction_target(live: &LiveStatus) -> String {
    let Some(snapshot) = live.snapshot.get() else {
        return "Workspace unavailable".into();
    };
    let Some(session_id) = live.recordable_session_id() else {
        return "No eligible session · nothing will be recorded".into();
    };
    snapshot
        .sessions
        .iter()
        .find(|session| session.id == session_id)
        .and_then(|session| {
            snapshot
                .agents
                .iter()
                .find(|agent| agent.id == session.agent_id)
        })
        .map(|agent| {
            if live.selected_session_can_deliver() {
                format!(
                    "Live Codex target · {} · {}",
                    agent.display_name, session_id
                )
            } else {
                format!("Record locally for {} · {}", agent.display_name, session_id)
            }
        })
        .unwrap_or_else(|| format!("Record locally in session {session_id}"))
}

fn live_session_label(live: &LiveStatus) -> String {
    let Some(snapshot) = live.snapshot.get() else {
        return "Waiting for the native workspace snapshot".into();
    };
    let Some(selected) = live.selected_session_id.get() else {
        return format!(
            "{} projects · {} sessions",
            snapshot.projects.len(),
            snapshot.sessions.len()
        );
    };
    snapshot
        .sessions
        .iter()
        .find(|session| session.id == selected)
        .and_then(|session| {
            snapshot
                .agents
                .iter()
                .find(|agent| agent.id == session.agent_id)
        })
        .map(|agent| format!("{} · {}", agent.display_name, selected))
        .unwrap_or_else(|| format!("Selected session · {selected}"))
}

const fn live_phase_name(phase: LoadPhase) -> &'static str {
    match phase {
        LoadPhase::Demo => "demo",
        LoadPhase::Loading => "loading",
        LoadPhase::Ready => "ready",
        LoadPhase::Empty => "empty",
        LoadPhase::Error => "error",
    }
}

const fn live_phase_tone(phase: LoadPhase) -> &'static str {
    match phase {
        LoadPhase::Ready => "healthy",
        LoadPhase::Loading | LoadPhase::Empty => "attention",
        LoadPhase::Error => "problem",
        LoadPhase::Demo => "quiet",
    }
}

#[component]
pub fn WorkspaceView(
    inspector_open: RwSignal<bool>,
    read_only: bool,
    notice: RwSignal<Option<String>>,
) -> impl IntoView {
    let model = expect_context::<WorkspaceModel>();
    let actions = expect_context::<WorkspaceActionSink>();
    let permission_state = RwSignal::new("pending");

    let allow_actions = actions;
    let reject_actions = actions;

    view! {
        <div class="workspace-layout conversation-layout">
            <header class="workspace-toolbar conversation-toolbar">
                <div class="toolbar-leading">
                    <WorkspaceNav />
                    <div class="conversation-heading">
                        <h1>{model.active_work.title}<span class="live-status"><StatusDot />"Working"</span></h1>
                        <p><span>{model.active_work.project}</span><span class="path-separator">"/"</span><span>{model.active_work.branch}</span></p>
                    </div>
                </div>
                <div class="toolbar-actions conversation-actions">
                    <div class="avatar-stack" aria-label="Assigned agents: Codex and Claude">
                        <AgentAvatar initials=model.agents[0].initials tone=model.agents[0].tone size="sm" />
                        <AgentAvatar initials=model.agents[1].initials tone=model.agents[1].tone size="sm" />
                    </div>
                    <button class="secondary-button" type="button" on:click=move |_| inspector_open.set(true)><Icon path=ICON_FOLDER />"Files"<span class="change-count">"5"</span></button>
                    <button class="icon-button" type="button" aria-label="Session actions" title="Session actions"><Icon path=ICON_MORE /></button>
                </div>
            </header>

            <div class="session-context-bar">
                <span class="demo-scope-note"><StatusDot tone="attention" />"Demonstration session — no agent or provider is connected"</span>
                <span class="session-context-path"><Icon path=ICON_BRANCH />{model.active_work.working_directory}</span>
            </div>

            <div class="conversation-stream" aria-label="Demonstration conversation" aria-live="polite">
                <article class="conversation-turn owner-turn">
                    <div class="turn-avatar owner-avatar-small" aria-hidden="true">"K"</div>
                    <div class="turn-content">
                        <div class="turn-meta"><strong>"You"</strong><span>"10:42 AM"</span><span class="assignment-label">"Assigned to Codex + Claude"</span></div>
                        <p>{model.active_work.owner_direction}</p>
                    </div>
                </article>

                <article class="conversation-turn agent-turn">
                    <AgentAvatar initials=model.agents[0].initials tone=model.agents[0].tone size="md" />
                    <div class="turn-content">
                        <div class="turn-meta"><strong>{model.agents[0].name}</strong><span>{model.agents[0].provider}" · "{model.agents[0].model}</span><EvidenceTag kind="Inferred" /></div>
                        <p>{model.active_work.agent_response}</p>

                        <section class="execution-plan" aria-label="Agent plan">
                            <header><strong>"Plan"</strong><span>"2 of 4 complete"</span></header>
                            <ol>
                                {model.plan.iter().copied().map(|step| view! {
                                    <li class=format!("plan-step plan-{}", step.state)>
                                        <span class="plan-state" aria-hidden="true"></span>
                                        <span><strong>{step.label}</strong><small>{step.detail}</small></span>
                                        <span class="plan-status">{step.state}</span>
                                    </li>
                                }).collect_view()}
                            </ol>
                        </section>

                        <details class="tool-call" open=true>
                            <summary><span><Icon path=ICON_TERMINAL /><strong>"4 tool calls"</strong><small>"Observed demonstration output"</small></span><Icon path=ICON_CHEVRON_RIGHT /></summary>
                            <div class="tool-activity-list">
                                {model.tools.iter().copied().map(|tool| view! {
                                    <div><span class="tool-kind">{tool.tool}</span><code>{tool.target}</code><span>{tool.result}</span></div>
                                }).collect_view()}
                            </div>
                        </details>

                        <section class=move || if permission_state.get() == "pending" { "permission-request" } else { "permission-request is-resolved" } aria-label="Permission request">
                                <div class="permission-icon"><Icon path=ICON_LOCK /></div>
                                <div class="permission-copy">
                                    <strong>"Permission required"</strong>
                                    <p>"Run a focused test command in the workspace sandbox?"</p>
                                    <code>{model.active_work.permission_command}</code>
                                    <span><Icon path=ICON_SHIELD />"Workspace files · network blocked · demo only"</span>
                                </div>
                                <div class="permission-actions">
                                    <button class="primary-button" type="button" disabled=move || read_only || permission_state.get() != "pending" on:click=move |_| { permission_state.set("allowed"); allow_actions.dispatch(WorkspaceAction::ResolvePermission("allow once")); }>"Allow once"</button>
                                    <button class="secondary-button" type="button" disabled=move || read_only || permission_state.get() != "pending" on:click=move |_| { permission_state.set("rejected"); reject_actions.dispatch(WorkspaceAction::ResolvePermission("reject")); }>"Reject"</button>
                                </div>
                                <Show when=move || permission_state.get() != "pending">
                                    <div class=move || format!("permission-resolution resolution-{}", permission_state.get()) role="status">
                                        <Icon path=if permission_state.get() == "allowed" { ICON_CHECK } else { ICON_STOP } />
                                        <span>{move || if permission_state.get() == "allowed" { "Allowed once in demo — no command executed" } else { "Rejected in demo — no command executed" }}</span>
                                    </div>
                                </Show>
                        </section>

                        <section class="inline-diff" aria-label="Demonstration code changes">
                            <header><span><Icon path=ICON_FILE /><strong>"readiness.rs"</strong></span><span class="diff-summary"><span>"+3"</span><span>"−1"</span></span></header>
                            <div class="diff-preview">
                                {model.diff.iter().copied().map(|line| view! {
                                    <div class=format!("diff-{}", line.kind)><span>{line.number}</span><code>{line.content}</code></div>
                                }).collect_view()}
                            </div>
                        </section>

                        <div class="verification-row">
                            <span class="verification-icon"><Icon path=ICON_CHECK /></span>
                            <span><strong>"Formatting passed"</strong><small>"cargo fmt --check · observed demo event"</small></span>
                            <EvidenceTag kind="Observed" />
                        </div>
                    </div>
                </article>

                <article class="conversation-turn handoff-turn">
                    <AgentAvatar initials=model.agents[1].initials tone=model.agents[1].tone size="md" />
                    <div class="turn-content">
                        <div class="turn-meta"><strong>{model.agents[1].name}</strong><span>"Queued reviewer"</span><EvidenceTag kind="Stale" /></div>
                        <p>"I’ll review the permission scope, auth recovery, and every claim derived from connector evidence after Codex completes verification."</p>
                    </div>
                </article>

                <div class="streaming-row" role="status">
                    <AgentAvatar initials=model.agents[0].initials tone=model.agents[0].tone size="sm" />
                    <span>{model.active_work.streaming_activity}</span>
                    <span class="streaming-dots" aria-hidden="true"><i></i><i></i><i></i></span>
                    <button type="button" class="text-button" disabled=read_only on:click=move |_| notice.set(Some("Stop requires confirmation when a live connector owns the session.".into()))><Icon path=ICON_STOP />"Stop"</button>
                </div>
            </div>

            <WorkspaceComposer read_only />
        </div>
    }
}

#[component]
fn WorkspaceComposer(read_only: bool) -> impl IntoView {
    let model = expect_context::<WorkspaceModel>();
    let actions = expect_context::<WorkspaceActionSink>();
    let live = expect_context::<LiveStatus>();
    let draft = RwSignal::new(String::new());
    let codex = RwSignal::new(true);
    let claude = RwSignal::new(true);
    let local = RwSignal::new(false);

    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        actions.dispatch(WorkspaceAction::SubmitPrompt {
            project_id: live.selected_project_id.get_untracked(),
            session_id: live.recordable_session_id(),
            body: draft.get(),
            allow_provider_delivery: false,
        });
    };
    let codex_actions = actions;
    let claude_actions = actions;
    let local_actions = actions;

    view! {
        <form class="workspace-composer" class:is-read-only=read_only on:submit=submit>
            <label class="sr-only" for="workspace-direction">"Direct the assigned agents"</label>
            <textarea
                id="workspace-direction"
                rows="2"
                placeholder=if read_only { "Open the owner app to direct agents…" } else if live.recordable_session_id().is_some() { "Record owner intent in the selected session…" } else { "No eligible stored session" }
                readonly=read_only
                prop:value=move || draft.get()
                on:input=move |event| draft.set(event_target_value(&event))
            ></textarea>
            <div class="composer-assignment-row">
                <span class="composer-field-label">"Assign"</span>
                <button type="button" class=move || assignment_class(codex.get()) aria-pressed=move || codex.get().to_string() disabled=read_only on:click=move |_| { codex.update(|value| *value = !*value); dispatch_selected_agents(&codex_actions, codex.get(), claude.get(), local.get()); }><AgentAvatar initials=model.agents[0].initials tone=model.agents[0].tone size="xs" />"Codex"</button>
                <button type="button" class=move || assignment_class(claude.get()) aria-pressed=move || claude.get().to_string() disabled=read_only on:click=move |_| { claude.update(|value| *value = !*value); dispatch_selected_agents(&claude_actions, codex.get(), claude.get(), local.get()); }><AgentAvatar initials=model.agents[1].initials tone=model.agents[1].tone size="xs" />"Claude"</button>
                <button type="button" class=move || assignment_class(local.get()) aria-pressed=move || local.get().to_string() disabled=read_only on:click=move |_| { local.update(|value| *value = !*value); dispatch_selected_agents(&local_actions, codex.get(), claude.get(), local.get()); }><Icon path=ICON_PLUS />"Agent"</button>
            </div>
            <div class="workspace-composer-toolbar">
                <div class="composer-options">
                    <label><span class="sr-only">"Model routing"</span><select disabled=read_only aria-label="Model routing"><option>"Auto route"</option><option>"Pinned models"</option></select></label>
                    <label><span class="sr-only">"Context scope"</span><select disabled=read_only aria-label="Context scope"><option>"Project context"</option><option>"Session only"</option><option>"Selected files"</option></select></label>
                    <label><span class="sr-only">"Isolation"</span><select disabled=read_only aria-label="Isolation"><option>"Workspace sandbox"</option><option>"Local VM"</option><option>"Remote VM"</option></select></label>
                </div>
                <div class="composer-submit-group">
                    <span>{move || if live.recordable_session_id().is_some() { "⌘ ↵ to record" } else { "No stored session" }}</span>
                    <button class="send-button" type="submit" disabled=read_only aria-label="Send direction"><Icon path=ICON_SEND /></button>
                </div>
            </div>
        </form>
    }
}

#[component]
pub fn WorkspaceInspector(inspector_open: RwSignal<bool>) -> impl IntoView {
    let model = expect_context::<WorkspaceModel>();
    let actions = expect_context::<WorkspaceActionSink>();
    let live = expect_context::<LiveStatus>();
    let panel = RwSignal::new("files");
    let selected_file = RwSignal::new(model.files[0].path.to_owned());

    view! {
        <div class="inspector-content file-inspector">
            <header class="inspector-header simple-inspector-header">
                <div><h2>"Workspace detail"</h2><p>"Demonstration session"</p></div>
                <button class="icon-button" type="button" aria-label="Close workspace detail" on:click=move |_| inspector_open.set(false)><Icon path=ICON_CLOSE /></button>
            </header>
            <div class="inspector-tabs" role="tablist" aria-label="Workspace detail">
                <button type="button" role="tab" aria-selected=move || (panel.get() == "files").to_string() class=move || tab_class(panel.get() == "files") on:click=move |_| panel.set("files")>"Files"<span>"5"</span></button>
                <button type="button" role="tab" aria-selected=move || (panel.get() == "activity").to_string() class=move || tab_class(panel.get() == "activity") on:click=move |_| panel.set("activity")>"Activity"</button>
                <button type="button" role="tab" aria-selected=move || (panel.get() == "evidence").to_string() class=move || tab_class(panel.get() == "evidence") on:click=move |_| panel.set("evidence")>"Evidence"</button>
            </div>

            <Show when=move || panel.get() == "files">
                <div class="file-panel">
                    <div class="file-tree" role="tree" aria-label="Changed files">
                        <div class="file-tree-heading"><span><Icon path=ICON_FOLDER />"hometender"</span><span class="diff-summary"><span>"+102"</span><span>"−11"</span></span></div>
                        {model.files.iter().copied().map(|file| {
                            view! {
                                <button class=move || if selected_file.get() == file.path { "file-tree-row is-selected" } else { "file-tree-row" } type="button" role="treeitem" aria-selected=move || (selected_file.get() == file.path).to_string() on:click=move |_| { selected_file.set(file.path.into()); actions.dispatch(WorkspaceAction::SelectFile(file.path.into())); }>
                                    <Icon path=ICON_FILE /><span><strong>{file.path}</strong><small>{file.state}</small></span><span class="file-delta">"+"{file.additions}" −"{file.removals}</span>
                                </button>
                            }
                        }).collect_view()}
                    </div>
                    <Show when=move || live.is_desktop()>
                        <div class="live-file-tree" role="tree" aria-label="Live project files">
                            <div class="file-tree-heading"><span><Icon path=ICON_FOLDER />"Local project"</span><span class="live-source-label">"Live"</span></div>
                            <Show when=move || live.project_directory.get().is_some() fallback=move || view! { <div class="context-loading-state"><span class="spinner"></span><span>"Loading project files"</span></div> }>
                                {move || live.project_directory.get().map(|directory| directory.entries.iter().take(80).map(|entry| {
                                    let path = entry.relative_path.clone();
                                    let click_path = path.clone();
                                    let project_id = live.selected_project_id.get();
                                    view! {
                                        <button class="file-tree-row" type="button" role="treeitem" disabled=entry.kind != "file" on:click=move |_| {
                                            selected_file.set(click_path.clone());
                                            if let Some(project_id) = project_id.clone() {
                                                live.load_file_preview(project_id, click_path.clone());
                                            }
                                        }><Icon path=if entry.kind == "directory" { ICON_FOLDER } else { ICON_FILE } /><span><strong>{entry.name.clone()}</strong><small>{entry.kind.clone()}</small></span><span class="file-delta">{entry.size_bytes.map(format_bytes).unwrap_or_default()}</span></button>
                                    }
                                }).collect_view())}
                            </Show>
                        </div>
                    </Show>
                    <div class="file-preview">
                        <header><span><strong>{move || selected_file.get().rsplit('/').next().unwrap_or_default().to_owned()}</strong><small>"Demonstration preview"</small></span><button class="icon-button mini" type="button" aria-label="File actions"><Icon path=ICON_MORE /></button></header>
                        <Show when=move || live.file_preview.get().is_some()>
                            {move || live.file_preview.get().map(|preview| {
                                let relative_path = preview.relative_path.clone();
                                let binary = preview.binary;
                                let size = preview.size_bytes;
                                let content = StoredValue::new(preview.content.clone().unwrap_or_default());
                                view! {
                                    <div class="live-preview"><header><strong>{relative_path}</strong><span class="live-source-label">"Live file"</span></header><Show when=move || !binary fallback=move || view! { <div class="preview-empty"><Icon path=ICON_FILE /><strong>"Binary file"</strong><span>{format_bytes(size)}</span></div> }><pre>{move || content.get_value()}</pre></Show></div>
                                }
                            })}
                        </Show>
                        <Show
                            when=move || selected_file.get() == model.files[0].path
                            fallback=move || view! { <div class="preview-empty"><Icon path=ICON_FILE /><strong>"Preview not included"</strong><span>"This demo only includes source content for readiness.rs."</span></div> }
                        >
                            <div class="code-preview" aria-label="File preview">
                                <div><span>"46"</span><code>"#[derive(Debug, Clone)]"</code></div>
                                <div><span>"47"</span><code>"pub enum ReadinessEvidence {"</code></div>
                                <div class="code-highlight"><span>"48"</span><code>"    Observed(ProbeEvidence),"</code></div>
                                <div class="code-highlight"><span>"49"</span><code>"    Inferred { reason: String },"</code></div>
                                <div class="code-highlight"><span>"50"</span><code>"    Unsupported,"</code></div>
                                <div><span>"51"</span><code>"}"</code></div>
                            </div>
                        </Show>
                    </div>
                </div>
            </Show>

            <Show when=move || panel.get() == "activity">
                <div class="activity-panel">
                    {model.tools.iter().copied().map(|tool| view! {
                        <div class="activity-event"><span class="activity-glyph"><Icon path=if tool.tool == "command" { ICON_TERMINAL } else { ICON_FILE } /></span><span><strong>{tool.tool}</strong><code>{tool.target}</code></span><small>{tool.result}</small></div>
                    }).collect_view()}
                    <div class="context-empty panel-empty"><p>"No background jobs"</p><span>"New tool activity will append here."</span></div>
                </div>
            </Show>

            <Show when=move || panel.get() == "evidence">
                <div class="evidence-panel">
                    <section class="inspector-section">
                        <h3>"Session evidence"</h3>
                        <dl class="detail-list"><div><dt>"Latest output"</dt><dd><EvidenceTag kind="Observed" /></dd></div><div><dt>"Agent intent"</dt><dd><EvidenceTag kind="Inferred" /></dd></div><div><dt>"Cloud state"</dt><dd>"Unsupported"</dd></div></dl>
                    </section>
                    <section class="inspector-section"><h3>"Estimated cost"</h3><div class="cost-emphasis"><strong>{model.active_work.estimated_cost}</strong><small>{model.active_work.token_usage}</small></div></section>
                    <section class="inspector-section"><h3>"Isolation"</h3><div class="surface-row"><Icon path=ICON_SHIELD /><span><strong>"Workspace sandbox"</strong><small>"Filesystem scoped · network blocked"</small></span><span class="state-label healthy"><StatusDot />"Demo"</span></div></section>
                </div>
            </Show>
        </div>
    }
}

#[component]
pub fn LiveWorkspaceInspector(inspector_open: RwSignal<bool>) -> impl IntoView {
    let live = expect_context::<LiveStatus>();

    view! {
        <div class="inspector-content file-inspector live-file-inspector">
            <header class="inspector-header simple-inspector-header">
                <div><h2>"Local project files"</h2><p>{move || live.active_project_name().unwrap_or_else(|| "No project selected".into())}</p></div>
                <button class="icon-button" type="button" aria-label="Close workspace detail" on:click=move |_| inspector_open.set(false)><Icon path=ICON_CLOSE /></button>
            </header>
            <div class="inspector-live-source"><span class=move || if live.project_directory.get().is_some() { "status-dot status-healthy" } else if live.selected_project_id.get().is_some() { "status-dot status-attention" } else { "status-dot status-quiet" } aria-hidden="true"></span><span><strong>{move || if live.project_directory.get().is_some() { "Project files loaded" } else if live.selected_project_id.get().is_some() { "Loading project boundary" } else { "No project boundary" }}</strong><small>{move || if live.project_directory.get().is_some() { "Directory and previews are read through Utu's scoped local commands." } else if live.selected_project_id.get().is_some() { "Waiting for a successful scoped directory response." } else { "Select a stored project before requesting local files." }}</small></span></div>
            <div class="file-panel live-only-file-panel">
                <div class="live-file-tree" role="tree" aria-label="Local project files">
                    <div class="file-tree-heading"><span><Icon path=ICON_FOLDER />{move || live.project_directory.get().map(|directory| if directory.relative_path.is_empty() { "Project root".into() } else { directory.relative_path.clone() }).unwrap_or_else(|| "Local project".into())}</span><span class="live-source-label">"Live"</span></div>
                    <Show when=move || live.selected_project_id.get().is_some() fallback=move || view! { <div class="preview-empty"><Icon path=ICON_FOLDER /><strong>"No project selected"</strong><span>"Add or select a local project before browsing files."</span></div> }>
                        <Show when=move || live.project_directory.get().is_some() fallback=move || view! { <div class="context-loading-state"><span class="spinner"></span><span>"Loading project files"</span></div> }>
                            {move || live.project_directory.get().map(|directory| directory.entries.iter().take(80).map(|entry| {
                                let click_path = entry.relative_path.clone();
                                let is_file = entry.kind == "file";
                                view! {
                                    <button class="file-tree-row" type="button" role="treeitem" disabled=!is_file on:click=move |_| {
                                        if let Some(project_id) = live.selected_project_id.get_untracked() {
                                            live.load_file_preview(project_id, click_path.clone());
                                        }
                                    }><Icon path=if is_file { ICON_FILE } else { ICON_FOLDER } /><span><strong>{entry.name.clone()}</strong><small>{entry.kind.clone()}</small></span><span class="file-delta">{entry.size_bytes.map(format_bytes).unwrap_or_default()}</span></button>
                                }
                            }).collect_view())}
                        </Show>
                    </Show>
                </div>
                <div class="file-preview live-file-preview">
                    <Show when=move || live.file_preview.get().is_some() fallback=move || view! { <div class="preview-empty"><Icon path=ICON_FILE /><strong>"Select a file"</strong><span>"Text previews remain on this device and are capped at 256 KB."</span></div> }>
                        {move || live.file_preview.get().map(|preview| {
                            let path = preview.relative_path.clone();
                            let binary = preview.binary;
                            let size = preview.size_bytes;
                            let truncated = preview.truncated;
                            let content = StoredValue::new(preview.content.clone().unwrap_or_default());
                            view! {
                                <header><span><strong>{path}</strong><small>{if truncated { "Live preview · truncated" } else { "Live preview" }}</small></span></header>
                                <Show when=move || !binary fallback=move || view! { <div class="preview-empty"><Icon path=ICON_FILE /><strong>"Binary file"</strong><span>{format_bytes(size)}</span></div> }>
                                    <pre>{move || content.get_value()}</pre>
                                </Show>
                            }
                        })}
                    </Show>
                </div>
            </div>
        </div>
    }
}

fn assignment_class(selected: bool) -> &'static str {
    if selected {
        "agent-assignment is-selected"
    } else {
        "agent-assignment"
    }
}

fn tab_class(active: bool) -> &'static str {
    if active { "is-active" } else { "" }
}

fn dispatch_selected_agents(actions: &WorkspaceActionSink, codex: bool, claude: bool, local: bool) {
    let mut selected = Vec::new();
    if codex {
        selected.push("Codex");
    }
    if claude {
        selected.push("Claude");
    }
    if local {
        selected.push("Local reviewer");
    }
    actions.dispatch(WorkspaceAction::AssignAgents(selected));
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1_024 {
        format!("{:.1} KB", bytes as f64 / 1_024.0)
    } else {
        format!("{bytes} B")
    }
}
