use leptos::{prelude::*, task::spawn_local};

use crate::{
    components::{
        AgentAvatar, AgentCliIcon, AppMarkGlyph, DemoBadge, ICON_ATTENTION, ICON_CLOSE,
        ICON_COMMAND, ICON_COST, ICON_FOLDER, ICON_HOME, ICON_NODES, ICON_ORBIT, ICON_PLUG,
        ICON_PLUS, ICON_SEARCH, ICON_SETTINGS, Icon, StatusDot,
    },
    ipc,
    views::{
        attention::{AttentionInspector, AttentionView},
        costs::CostsView,
        fleet::{FleetInspector, FleetView},
        integrations::{IntegrationsInspector, IntegrationsView, LiveIntegrationsInspector},
        overview::LiveOverviewView,
        projects::{ProjectInspector, ProjectsView},
        settings::SettingsView,
        workspace::{LiveWorkspaceInspector, LiveWorkspaceView, WorkspaceInspector, WorkspaceView},
    },
    workspace_data::{
        LiveStatus, LoadPhase, WorkspaceAction, WorkspaceActionSink, WorkspaceModel,
        demo_action_notice, relative_unix_ms, session_detail, session_is_running,
        session_state_tone, session_title,
    },
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AppView {
    #[default]
    Workspace,
    Overview,
    Attention,
    Projects,
    Fleet,
    Integrations,
    Settings,
    Costs,
}

impl AppView {
    const fn label(self) -> &'static str {
        match self {
            Self::Workspace => "Workspace",
            Self::Overview => "Overview",
            Self::Attention => "Attention",
            Self::Projects => "Projects",
            Self::Fleet => "Fleet",
            Self::Integrations => "Integrations",
            Self::Settings => "Settings",
            Self::Costs => "Costs",
        }
    }

    const fn subtitle(self, read_only: bool) -> &'static str {
        if read_only {
            return "Read-only demo status";
        }
        match self {
            Self::Workspace => "Projects and sessions",
            Self::Overview => "Active agent sessions",
            Self::Attention => "Decisions and problems",
            Self::Projects => "Work across outcomes",
            Self::Fleet => "Running agent sessions",
            Self::Integrations => "Connectors and capabilities",
            Self::Settings => "Local owner configuration",
            Self::Costs => "Observed and unobserved spend",
        }
    }

    const fn titlebar(self) -> &'static str {
        match self {
            Self::Workspace => "HomeTender · Release handoff",
            _ => self.label(),
        }
    }
}

#[component]
pub fn App() -> impl IntoView {
    let active_view = RwSignal::new(AppView::Workspace);
    let inspector_open = RwSignal::new(false);
    let context_open = RwSignal::new(false);
    let notice = RwSignal::new(None::<String>);
    let project_creator_open = RwSignal::new(false);
    let task_creator_open = RwSignal::new(false);
    let task_creator_project_id = RwSignal::new(None::<String>);
    let surface = crate::ipc::RuntimeSurface::detect();
    let read_only = !surface.is_desktop();
    let live = LiveStatus::new(surface);
    provide_context(WorkspaceModel::demo());
    provide_context(live);
    provide_context(active_view);
    crate::theme::ThemeController::install();
    let action_live = live;
    provide_context(WorkspaceActionSink(Callback::new(move |action| {
        match &action {
            WorkspaceAction::SelectProject(project_id) if action_live.is_desktop() => {
                action_live.project_directory.set(None);
                action_live.file_preview.set(None);
                action_live.selected_file_path.set(None);
                action_live
                    .selected_project_id
                    .set(Some(project_id.clone()));
                let session_id = action_live.snapshot.get_untracked().and_then(|snapshot| {
                    snapshot
                        .sessions
                        .iter()
                        .filter(|session| session.project_id == *project_id)
                        .find(|session| snapshot.session_can_receive_direction(&session.id))
                        .or_else(|| {
                            snapshot
                                .sessions
                                .iter()
                                .find(|session| session.project_id == *project_id)
                        })
                        .map(|session| session.id.clone())
                });
                action_live.selected_session_id.set(session_id.clone());
                if let Some(session_id) = session_id {
                    action_live.load_session_stream(session_id);
                } else {
                    action_live.requested_stream_session_id.set(None);
                    action_live.session_stream.set(None);
                    action_live.stream_loading.set(false);
                }
                action_live.load_project_directory(project_id.clone(), None);
                return;
            }
            WorkspaceAction::SelectSession(session_id) if action_live.is_desktop() => {
                if let Some(project_id) =
                    action_live.snapshot.get_untracked().and_then(|snapshot| {
                        snapshot
                            .sessions
                            .iter()
                            .find(|session| session.id == *session_id)
                            .map(|session| session.project_id.clone())
                    })
                    && action_live.selected_project_id.get_untracked().as_deref()
                        != Some(project_id.as_str())
                {
                    action_live
                        .selected_project_id
                        .set(Some(project_id.clone()));
                    action_live.file_preview.set(None);
                    action_live.selected_file_path.set(None);
                    action_live.load_project_directory(project_id, None);
                }
                action_live
                    .selected_session_id
                    .set(Some(session_id.clone()));
                action_live.load_session_stream(session_id.clone());
                return;
            }
            WorkspaceAction::SubmitPrompt {
                project_id,
                session_id,
                body,
                allow_provider_delivery,
            } if action_live.is_desktop() => {
                action_live.send_direction(
                    project_id.clone(),
                    session_id.clone(),
                    body.clone(),
                    *allow_provider_delivery,
                );
                return;
            }
            WorkspaceAction::RefreshConnector(_) if action_live.is_desktop() => {
                action_live.refresh_connectors();
                return;
            }
            WorkspaceAction::SelectFile(path) if action_live.is_desktop() => {
                if let Some(project_id) = action_live.selected_project_id.get_untracked() {
                    action_live.load_file_preview(project_id, path.clone());
                }
                return;
            }
            WorkspaceAction::OpenCreateProject if action_live.is_desktop() => {
                action_live.project_create_error.set(None);
                project_creator_open.set(true);
                return;
            }
            WorkspaceAction::OpenCreateTask(project_id) if action_live.is_desktop() => {
                let snapshot = action_live.snapshot.get_untracked();
                let in_workbench = snapshot.as_ref().is_some_and(|snapshot| {
                    snapshot
                        .projects
                        .iter()
                        .any(|project| project.id == *project_id)
                });
                let ignored = snapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.project_is_ignored(project_id));
                if in_workbench {
                    action_live.task_create_error.set(None);
                    task_creator_project_id.set(Some(project_id.clone()));
                    task_creator_open.set(true);
                } else if ignored {
                    notice.set(Some(
                        "Show this project in the workbench before creating a task.".into(),
                    ));
                } else {
                    notice.set(Some(
                        "Select a stored project before creating a task.".into(),
                    ));
                }
                return;
            }
            WorkspaceAction::CreateProject { name, root_path } if action_live.is_desktop() => {
                let close_creator = Callback::new(move |_| project_creator_open.set(false));
                action_live.create_project(name.clone(), root_path.clone(), close_creator);
                return;
            }
            WorkspaceAction::SetProjectIgnored {
                project_id,
                ignored,
            } if action_live.is_desktop() => {
                action_live.set_project_ignored(project_id.clone(), *ignored);
                return;
            }
            WorkspaceAction::CreateTask {
                project_id,
                title,
                detail,
                assignee_agent_ids,
            } if action_live.is_desktop() => {
                let close_creator = Callback::new(move |_| task_creator_open.set(false));
                action_live.create_task(
                    project_id.clone(),
                    title.clone(),
                    detail.clone(),
                    assignee_agent_ids.clone(),
                    close_creator,
                );
                return;
            }
            WorkspaceAction::ConfigureConnector(connector_id) if action_live.is_desktop() => {
                action_live
                    .selected_connector_id
                    .set(Some(connector_id.clone()));
                return;
            }
            WorkspaceAction::SyncProjectSessions { project_id } if action_live.is_desktop() => {
                action_live.sync_project_sessions(project_id.clone());
                return;
            }
            _ => {}
        }
        let message = demo_action_notice(&action, read_only);
        if message.is_empty() {
            notice.set(None);
        } else {
            notice.set(Some(message));
        }
    })));
    live.start();
    Effect::new(move || {
        if let Some(message) = live.error.get() {
            notice.set(Some(message));
        }
    });
    let has_context_rail = Signal::derive(move || {
        !matches!(
            active_view.get(),
            AppView::Integrations | AppView::Settings | AppView::Costs
        )
    });
    view! {
        <div class="app-frame">
            <header class="native-titlebar" data-tauri-drag-region="">
                <div class="titlebar-leading" data-tauri-drag-region="">
                    <span class="titlebar-product" data-tauri-drag-region="">"Utu"</span>
                </div>
                <span class="titlebar-view" data-tauri-drag-region="">{move || {
                    if active_view.get() == AppView::Workspace {
                        live.active_project_name().map(|project| format!("{project} · Workspace")).unwrap_or_else(|| if live.is_desktop() { "Workspace".into() } else { active_view.get().titlebar().into() })
                    } else {
                        active_view.get().titlebar().into()
                    }
                }}</span>
                <Show when=move || live.is_desktop() fallback=move || view! { <DemoBadge web=true /> }>
                    <span class="titlebar-agent-status" title=move || titlebar_status_label(&live)>
                        <span class=move || format!("status-dot status-{}", agent_system_tone(&live)) aria-hidden="true"></span>
                    </span>
                </Show>
            </header>

            <div
                class="app-shell"
                class:inspector-is-open=move || inspector_open.get()
                class:context-is-open=move || context_open.get()
                class:has-context=move || has_context_rail.get()
            >
                <UtilityRail active_view context_open />
                <Show when=move || has_context_rail.get()>
                    <ContextRail active_view read_only />
                </Show>

                <section class="work-surface" aria-label="Utu workspace">
                    <div class="mobile-workspace-switch">
                        <button
                            class="icon-button"
                            type="button"
                            aria-label="Open context"
                            on:click=move |_| context_open.update(|open| *open = !*open)
                        ><Icon path=ICON_COMMAND /></button>
                        <span class="mobile-view-title">{move || active_view.get().label()}</span>
                    </div>

                    <Show when=move || active_view.get() == AppView::Workspace>
                        <Show when=move || live.is_desktop() fallback=move || view! { <WorkspaceView inspector_open read_only notice /> }>
                            <LiveWorkspaceView inspector_open on_review_evidence=Callback::new(move |_| active_view.set(AppView::Integrations)) />
                        </Show>
                    </Show>
                    <Show when=move || active_view.get() == AppView::Overview>
                        <Show when=move || live.is_desktop() fallback=move || view! { <div class="overview-surface"><div class="overview-state"><span class="status-dot status-attention" aria-hidden="true"></span><p>"Overview requires the native owner app."</p></div></div> }>
                            <LiveOverviewView />
                        </Show>
                    </Show>
                    <Show when=move || active_view.get() == AppView::Attention>
                        <Show when=move || live.is_desktop() fallback=move || view! { <AttentionView inspector_open read_only notice /> }>
                            <LiveCollectionView kind="attention" inspector_open />
                        </Show>
                    </Show>
                    <Show when=move || active_view.get() == AppView::Projects>
                        <Show when=move || live.is_desktop() fallback=move || view! { <ProjectsView inspector_open read_only notice /> }>
                            <LiveProjectsView inspector_open />
                        </Show>
                    </Show>
                    <Show when=move || active_view.get() == AppView::Fleet>
                        <Show when=move || live.is_desktop() fallback=move || view! { <FleetView inspector_open read_only notice /> }>
                            <LiveFleetView inspector_open />
                        </Show>
                    </Show>
                    <Show when=move || active_view.get() == AppView::Integrations>
                        <IntegrationsView inspector_open read_only notice />
                    </Show>
                    <Show when=move || active_view.get() == AppView::Settings>
                        <SettingsView />
                    </Show>
                    <Show when=move || active_view.get() == AppView::Costs>
                        <CostsView />
                    </Show>
                </section>

                <Show when=move || inspector_open.get()>
                    <aside class="inspector" aria-label="Selection details">
                        <Show when=move || active_view.get() == AppView::Workspace>
                            <Show when=move || live.is_desktop() fallback=move || view! { <WorkspaceInspector inspector_open /> }>
                                <LiveWorkspaceInspector inspector_open />
                            </Show>
                        </Show>
                        <Show when=move || active_view.get() == AppView::Attention>
                            <Show when=move || live.is_desktop() fallback=move || view! { <AttentionInspector inspector_open read_only notice /> }>
                                <LiveCollectionInspector kind="attention" inspector_open />
                            </Show>
                        </Show>
                        <Show when=move || active_view.get() == AppView::Projects>
                            <Show when=move || live.is_desktop() fallback=move || view! { <ProjectInspector inspector_open read_only notice /> }>
                                <LiveCollectionInspector kind="projects" inspector_open />
                            </Show>
                        </Show>
                        <Show when=move || active_view.get() == AppView::Fleet>
                            <Show when=move || live.is_desktop() fallback=move || view! { <FleetInspector inspector_open read_only notice /> }>
                                <LiveCollectionInspector kind="fleet" inspector_open />
                            </Show>
                        </Show>
                        <Show when=move || active_view.get() == AppView::Integrations>
                            <Show when=move || live.is_desktop() fallback=move || view! { <IntegrationsInspector inspector_open read_only notice /> }>
                                <LiveIntegrationsInspector inspector_open />
                            </Show>
                        </Show>
                        <Show when=move || matches!(active_view.get(), AppView::Overview | AppView::Settings | AppView::Costs)>
                            <div class="inspector-content">
                                <header class="inspector-header simple-inspector-header">
                                    <div><h2>{move || active_view.get().label()}</h2><p>"Details"</p></div>
                                    <button class="icon-button" type="button" aria-label="Close details" on:click=move |_| inspector_open.set(false)><Icon path=ICON_CLOSE /></button>
                                </header>
                                <section class="inspector-section"><p class="inspector-note">{move || active_view.get().subtitle(read_only)}</p></section>
                            </div>
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

            <Show when=move || project_creator_open.get()>
                <CreateProjectSheet open=project_creator_open />
            </Show>
            <Show when=move || task_creator_open.get() && task_creator_project_id.get().is_some()>
                <CreateTaskSheet open=task_creator_open project_id=task_creator_project_id />
            </Show>
        </div>
    }
}

#[component]
fn CreateProjectSheet(open: RwSignal<bool>) -> impl IntoView {
    let live = expect_context::<LiveStatus>();
    let actions = expect_context::<WorkspaceActionSink>();
    let name = RwSignal::new(String::new());
    let root_path = RwSignal::new(String::new());
    let validation_error = RwSignal::new(None::<String>);

    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        let current_name = name.get_untracked();
        let current_root = root_path.get_untracked();
        if let Some(error) = validate_project_input(&current_name, &current_root) {
            validation_error.set(Some(error));
            return;
        }
        validation_error.set(None);
        actions.dispatch(WorkspaceAction::CreateProject {
            name: current_name.trim().to_owned(),
            root_path: current_root.trim().to_owned(),
        });
    };

    view! {
        <div
            class="native-sheet-layer"
            role="presentation"
            on:keydown=move |event| {
                if event.key() == "Escape" && !live.project_creating.get_untracked() {
                    open.set(false);
                }
            }
        >
            <section class="native-sheet project-create-sheet" role="dialog" aria-modal="true" aria-labelledby="create-project-title">
                <header class="native-sheet-header">
                    <span class="sheet-icon" aria-hidden="true"><Icon path=ICON_FOLDER /></span>
                    <span><h2 id="create-project-title">"Add a local project"</h2><p>"Give Utu a durable name and folder boundary."</p></span>
                    <button class="icon-button" type="button" aria-label="Close project creator" disabled=move || live.project_creating.get() on:click=move |_| open.set(false)><Icon path=ICON_CLOSE /></button>
                </header>

                <form class="native-sheet-form" on:submit=submit>
                    <label class="native-form-field" for="project-name">
                        <span>"Project name"</span>
                        <input
                            id="project-name"
                            type="text"
                            maxlength="120"
                            autocomplete="off"
                            autofocus=true
                            placeholder="Utu"
                            disabled=move || live.project_creating.get()
                            prop:value=move || name.get()
                            on:input=move |event| {
                                name.set(event_target_value(&event));
                                validation_error.set(None);
                                live.project_create_error.set(None);
                            }
                        />
                    </label>

                    <label class="native-form-field" for="project-root">
                        <span>"Local root folder"</span>
                        <div class="path-input-row">
                            <input
                                id="project-root"
                                class="path-input"
                                type="text"
                                maxlength="4096"
                                autocomplete="off"
                                spellcheck="false"
                                placeholder="/Users/you/Projects/utu"
                                disabled=move || live.project_creating.get()
                                prop:value=move || root_path.get()
                                on:input=move |event| {
                                    root_path.set(event_target_value(&event));
                                    validation_error.set(None);
                                    live.project_create_error.set(None);
                                }
                            />
                            <button
                                class="secondary-button path-browse-button"
                                type="button"
                                disabled=move || live.project_creating.get()
                                on:click=move |_| {
                                    let root_path = root_path;
                                    let validation_error = validation_error;
                                    spawn_local(async move {
                                        match ipc::pick_folder().await {
                                            Ok(Some(path)) => {
                                                root_path.set(path);
                                                validation_error.set(None);
                                                live.project_create_error.set(None);
                                            }
                                            Ok(None) => {}
                                            Err(error) => {
                                                validation_error.set(Some(format!(
                                                    "Folder picker failed: {error}"
                                                )));
                                            }
                                        }
                                    });
                                }
                            >"Browse…"</button>
                        </div>
                        <small>"Use an absolute path to a folder already on this device. Utu resolves and verifies it natively."</small>
                    </label>

                    <Show when=move || validation_error.get().is_some() || live.project_create_error.get().is_some()>
                        <div class="native-form-error" role="alert"><Icon path=ICON_ATTENTION /><span>{move || validation_error.get().or_else(|| live.project_create_error.get()).unwrap_or_default()}</span></div>
                    </Show>

                    <footer class="native-sheet-actions">
                        <button class="secondary-button" type="button" disabled=move || live.project_creating.get() on:click=move |_| open.set(false)>"Cancel"</button>
                        <button class="primary-button" type="submit" disabled=move || live.project_creating.get()>
                            <Show when=move || live.project_creating.get() fallback=move || view! { <><Icon path=ICON_PLUS />"Add project"</> }>
                                <span class="spinner"></span>"Adding project…"
                            </Show>
                        </button>
                    </footer>
                </form>
            </section>
        </div>
    }
}

#[component]
fn CreateTaskSheet(open: RwSignal<bool>, project_id: RwSignal<Option<String>>) -> impl IntoView {
    let live = expect_context::<LiveStatus>();
    let actions = expect_context::<WorkspaceActionSink>();
    let title = RwSignal::new(String::new());
    let detail = RwSignal::new(String::new());
    let selected_agents = RwSignal::new(Vec::<String>::new());
    let validation_error = RwSignal::new(None::<String>);

    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        let Some(current_project_id) = project_id.get_untracked() else {
            validation_error.set(Some(
                "Select a stored project before creating a task.".into(),
            ));
            return;
        };
        let current_title = title.get_untracked();
        let current_detail = detail.get_untracked();
        if let Some(error) = validate_task_input(&current_title, &current_detail) {
            validation_error.set(Some(error));
            return;
        }
        validation_error.set(None);
        actions.dispatch(WorkspaceAction::CreateTask {
            project_id: current_project_id,
            title: current_title.trim().to_owned(),
            detail: current_detail.trim().to_owned(),
            assignee_agent_ids: selected_agents.get_untracked(),
        });
    };

    view! {
        <div
            class="native-sheet-layer"
            role="presentation"
            on:keydown=move |event| {
                if event.key() == "Escape" && !live.task_creating.get_untracked() {
                    open.set(false);
                }
            }
        >
            <section class="native-sheet task-create-sheet" role="dialog" aria-modal="true" aria-labelledby="create-task-title">
                <header class="native-sheet-header">
                    <span class="sheet-icon" aria-hidden="true"><Icon path=ICON_PLUS /></span>
                    <span><h2 id="create-task-title">"Create a task"</h2><p>{move || task_project_label(&live, project_id.get())}</p></span>
                    <button class="icon-button" type="button" aria-label="Close task creator" disabled=move || live.task_creating.get() on:click=move |_| open.set(false)><Icon path=ICON_CLOSE /></button>
                </header>

                <form class="native-sheet-form" on:submit=submit>
                    <label class="native-form-field" for="task-title">
                        <span>"Task title"</span>
                        <input
                            id="task-title"
                            type="text"
                            maxlength="200"
                            autocomplete="off"
                            autofocus=true
                            placeholder="Prepare the release handoff"
                            disabled=move || live.task_creating.get()
                            prop:value=move || title.get()
                            on:input=move |event| {
                                title.set(event_target_value(&event));
                                validation_error.set(None);
                                live.task_create_error.set(None);
                            }
                        />
                    </label>

                    <label class="native-form-field" for="task-detail">
                        <span>"Details "<em>"Optional"</em></span>
                        <textarea
                            id="task-detail"
                            rows="3"
                            maxlength="8000"
                            placeholder="Outcome, constraints, and evidence the agents should preserve."
                            disabled=move || live.task_creating.get()
                            prop:value=move || detail.get()
                            on:input=move |event| {
                                detail.set(event_target_value(&event));
                                validation_error.set(None);
                                live.task_create_error.set(None);
                            }
                        ></textarea>
                    </label>

                    <fieldset class="agent-assignment-field" disabled=move || live.task_creating.get()>
                        <legend><span>"Assign agents"</span><small>{move || assignment_count_label(selected_agents.get().len())}</small></legend>
                        <div class="agent-assignment-list">
                            {move || live.snapshot.get().map(|snapshot| snapshot.agents.iter().map(|agent| {
                                let agent_id = agent.id.clone();
                                let checked_id = agent_id.clone();
                                let toggle_id = agent_id.clone();
                                let initials = agent.display_name.chars().filter(|character| character.is_alphanumeric()).take(2).collect::<String>().to_uppercase();
                                let name = agent.display_name.clone();
                                let detail = agent.model.clone().unwrap_or_else(|| agent.connector_id.clone());
                                view! {
                                    <label class=move || if selected_agents.get().contains(&checked_id) { "agent-assignment-row is-selected" } else { "agent-assignment-row" }>
                                        <input type="checkbox" prop:checked=move || selected_agents.get().contains(&agent_id) on:change=move |_| toggle_agent(&selected_agents, &toggle_id) />
                                        <span class="agent-assignment-avatar">{initials}</span>
                                        <span><strong>{name}</strong><small>{detail}</small></span>
                                        <span class="assignment-check" aria-hidden="true"></span>
                                    </label>
                                }
                            }).collect_view())}
                            <Show when=move || live.snapshot.get().is_none_or(|snapshot| snapshot.agents.is_empty())>
                                <div class="agent-assignment-empty"><strong>"No stored agents yet"</strong><span>"The task will remain unassigned. Run connector checks, then edit the task when an agent is available."</span></div>
                            </Show>
                        </div>
                    </fieldset>

                    <Show when=move || validation_error.get().is_some() || live.task_create_error.get().is_some()>
                        <div class="native-form-error" role="alert"><Icon path=ICON_ATTENTION /><span>{move || validation_error.get().or_else(|| live.task_create_error.get()).unwrap_or_default()}</span></div>
                    </Show>

                    <footer class="native-sheet-actions">
                        <span class="native-sheet-footnote">"Saved locally as a draft"</span>
                        <button class="secondary-button" type="button" disabled=move || live.task_creating.get() on:click=move |_| open.set(false)>"Cancel"</button>
                        <button class="primary-button" type="submit" disabled=move || live.task_creating.get()>
                            <Show when=move || live.task_creating.get() fallback=move || view! { <><Icon path=ICON_PLUS />"Create task"</> }>
                                <span class="spinner"></span>"Creating task…"
                            </Show>
                        </button>
                    </footer>
                </form>
            </section>
        </div>
    }
}

fn validate_project_input(name: &str, root_path: &str) -> Option<String> {
    let name = name.trim();
    let root_path = root_path.trim();
    if name.is_empty() {
        return Some("Enter a project name.".into());
    }
    if name.chars().count() > 120 {
        return Some("Keep the project name to 120 characters or fewer.".into());
    }
    if root_path.is_empty() {
        return Some("Enter the absolute path to the project folder on this device.".into());
    }
    if root_path.chars().count() > 4096 || root_path.contains('\0') {
        return Some(
            "The project folder path is too long or contains an invalid character.".into(),
        );
    }
    if !looks_like_absolute_path(root_path) {
        return Some(
            "Use an absolute folder path, such as /Users/you/Projects/utu or C:\\Users\\you\\Projects\\utu."
                .into(),
        );
    }
    None
}

fn validate_task_input(title: &str, detail: &str) -> Option<String> {
    if title.trim().is_empty() {
        return Some("Enter a task title.".into());
    }
    if title.trim().chars().count() > 200 {
        return Some("Keep the task title to 200 characters or fewer.".into());
    }
    if detail.trim().chars().count() > 8000 {
        return Some("Keep task details to 8,000 characters or fewer.".into());
    }
    None
}

fn looks_like_absolute_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    path.starts_with('/')
        || path.starts_with("\\\\")
        || (bytes.len() >= 3
            && bytes[1] == b':'
            && (bytes[2] == b'\\' || bytes[2] == b'/')
            && bytes[0].is_ascii_alphabetic())
}

fn toggle_agent(selected_agents: &RwSignal<Vec<String>>, agent_id: &str) {
    selected_agents.update(|agents| {
        if let Some(index) = agents.iter().position(|selected| selected == agent_id) {
            agents.remove(index);
        } else {
            agents.push(agent_id.to_owned());
        }
    });
}

fn assignment_count_label(count: usize) -> String {
    match count {
        0 => "Unassigned".into(),
        1 => "1 agent".into(),
        count => format!("{count} agents"),
    }
}

fn task_project_label(live: &LiveStatus, project_id: Option<String>) -> String {
    let Some(project_id) = project_id else {
        return "No project selected".into();
    };
    live.snapshot
        .get()
        .and_then(|snapshot| {
            snapshot
                .projects
                .iter()
                .find(|project| project.id == project_id)
                .map(|project| format!("{} · local draft", project.name))
        })
        .unwrap_or_else(|| "Selected local project".into())
}

#[component]
fn LiveCollectionView(kind: &'static str, inspector_open: RwSignal<bool>) -> impl IntoView {
    let live = expect_context::<LiveStatus>();
    let (title, subtitle, empty_title, empty_detail) = live_collection_copy(kind);

    view! {
        <div class="workspace-layout live-collection-layout">
            <header class="workspace-toolbar">
                <div class="toolbar-leading"><div><h1>{title}</h1><p>{subtitle}</p></div></div>
                <button class="secondary-button" type="button" on:click=move |_| inspector_open.set(true)>"Details"</button>
            </header>
            <div class="truth-banner live-truth-banner" role="note">
                <Icon path=ICON_ATTENTION />
                <span><strong>"Local store projection"</strong>"Only records returned by the native Utu store are shown here. No sample agent, task, sandbox, or provider state is mixed in."</span>
            </div>
            <div class="live-collection-content">
                <Show when=move || live.phase.get() == crate::workspace_data::LoadPhase::Loading>
                    <div class="live-collection-empty"><span class="spinner"></span><strong>"Loading local records"</strong><small>"Waiting for the native workspace snapshot."</small></div>
                </Show>
                <Show when=move || live.phase.get() == crate::workspace_data::LoadPhase::Error>
                    <div class="live-collection-empty"><span class="live-workspace-glyph is-problem"><Icon path=ICON_ATTENTION /></span><strong>"Local records are unavailable"</strong><small>{move || live.error.get().unwrap_or_else(|| "The native workspace snapshot failed its integrity or loading checks.".into())}</small></div>
                </Show>
                <Show when=move || matches!(live.phase.get(), crate::workspace_data::LoadPhase::Empty | crate::workspace_data::LoadPhase::Ready) && live_collection_rows(&live, kind).is_empty()>
                    <div class="live-collection-empty"><span class="live-workspace-glyph"><Icon path=ICON_FOLDER /></span><strong>{empty_title}</strong><small>{empty_detail}</small></div>
                </Show>
                <div class="live-record-list">
                    {move || live_collection_rows(&live, kind).into_iter().map(|(_, name, detail, state)| view! {
                        <article class="live-record-row"><span class="live-record-mark"><StatusDot tone="quiet" /></span><span><strong>{name}</strong><small>{detail}</small></span><span class="state-label quiet">{state}</span></article>
                    }).collect_view()}
                </div>
            </div>
        </div>
    }
}

#[component]
fn LiveProjectsView(inspector_open: RwSignal<bool>) -> impl IntoView {
    let live = expect_context::<LiveStatus>();
    let actions = expect_context::<WorkspaceActionSink>();
    let active_view = expect_context::<RwSignal<AppView>>();
    let pending_ignore_id = RwSignal::new(None::<String>);

    view! {
        <div class="workspace-layout live-collection-layout live-projects-layout">
            <header class="workspace-toolbar">
                <div class="toolbar-leading"><div><h1>"Projects"</h1><p>"Select a project to list its agent sessions"</p></div></div>
                <div class="toolbar-actions">
                    <button class="secondary-button" type="button" on:click=move |_| inspector_open.set(true)>"Details"</button>
                    <button class="secondary-button" type="button" on:click=move |_| actions.dispatch(WorkspaceAction::OpenCreateProject)><Icon path=ICON_PLUS />"Add project"</button>
                    <button
                        class="primary-button"
                        type="button"
                        disabled=move || {
                            live.phase.get() == LoadPhase::Error
                                || selected_workbench_project_id(&live).is_none()
                        }
                        on:click=move |_| {
                            if let Some(project_id) = selected_workbench_project_id(&live) {
                                actions.dispatch(WorkspaceAction::OpenCreateTask(project_id));
                            }
                        }
                    ><Icon path=ICON_PLUS />"New task"</button>
                </div>
            </header>
            <div class="truth-banner live-truth-banner" role="note">
                <Icon path=ICON_ATTENTION />
                <span><strong>"Local work records"</strong>"Discovery still collects every session it finds. Ignore hides a project from the workbench without deleting stored sessions."</span>
            </div>
            <div class="live-projects-content">
                <Show when=move || live.phase.get() == LoadPhase::Loading>
                    <div class="live-collection-empty"><span class="spinner"></span><strong>"Loading local projects"</strong><small>"Waiting for the native workspace snapshot."</small></div>
                </Show>
                <Show when=move || live.phase.get() == LoadPhase::Error>
                    <div class="live-collection-empty"><span class="live-workspace-glyph is-problem"><Icon path=ICON_ATTENTION /></span><strong>"Local projects are unavailable"</strong><small>{move || live.error.get().unwrap_or_else(|| "The native workspace snapshot failed its safety checks.".into())}</small></div>
                </Show>
                <Show when=move || matches!(live.phase.get(), LoadPhase::Empty | LoadPhase::Ready) && live.snapshot.get().is_some_and(|snapshot| snapshot.projects.is_empty() && snapshot.ignored_projects.is_empty())>
                    <div class="live-collection-empty live-projects-empty">
                        <span class="live-workspace-glyph"><Icon path=ICON_FOLDER /></span>
                        <strong>"Start with a local project"</strong>
                        <small>"Add a folder boundary, then create tasks and assign any stored agents."</small>
                        <button class="primary-button" type="button" on:click=move |_| actions.dispatch(WorkspaceAction::OpenCreateProject)><Icon path=ICON_PLUS />"Add project"</button>
                    </div>
                </Show>
                <Show when=move || matches!(live.phase.get(), LoadPhase::Empty | LoadPhase::Ready) && live.snapshot.get().is_some_and(|snapshot| snapshot.projects.is_empty() && !snapshot.ignored_projects.is_empty())>
                    <div class="live-collection-empty live-projects-empty">
                        <span class="live-workspace-glyph"><Icon path=ICON_FOLDER /></span>
                        <strong>"No projects in the workbench"</strong>
                        <small>"Discovery still collects sessions. Ignored projects stay listed below."</small>
                        <button class="primary-button" type="button" on:click=move |_| actions.dispatch(WorkspaceAction::OpenCreateProject)><Icon path=ICON_PLUS />"Add project"</button>
                    </div>
                </Show>
                <div class="live-project-groups">
                    {move || live.snapshot.get().map(|snapshot| {
                        let selected = live.selected_project_id.get();
                        snapshot.projects.iter().map(|project| {
                        let project_id = project.id.clone();
                        let select_id = project_id.clone();
                        let task_id = project_id.clone();
                        let project_name = project.name.clone();
                        let root_path = project.root_path.clone().unwrap_or_else(|| "No local root recorded".into());
                        let project_state = project.state.clone();
                        let is_selected = selected.as_deref() == Some(project_id.as_str());
                        let session_count = snapshot
                            .sessions
                            .iter()
                            .filter(|session| session.project_id == project_id)
                            .count();
                        let mut sessions = if is_selected {
                            snapshot
                                .sessions
                                .iter()
                                .filter(|session| session.project_id == project_id)
                                .cloned()
                                .collect::<Vec<_>>()
                        } else {
                            Vec::new()
                        };
                        sort_sessions_for_display(&mut sessions);
                        let tasks = if is_selected {
                            snapshot.tasks.iter().filter(|task| task.project_id == project_id).cloned().collect::<Vec<_>>()
                        } else {
                            Vec::new()
                        };
                        let task_count = tasks.len();
                        let agents = snapshot.agents.clone();
                        let empty_task_id = StoredValue::new(project_id.clone());
                        let empty_sync_id = StoredValue::new(project_id.clone());
                        let ignore_id = project_id.clone();
                        let confirm_ignore_id = project_id.clone();
                        let is_pending_ignore = pending_ignore_id.get().as_deref() == Some(project_id.as_str());
                        view! {
                            <section class=if is_selected { "live-project-group is-selected" } else { "live-project-group" }>
                                <header class="live-project-group-header">
                                    <button class="live-project-select" type="button" on:click=move |_| { pending_ignore_id.set(None); actions.dispatch(WorkspaceAction::SelectProject(select_id.clone())); }>
                                        <span class="live-project-mark"><Icon path=ICON_FOLDER /></span>
                                        <span><strong>{project_name}</strong><small>{root_path}</small></span>
                                        <span class="live-project-summary"><span class="state-label quiet">{project_state}</span><small>{session_count}" sessions"</small></span>
                                    </button>
                                    <div class="live-project-header-actions">
                                        <button class="secondary-button compact-project-action" type="button" on:click=move |_| { actions.dispatch(WorkspaceAction::SelectProject(ignore_id.clone())); pending_ignore_id.set(Some(ignore_id.clone())); }>"Ignore"</button>
                                        <button class="secondary-button compact-project-action" type="button" on:click=move |_| actions.dispatch(WorkspaceAction::OpenCreateTask(task_id.clone()))><Icon path=ICON_PLUS />"Task"</button>
                                    </div>
                                </header>
                                {if is_selected && is_pending_ignore {
                                    view! {
                                        <div class="live-ignore-confirm">
                                            <p>"Ignoring hides this project from Projects, Fleet, Costs, Attention, and Overview. Collected sessions stay in the local store. Discovery will keep collecting."</p>
                                            <div class="live-ignore-confirm-actions">
                                                <button class="secondary-button compact-project-action" type="button" on:click=move |_| pending_ignore_id.set(None)>"Cancel"</button>
                                                <button class="secondary-button compact-project-action" type="button" on:click=move |_| { pending_ignore_id.set(None); actions.dispatch(WorkspaceAction::SetProjectIgnored { project_id: confirm_ignore_id.clone(), ignored: true }); }>"Ignore"</button>
                                            </div>
                                        </div>
                                    }.into_any()
                                } else if is_selected {
                                    view! {
                                        <div class="live-task-list">
                                            {sessions.into_iter().map(|session| {
                                                let session_id = session.id.clone();
                                                let selected_id = session_id.clone();
                                                let title = session_title(&snapshot, &session);
                                                let detail = session_detail(&snapshot, &session, false);
                                                let state = session.state.clone();
                                                let tone = session_state_tone(&session.state);
                                                view! {
                                                    <button
                                                        class=move || if live.selected_session_id.get().as_deref() == Some(selected_id.as_str()) { "live-session-row is-selected" } else { "live-session-row" }
                                                        type="button"
                                                        on:click=move |_| {
                                                            actions.dispatch(WorkspaceAction::SelectSession(session_id.clone()));
                                                            active_view.set(AppView::Workspace);
                                                        }
                                                    >
                                                        <span class="live-task-state"><StatusDot tone /></span>
                                                        <span><strong>{title}</strong><small>{detail}</small></span>
                                                        <span class=move || format!("state-label {tone}")>{state}</span>
                                                    </button>
                                                }
                                            }).collect_view()}
                                            {tasks.into_iter().map(|task| {
                                                let assignment = task_assignment_label(&agents, &task.assignee_agent_ids);
                                                view! {
                                                    <article class="live-task-row">
                                                        <span class="live-task-state"><StatusDot tone=if task.assignee_agent_ids.is_empty() { "quiet" } else { "healthy" } /></span>
                                                        <span><strong>{task.title}</strong><small>{assignment}</small></span>
                                                        <span class="state-label quiet">{task.state}</span>
                                                    </article>
                                                }
                                            }).collect_view()}
                                            <Show when=move || session_count == 0 && task_count == 0>
                                                <div class="live-task-empty">
                                                    <span>"No sessions in this project"</span>
                                                    <button type="button" on:click=move |_| actions.dispatch(WorkspaceAction::SyncProjectSessions { project_id: Some(empty_sync_id.get_value()) })>"Sync sessions"</button>
                                                    <button type="button" on:click=move |_| actions.dispatch(WorkspaceAction::OpenCreateTask(empty_task_id.get_value()))>"Create a task"</button>
                                                </div>
                                            </Show>
                                        </div>
                                    }.into_any()
                                } else {
                                    view! { <></> }.into_any()
                                }}
                            </section>
                        }
                    }).collect_view()})}
                </div>
                {move || live.snapshot.get().filter(|snapshot| !snapshot.ignored_projects.is_empty()).map(|snapshot| {
                    let selected = live.selected_project_id.get();
                    let ignored_count = snapshot.ignored_projects.len();
                    view! {
                        <section class="live-ignored-projects" aria-label="Ignored projects">
                            <div class="section-label"><span>"Ignored"</span><span class="count">{ignored_count}</span></div>
                            <p class="live-ignored-note">"Hidden from the workbench. Collected sessions stay stored. Discovery still runs."</p>
                            {snapshot.ignored_projects.iter().map(|project| {
                                let project_id = project.id.clone();
                                let select_id = project_id.clone();
                                let unignore_id = project_id.clone();
                                let project_name = project.name.clone();
                                let root_path = project.root_path.clone().unwrap_or_else(|| "No local root recorded".into());
                                let is_selected = selected.as_deref() == Some(project_id.as_str());
                                view! {
                                    <section class=if is_selected { "live-project-group live-ignored-group is-selected" } else { "live-project-group live-ignored-group" }>
                                        <header class="live-project-group-header">
                                            <button class="live-project-select" type="button" on:click=move |_| { pending_ignore_id.set(None); actions.dispatch(WorkspaceAction::SelectProject(select_id.clone())); }>
                                                <span class="live-project-mark"><Icon path=ICON_FOLDER /></span>
                                                <span><strong>{project_name}</strong><small>{root_path}</small></span>
                                                <span class="live-project-summary"><span class="state-label quiet">"Ignored"</span></span>
                                            </button>
                                            <button class="secondary-button compact-project-action" type="button" on:click=move |_| actions.dispatch(WorkspaceAction::SetProjectIgnored { project_id: unignore_id.clone(), ignored: false })>"Show"</button>
                                        </header>
                                        {if is_selected {
                                            view! {
                                                <div class="live-ignored-detail">
                                                    <p>"This project is hidden from operating lists. Collected sessions stay in the local store."</p>
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! { <></> }.into_any()
                                        }}
                                    </section>
                                }
                            }).collect_view()}
                        </section>
                    }
                })}
            </div>
        </div>
    }
}

fn selected_workbench_project_id(live: &LiveStatus) -> Option<String> {
    let project_id = live.selected_project_id.get()?;
    let snapshot = live.snapshot.get()?;
    if snapshot.project_is_ignored(&project_id) {
        None
    } else {
        Some(project_id)
    }
}

fn task_assignment_label(agents: &[crate::ipc::AgentRecord], assignee_ids: &[String]) -> String {
    if assignee_ids.is_empty() {
        return "Unassigned local draft".into();
    }
    let names = assignee_ids
        .iter()
        .map(|agent_id| {
            agents
                .iter()
                .find(|agent| agent.id == *agent_id)
                .map(|agent| agent.display_name.clone())
                .unwrap_or_else(|| "Unknown agent".into())
        })
        .collect::<Vec<_>>();
    format!("Assigned to {}", names.join(" + "))
}

#[component]
fn LiveFleetView(inspector_open: RwSignal<bool>) -> impl IntoView {
    let live = expect_context::<LiveStatus>();
    let actions = expect_context::<WorkspaceActionSink>();
    let active_view = expect_context::<RwSignal<AppView>>();

    // Flat pre-sorted item list — only recomputed when snapshot changes, not on scroll.
    let fleet_items = Signal::derive(move || {
        live.snapshot
            .get()
            .map(|s| build_fleet_items(&s))
            .unwrap_or_default()
    });

    // Tracks scroll position of the `.live-collection-content` div.
    let scroll_top = RwSignal::new(0.0f64);

    view! {
        <div class="workspace-layout live-collection-layout">
            <header class="workspace-toolbar">
                <div class="toolbar-leading"><div><h1>"Fleet"</h1><p>{move || fleet_subtitle(&live)}</p></div></div>
                <button class="secondary-button" type="button" on:click=move |_| inspector_open.set(true)>"Details"</button>
            </header>
            <div class="truth-banner live-truth-banner" role="note">
                <Icon path=ICON_ATTENTION />
                <span><strong>"Observed agent sessions"</strong>"Sessions from workbench projects are listed. Running sessions stay at the top; idle history remains visible."</span>
            </div>
            <div
                class="live-collection-content"
                on:scroll=move |ev| {
                    use wasm_bindgen::JsCast;
                    if let Some(t) = ev.current_target() {
                        scroll_top.set(t.unchecked_into::<web_sys::Element>().scroll_top() as f64);
                    }
                }
            >
                <Show when=move || live.phase.get() == LoadPhase::Loading>
                    <div class="live-collection-empty"><span class="spinner"></span><strong>"Loading agent sessions"</strong><small>"Waiting for the native workspace snapshot."</small></div>
                </Show>
                <Show when=move || live.phase.get() == LoadPhase::Error>
                    <div class="live-collection-empty"><span class="live-workspace-glyph is-problem"><Icon path=ICON_ATTENTION /></span><strong>"Agent sessions are unavailable"</strong><small>{move || live.error.get().unwrap_or_else(|| "The native workspace snapshot failed its safety checks.".into())}</small></div>
                </Show>
                <Show when=move || matches!(live.phase.get(), LoadPhase::Empty | LoadPhase::Ready) && live.snapshot.get().is_some_and(|snapshot| snapshot.sessions.is_empty())>
                    <div class="live-collection-empty"><span class="live-workspace-glyph"><Icon path=ICON_NODES /></span><strong>"No agent sessions yet"</strong><small>"Sync for All on Integrations imports Claude Code and Codex session metadata."</small></div>
                </Show>
                // Virtual list rendered directly inside the scroll container.
                {move || {
                    let items = fleet_items.get();
                    let total = items.len();
                    if total == 0 {
                        return view! { <></> }.into_any();
                    }
                    const ROW_H: f64 = 60.0;
                    const OVERSCAN: usize = 6;
                    let top = scroll_top.get();
                    let vp = web_sys::window()
                        .and_then(|w| w.inner_height().ok())
                        .and_then(|v| v.as_f64())
                        .unwrap_or(700.0);
                    let start = ((top / ROW_H) as usize).saturating_sub(OVERSCAN);
                    let end   = (((top + vp) / ROW_H).ceil() as usize + OVERSCAN).min(total);
                    let top_px = start as f64 * ROW_H;
                    let bot_px = total.saturating_sub(end) as f64 * ROW_H;
                    view! {
                        <div class="live-record-list">
                            <div style=format!("height:{top_px}px;flex-shrink:0")></div>
                            {items.into_iter().skip(start).take(end - start).map(|item| {
                                match item {
                                    FleetItem::GroupHeader { label, tone, count } => view! {
                                        <div class="live-fleet-group-header">
                                            <span class=format!("state-label {tone}")>{label}</span>
                                            <small>{count}</small>
                                        </div>
                                    }.into_any(),
                                    FleetItem::RunningEmpty => view! {
                                        <div class="live-task-empty"><span>"No agents are running right now"</span></div>
                                    }.into_any(),
                                    FleetItem::Session { id, title, detail, state, row_tone, connector_id } => {
                                        let session_id = id.clone();
                                        let selected_id = id.clone();
                                        view! {
                                            <button
                                                class=move || if live.selected_session_id.get().as_deref() == Some(selected_id.as_str()) {
                                                    "live-record-row live-session-row is-selected"
                                                } else {
                                                    "live-record-row live-session-row"
                                                }
                                                type="button"
                                                on:click=move |_| {
                                                    actions.dispatch(WorkspaceAction::SelectSession(session_id.clone()));
                                                    inspector_open.set(true);
                                                    active_view.set(AppView::Workspace);
                                                }
                                            >
                                                <span class="live-record-mark">
                                                    <AgentCliIcon connector_id=connector_id.unwrap_or_default() size="sm" />
                                                </span>
                                                <span><strong>{title}</strong><small>{detail}</small></span>
                                                <span class=move || format!("state-label {row_tone}")>{state}</span>
                                            </button>
                                        }.into_any()
                                    },
                                }
                            }).collect_view()}
                            <div style=format!("height:{bot_px}px;flex-shrink:0")></div>
                        </div>
                    }.into_any()
                }}
            </div>
        </div>
    }
}

fn fleet_subtitle(live: &LiveStatus) -> String {
    let Some(snapshot) = live.snapshot.get() else {
        return "Observed agent sessions".into();
    };
    let running = snapshot
        .sessions
        .iter()
        .filter(|session| session_is_running(session))
        .count();
    format!("{running} running · {} observed", snapshot.sessions.len())
}

fn sort_sessions_for_display(sessions: &mut [crate::ipc::SessionRecord]) {
    sessions.sort_by(|left, right| {
        fleet_session_rank(&left.state)
            .cmp(&fleet_session_rank(&right.state))
            .then(
                right
                    .last_observed_at_unix_ms
                    .unwrap_or(0)
                    .cmp(&left.last_observed_at_unix_ms.unwrap_or(0)),
            )
    });
}

fn fleet_session_groups(
    sessions: &[crate::ipc::SessionRecord],
) -> Vec<(&'static str, &'static str, Vec<crate::ipc::SessionRecord>)> {
    let mut running = Vec::new();
    let mut waiting = Vec::new();
    let mut problem = Vec::new();
    let mut idle = Vec::new();
    for session in sessions {
        match session.state.as_str() {
            "running" => running.push(session.clone()),
            "waiting" => waiting.push(session.clone()),
            "problem" => problem.push(session.clone()),
            _ => idle.push(session.clone()),
        }
    }
    sort_sessions_for_display(&mut running);
    sort_sessions_for_display(&mut waiting);
    sort_sessions_for_display(&mut problem);
    sort_sessions_for_display(&mut idle);
    if sessions.is_empty() {
        return Vec::new();
    }
    let mut groups = vec![("Running", "healthy", running)];
    if !waiting.is_empty() {
        groups.push(("Waiting", "attention", waiting));
    }
    if !problem.is_empty() {
        groups.push(("Problems", "problem", problem));
    }
    if !idle.is_empty() {
        groups.push(("Idle", "quiet", idle));
    }
    groups
}

fn fleet_session_rank(state: &str) -> u8 {
    match state {
        "running" => 0,
        "waiting" => 1,
        "problem" => 2,
        "idle" => 3,
        _ => 4,
    }
}

// Flat list item for the virtual Fleet view.
#[derive(Clone)]
enum FleetItem {
    GroupHeader {
        label: &'static str,
        tone: &'static str,
        count: usize,
    },
    RunningEmpty,
    Session {
        id: String,
        title: String,
        detail: String,
        state: String,
        row_tone: &'static str,
        connector_id: Option<String>,
    },
}

fn build_fleet_items(snapshot: &crate::ipc::WorkspaceSnapshot) -> Vec<FleetItem> {
    let mut sessions = snapshot.sessions.clone();
    sort_sessions_for_display(&mut sessions);
    let groups = fleet_session_groups(&sessions);
    let mut items = Vec::new();
    for (label, tone, group_sessions) in groups {
        let count = group_sessions.len();
        items.push(FleetItem::GroupHeader { label, tone, count });
        if label == "Running" && group_sessions.is_empty() {
            items.push(FleetItem::RunningEmpty);
        } else {
            for s in group_sessions {
                let connector_id = snapshot
                    .agents
                    .iter()
                    .find(|a| a.id == s.agent_id)
                    .map(|a| a.connector_id.clone());
                items.push(FleetItem::Session {
                    id: s.id.clone(),
                    title: session_title(snapshot, &s),
                    detail: session_detail(snapshot, &s, false),
                    state: s.state.clone(),
                    row_tone: session_state_tone(&s.state),
                    connector_id,
                });
            }
        }
    }
    items
}

#[component]
fn LiveCollectionContext(kind: &'static str) -> impl IntoView {
    let live = expect_context::<LiveStatus>();
    let actions = expect_context::<WorkspaceActionSink>();
    let (title, _, empty_title, _) = live_collection_copy(kind);
    view! {
        <div class="context-section live-context-section">
            {move || {
                let rows = if kind == "fleet" {
                    live_collection_rows(&live, kind)
                        .into_iter()
                        .filter(|(_, _, _, state)| {
                            matches!(state.as_str(), "running" | "waiting" | "problem")
                        })
                        .collect::<Vec<_>>()
                } else {
                    live_collection_rows(&live, kind)
                };
                let count = rows.len();
                let empty = rows.is_empty();
                view! {
                    <>
                        <div class="section-label"><span>{title}</span><span class="count">{count}</span></div>
                        {rows.into_iter().map(|(id, name, detail, state)| {
                            let select_id = id.clone();
                            let selected_id = id.clone();
                            let tone = session_state_tone(&state);
                            view! {
                                <button
                                    class=move || {
                                        let selected = if kind == "projects" {
                                            live.selected_project_id.get()
                                        } else {
                                            live.selected_session_id.get()
                                        };
                                        if selected.as_deref() == Some(selected_id.as_str()) {
                                            "context-row live-context-row is-selected"
                                        } else {
                                            "context-row live-context-row"
                                        }
                                    }
                                    type="button"
                                    on:click=move |_| {
                                        if kind == "projects" {
                                            actions.dispatch(WorkspaceAction::SelectProject(select_id.clone()));
                                        } else if kind == "fleet" {
                                            actions.dispatch(WorkspaceAction::SelectSession(select_id.clone()));
                                        }
                                    }
                                >
                                    <StatusDot tone /><span class="row-copy"><strong>{name}</strong><small>{detail}</small></span>
                                </button>
                            }
                        }).collect_view()}
                        <Show when=move || empty>
                            <div class="context-empty"><p>{empty_title}</p><span>"The native store returned no records."</span></div>
                        </Show>
                    </>
                }
            }}
        </div>
    }
}

#[component]
fn LiveCollectionInspector(kind: &'static str, inspector_open: RwSignal<bool>) -> impl IntoView {
    let live = expect_context::<LiveStatus>();
    let actions = expect_context::<WorkspaceActionSink>();
    let (title, _, _, _) = live_collection_copy(kind);
    view! {
        <div class="inspector-content">
            <header class="inspector-header simple-inspector-header"><div><h2>{title}</h2><p>"Native local records"</p></div><button class="icon-button" type="button" aria-label="Close details" on:click=move |_| inspector_open.set(false)><Icon path=crate::components::ICON_CLOSE /></button></header>
            <Show when=move || kind == "fleet" || kind == "projects">
                {move || live.snapshot.get().and_then(|snapshot| {
                    if kind == "projects" {
                        let project_id = live.selected_project_id.get()?;
                        let project = snapshot.find_project(&project_id)?.clone();
                        let ignored = project.ignored || snapshot.project_is_ignored(&project_id);
                        let membership_id = project_id.clone();
                        let mut sessions = snapshot
                            .sessions
                            .iter()
                            .filter(|session| session.project_id == project_id)
                            .cloned()
                            .collect::<Vec<_>>();
                        sort_sessions_for_display(&mut sessions);
                        let root = project.root_path.clone().unwrap_or_else(|| "No local root recorded".into());
                        return Some(view! {
                            <>
                            <section class="inspector-section">
                                <h3>"Selected project"</h3>
                                <dl class="detail-list">
                                    <div><dt>"Name"</dt><dd>{project.name.clone()}</dd></div>
                                    <div><dt>"Root"</dt><dd>{root}</dd></div>
                                    <div><dt>"State"</dt><dd>{project.state.clone()}</dd></div>
                                    <div><dt>"Membership"</dt><dd>{if ignored { "Hidden from the workbench" } else { "In the workbench" }}</dd></div>
                                    <div><dt>"Sessions"</dt><dd>{if ignored { "Still stored".to_string() } else { sessions.len().to_string() }}</dd></div>
                                </dl>
                            </section>
                            <section class="inspector-section">
                                <h3>"Owner membership"</h3>
                                <p class="inspector-note">"Ignoring hides this project from operating lists. Collected sessions stay in the local store. Discovery still runs."</p>
                                {if ignored {
                                    view! {
                                        <button class="secondary-button" type="button" on:click=move |_| actions.dispatch(WorkspaceAction::SetProjectIgnored { project_id: membership_id.clone(), ignored: false })>"Show in workbench"</button>
                                    }.into_any()
                                } else {
                                    view! {
                                        <button class="secondary-button" type="button" on:click=move |_| actions.dispatch(WorkspaceAction::SetProjectIgnored { project_id: membership_id.clone(), ignored: true })>"Ignore project"</button>
                                    }.into_any()
                                }}
                            </section>
                            <section class="inspector-section">
                                <h3>"Sessions"</h3>
                                {if ignored {
                                    view! { <p class="inspector-note">"Session rows stay off the workbench while this project is ignored."</p> }.into_any()
                                } else if sessions.is_empty() {
                                    view! { <p class="inspector-note">"No session metadata is stored for this project yet."</p> }.into_any()
                                } else {
                                    view! {
                                        <dl class="detail-list">
                                            {sessions.into_iter().map(|session| {
                                                let title = session_title(&snapshot, &session);
                                                let detail = session_detail(&snapshot, &session, false);
                                                view! {
                                                    <div><dt>{title}</dt><dd>{detail}" · "{session.state}</dd></div>
                                                }
                                            }).collect_view()}
                                        </dl>
                                    }.into_any()
                                }}
                            </section>
                            </>
                        }.into_any());
                    }
                    let session_id = live.selected_session_id.get()?;
                    let session = snapshot.sessions.iter().find(|session| session.id == session_id)?.clone();
                    let title = session_title(&snapshot, &session);
                    let agent = snapshot.agents.iter().find(|agent| agent.id == session.agent_id).map(|agent| agent.display_name.clone()).unwrap_or_else(|| session.agent_id.clone());
                    let project = snapshot.projects.iter().find(|project| project.id == session.project_id).map(|project| project.name.clone()).unwrap_or_else(|| session.project_id.clone());
                    let observed = relative_unix_ms(snapshot.generated_at_unix_ms, session.last_observed_at_unix_ms);
                    let provider = session.provider_session_id.clone().unwrap_or_else(|| "Not recorded".into());
                    Some(view! {
                        <section class="inspector-section">
                            <h3>"Selected session"</h3>
                            <dl class="detail-list">
                                <div><dt>"Title"</dt><dd>{title}</dd></div>
                                <div><dt>"Agent"</dt><dd>{agent}</dd></div>
                                <div><dt>"Project"</dt><dd>{project}</dd></div>
                                <div><dt>"State"</dt><dd>{session.state.clone()}</dd></div>
                                <div><dt>"Last observed"</dt><dd>{observed}</dd></div>
                                <div><dt>"Provider session"</dt><dd>{provider}</dd></div>
                            </dl>
                        </section>
                    }.into_any())
                })}
            </Show>
            <section class="inspector-section"><h3>"Source boundary"</h3><p class="inspector-note">"This panel reflects the persisted owner-device snapshot. Provider activity appears only after a connector reports and Utu stores it."</p></section>
            <section class="inspector-section"><h3>"Store"</h3><dl class="detail-list"><div><dt>"Records"</dt><dd>{move || live_collection_rows(&live, kind).len()}</dd></div><div><dt>"Integrity"</dt><dd>{move || live.snapshot.get().map(|snapshot| if snapshot.store.integrity_ok { "Healthy" } else { "Needs attention" }).unwrap_or("Unavailable")}</dd></div><div><dt>"Schema"</dt><dd>{move || live.snapshot.get().map(|snapshot| snapshot.store.schema_version.to_string()).unwrap_or_else(|| "—".into())}</dd></div></dl></section>
        </div>
    }
}

fn live_collection_copy(kind: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    match kind {
        "attention" => (
            "Attention",
            "Decisions and problems recorded locally",
            "Nothing needs attention",
            "Connector problems and agent requests will appear after they are observed and stored.",
        ),
        "projects" => (
            "Projects",
            "Outcomes stored on this owner device",
            "No local projects",
            "Add a local folder boundary to begin; no demonstration projects are shown as live.",
        ),
        _ => (
            "Fleet",
            "Running and observed agent sessions",
            "No agent sessions",
            "Sync for All imports Claude Code and Codex session metadata. Running sessions appear first.",
        ),
    }
}

fn live_collection_rows(live: &LiveStatus, kind: &str) -> Vec<(String, String, String, String)> {
    let Some(snapshot) = live.snapshot.get() else {
        return Vec::new();
    };
    match kind {
        "attention" => snapshot
            .attention
            .iter()
            .map(|item| {
                (
                    item.title.clone(),
                    item.title.clone(),
                    "Stored attention item".into(),
                    item.severity.clone(),
                )
            })
            .collect(),
        "projects" => snapshot
            .projects
            .iter()
            .map(|project| {
                let sessions = snapshot
                    .sessions
                    .iter()
                    .filter(|session| session.project_id == project.id)
                    .count();
                (
                    project.id.clone(),
                    project.name.clone(),
                    format!(
                        "{sessions} sessions · {}",
                        project
                            .root_path
                            .clone()
                            .unwrap_or_else(|| "No root path".into())
                    ),
                    project.state.clone(),
                )
            })
            .collect(),
        _ => {
            let mut sessions = snapshot.sessions.clone();
            sort_sessions_for_display(&mut sessions);
            sessions
                .iter()
                .map(|session| {
                    (
                        session.id.clone(),
                        session_title(&snapshot, session),
                        session_detail(&snapshot, session, true),
                        session.state.clone(),
                    )
                })
                .collect()
        }
    }
}

#[component]
fn UtilityRail(active_view: RwSignal<AppView>, context_open: RwSignal<bool>) -> impl IntoView {
    let actions = expect_context::<WorkspaceActionSink>();
    let live = expect_context::<LiveStatus>();
    view! {
        <nav class="utility-rail" aria-label="Application">
            <div class="utility-primary">
                <button class="app-mark" type="button" aria-label="Utu home" on:click=move |_| { active_view.set(AppView::Workspace); actions.dispatch(WorkspaceAction::SelectView("workspace")); }>
                    <AppMarkGlyph />
                </button>
                <button
                    class=move || rail_class(active_view.get() == AppView::Overview)
                    type="button"
                    aria-label="Overview"
                    title="Overview"
                    on:click=move |_| { active_view.set(AppView::Overview); context_open.set(false); }
                >
                    <Icon path=ICON_ORBIT />
                    {move || {
                        let running = live.snapshot.get().map(|s| s.sessions.iter().filter(|ses| ses.state == "running" || ses.state == "waiting").count()).unwrap_or(0);
                        if running > 0 {
                            view! { <span class="rail-badge">{running}</span> }.into_any()
                        } else {
                            view! {}.into_any()
                        }
                    }}
                </button>
                <button class=move || rail_class(active_view.get() == AppView::Workspace) type="button" aria-label="Workspace" title="Workspace" on:click=move |_| { active_view.set(AppView::Workspace); context_open.set(false); actions.dispatch(WorkspaceAction::SelectView("workspace")); }><Icon path=ICON_HOME /></button>
                <button class=move || rail_class(active_view.get() == AppView::Attention) type="button" aria-label="Attention" title="Attention" on:click=move |_| { active_view.set(AppView::Attention); context_open.set(false); actions.dispatch(WorkspaceAction::SelectView("attention")); }><Icon path=ICON_ATTENTION /><span class="rail-alert"></span></button>
                <button class=move || rail_class(active_view.get() == AppView::Fleet) type="button" aria-label="Fleet" title="Fleet" on:click=move |_| { active_view.set(AppView::Fleet); context_open.set(false); actions.dispatch(WorkspaceAction::SelectView("fleet")); }><Icon path=ICON_NODES /></button>
                <button class=move || rail_class(active_view.get() == AppView::Integrations) type="button" aria-label="Integrations" title="Integrations" on:click=move |_| { active_view.set(AppView::Integrations); context_open.set(false); actions.dispatch(WorkspaceAction::SelectView("integrations")); }><Icon path=ICON_PLUG /><span class="rail-alert rail-alert-soft"></span></button>
                <button class="rail-button compact-planned-action" type="button" disabled=true aria-label="Search · planned" title="Search is not wired yet"><Icon path=ICON_SEARCH /></button>
            </div>
            <div class="utility-secondary">
                <button class=move || rail_class(active_view.get() == AppView::Costs) type="button" aria-label="Costs" title="Costs" on:click=move |_| { active_view.set(AppView::Costs); context_open.set(false); }><Icon path=ICON_COST /></button>
                <button class=move || rail_class(active_view.get() == AppView::Settings) type="button" aria-label="Settings" title="Settings" on:click=move |_| { active_view.set(AppView::Settings); context_open.set(false); }><Icon path=ICON_SETTINGS /></button>
                <button class="owner-avatar" type="button" aria-label="Owner profile">"K"<StatusDot /></button>
            </div>
        </nav>
    }
}

#[component]
fn ContextRail(active_view: RwSignal<AppView>, read_only: bool) -> impl IntoView {
    let live = expect_context::<LiveStatus>();
    view! {
        <aside class="context-rail" aria-label="Workspace context">
            <div class="context-header">
                <div class="context-heading">
                    <h2>{move || active_view.get().label()}</h2>
                    <p>{move || active_view.get().subtitle(read_only)}</p>
                </div>
            </div>

            <div class="context-content">
                <Show when=move || active_view.get() == AppView::Workspace>
                    <WorkspaceContext read_only />
                </Show>
                <Show when=move || active_view.get() == AppView::Overview>
                    <OverviewContext />
                </Show>
                <Show when=move || active_view.get() == AppView::Attention>
                    <Show when=move || live.is_desktop() fallback=AttentionContext>
                        <LiveCollectionContext kind="attention" />
                    </Show>
                </Show>
                <Show when=move || active_view.get() == AppView::Projects>
                    <Show when=move || live.is_desktop() fallback=ProjectContext>
                        <LiveCollectionContext kind="projects" />
                    </Show>
                </Show>
                <Show when=move || active_view.get() == AppView::Fleet>
                    <Show when=move || live.is_desktop() fallback=FleetContext>
                        <LiveCollectionContext kind="fleet" />
                    </Show>
                </Show>
                <Show when=move || active_view.get() == AppView::Integrations>
                    <IntegrationsContext />
                </Show>
                <Show when=move || matches!(active_view.get(), AppView::Settings | AppView::Costs)>
                    <div class="context-section">
                        <div class="section-label"><span>{move || active_view.get().label()}</span></div>
                        <div class="context-empty"><p>{move || active_view.get().subtitle(read_only)}</p></div>
                    </div>
                </Show>
            </div>
            <div class="context-footer">
                <Show when=move || live.is_desktop() fallback=move || view! { <><span><StatusDot tone="attention" />"Demonstration data"</span><span class="footer-state footer-demo">"Not live"</span></> }>
                    <span><span class=move || format!("status-dot status-{}", live_phase_tone(live.phase.get())) aria-hidden="true"></span>{move || live_footer_label(&live)}</span>
                    <span class="footer-state">{move || live_store_label(&live)}</span>
                </Show>
            </div>
        </aside>
    }
}

#[component]
fn WorkspaceContext(read_only: bool) -> impl IntoView {
    let model = expect_context::<WorkspaceModel>();
    let actions = expect_context::<WorkspaceActionSink>();
    let live = expect_context::<LiveStatus>();
    view! {
        <div class="context-section workspace-projects">
            <div class="section-label"><span>"Projects"</span><button class="bare-plus" type="button" disabled=read_only title=if live.is_desktop() { "Add local project" } else { "Project creation requires the native owner app" } aria-label="Add project" on:click=move |_| actions.dispatch(WorkspaceAction::OpenCreateProject)>"+"</button></div>
            <Show when=move || live.snapshot.get().is_some() fallback=move || view! {
                <div class="context-loading-state">
                    <span class=move || if live.phase.get() == crate::workspace_data::LoadPhase::Loading { "spinner" } else { "status-dot status-attention" }></span>
                    <span>{move || live.phase.get().label()}</span>
                </div>
            }>
                {move || live.snapshot.get().map(|snapshot| snapshot.projects.iter().map(|project| {
                    let project_id = project.id.clone();
                    let project_id_action = project_id.clone();
                    let project_id_label = project_id.clone();
                    let project_name = project.name.clone();
                    view! {
                        <button
                            class=move || if live.selected_project_id.get().as_deref() == Some(project_id.as_str()) { "project-context-row is-selected" } else { "project-context-row" }
                            type="button"
                            on:click=move |_| actions.dispatch(WorkspaceAction::SelectProject(project_id_action.clone()))
                        >
                            <AgentAvatar initials="PR" tone="teal" size="md" />
                            <span><strong>{project_name}</strong><small><StatusDot tone="quiet" />{move || project_state_label(&live, &project_id_label)}</small></span>
                        </button>
                    }
                }).collect_view())}
            </Show>
            <Show when=move || !live.is_desktop()>
            {model.projects.iter().copied().map(|project| {
                view! {
                    <button
                        class=if project.id == model.active_project { "project-context-row is-selected" } else { "project-context-row" }
                        type="button"
                        on:click=move |_| actions.dispatch(WorkspaceAction::SelectProject(project.id.into()))
                    >
                        <AgentAvatar initials=project.initials tone=project.tone size="md" />
                        <span><strong>{project.name}</strong><small><StatusDot />{project.running}" running"<span class="context-waiting">{project.waiting}" waiting"</span></small></span>
                    </button>
                }
            }).collect_view()}
            </Show>
        </div>
        // Sessions are shown in the workspace sessions pane (middle column) on the live desktop
        // path. Keep the demo-mode section for the read-only web fallback.
        <Show when=move || !live.is_desktop()>
            <div class="context-section session-context">
                <div class="section-label"><span>"Sessions"</span></div>
                {model.sessions.iter().copied().map(|session| {
                    view! {
                        <button
                            class=if session.id == model.active_session { "session-context-row is-selected" } else { "session-context-row" }
                            type="button"
                            on:click=move |_| actions.dispatch(WorkspaceAction::SelectSession(session.id.into()))
                        >
                            <span class="session-state"><StatusDot tone=session.tone /></span>
                            <span><strong>{session.title}</strong><small>{session.agents}" · "{session.freshness}</small></span>
                            {session.unread.map(|unread| view! { <span class="unread-mark">{unread}</span> })}
                        </button>
                    }
                }).collect_view()}
            </div>
            <div class="context-empty"><p>"No background jobs"</p><span>"New agent activity will appear here."</span></div>
        </Show>
    }
}

#[component]
fn IntegrationsContext() -> impl IntoView {
    let model = expect_context::<WorkspaceModel>();
    let actions = expect_context::<WorkspaceActionSink>();
    let live = expect_context::<LiveStatus>();
    view! {
        // On live desktop, the main Integrations pane shows all CLI details —
        // avoid duplicating that here. Just show a minimal summary count.
        <Show when=move || live.is_desktop() fallback=move || view! {
            <><div class="context-section"><div class="section-label"><span>"Local CLI"</span><span class="count">"4"</span></div>{model.connectors.iter().copied().filter(|connector| connector.family == "Local CLI").map(|connector| { view! { <button class=if connector.id == "codex-cli" { "integration-context-row is-selected" } else { "integration-context-row" } type="button" on:click=move |_| actions.dispatch(WorkspaceAction::ConfigureConnector(connector.id.into()))><StatusDot tone=connector.tone /><span><strong>{connector.name}</strong><small>"Demo · "{connector.status}</small></span></button> } }).collect_view()}</div><div class="context-section"><div class="section-label"><span>"Cloud"</span><span class="count">"3"</span></div>{model.connectors.iter().copied().filter(|connector| connector.family == "Cloud").map(|connector| { view! { <button class="integration-context-row" type="button" on:click=move |_| actions.dispatch(WorkspaceAction::ConfigureConnector(connector.id.into()))><StatusDot tone=connector.tone /><span><strong>{connector.name}</strong><small>"Planned connector"</small></span></button> } }).collect_view()}</div></>
        }>
            <div class="context-section">
                <div class="section-label">
                    <span>"Observed"</span>
                    <span class="count">
                        {move || live.diagnostics.get().map(|r| r.connectors.len()).unwrap_or_default()}
                    </span>
                </div>
                <Show when=move || live.diagnostics.get().is_none()>
                    <div class="context-loading-state">
                        <span class="spinner"></span><span>"Checking CLIs"</span>
                    </div>
                </Show>
                {move || live.diagnostics.get().map(|report| {
                    let ready = report.connectors.iter().filter(|c| c.readiness == "ready").count();
                    let total = report.connectors.len();
                    view! {
                        <div class="integrations-context-summary">
                            <span class="state-label healthy">{ready}" ready"</span>
                            {if ready < total {
                                view! { <span class="state-label attention">{total - ready}" need attention"</span> }.into_any()
                            } else {
                                view! {}.into_any()
                            }}
                        </div>
                    }
                })}
            </div>
        </Show>
    }
}

#[component]
fn OverviewContext() -> impl IntoView {
    let live = expect_context::<LiveStatus>();
    view! {
        <div class="context-section">
            <div class="section-label">
                <span>"Active"</span>
                <span class="count">
                    {move || live.snapshot.get()
                        .map(|s| s.sessions.iter().filter(|ses| matches!(ses.state.as_str(), "running" | "waiting")).count())
                        .unwrap_or_default()}
                </span>
            </div>
            {move || {
                let snap = live.snapshot.get();
                let sessions: Vec<_> = snap.as_ref().map(|s| {
                    s.sessions.iter()
                        .filter(|ses| matches!(ses.state.as_str(), "running" | "waiting" | "problem"))
                        .cloned()
                        .collect()
                }).unwrap_or_default();
                sessions.into_iter().map(|session| {
                    let tone = session_state_tone(&session.state);
                    let title = snap.as_ref().map(|s| session_title(s, &session)).unwrap_or_else(|| session.id.clone());
                    let connector_id = snap.as_ref().and_then(|s| {
                        s.agents.iter().find(|a| a.id == session.agent_id).map(|a| a.connector_id.clone())
                    }).unwrap_or_else(|| session.agent_id.clone());
                    view! {
                        <div class="overview-context-row">
                            <AgentCliIcon connector_id=connector_id size="sm" />
                            <span><strong>{title}</strong></span>
                            <span class=format!("state-label {tone}")>{session.state.clone()}</span>
                        </div>
                    }
                }).collect_view()
            }}
            <Show when=move || live.snapshot.get().is_some_and(|s| s.sessions.iter().all(|ses| !matches!(ses.state.as_str(), "running" | "waiting" | "problem")))>
                <div class="context-empty"><p>"No active sessions"</p></div>
            </Show>
        </div>
    }
}

fn project_state_label(live: &LiveStatus, project_id: &str) -> String {
    let sessions = live
        .snapshot
        .get()
        .map(|snapshot| {
            snapshot
                .sessions
                .iter()
                .filter(|session| session.project_id == project_id)
                .count()
        })
        .unwrap_or_default();
    format!("{sessions} sessions")
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

fn live_phase_tone(phase: crate::workspace_data::LoadPhase) -> &'static str {
    use crate::workspace_data::LoadPhase;
    match phase {
        LoadPhase::Ready => "healthy",
        LoadPhase::Loading | LoadPhase::Empty => "attention",
        LoadPhase::Error => "problem",
        LoadPhase::Demo => "quiet",
    }
}

fn live_footer_label(live: &LiveStatus) -> String {
    let phase = live.phase.get();
    if phase == crate::workspace_data::LoadPhase::Ready {
        let projects = live
            .snapshot
            .get()
            .map(|snapshot| snapshot.projects.len())
            .unwrap_or_default();
        format!("Local store · {projects} projects")
    } else {
        phase.label().into()
    }
}

fn live_store_label(live: &LiveStatus) -> String {
    live.snapshot
        .get()
        .map(|snapshot| {
            if snapshot.store.integrity_ok && snapshot.store.foreign_keys_enabled {
                format!("Schema {} · healthy", snapshot.store.schema_version)
            } else {
                "Store needs attention".into()
            }
        })
        .unwrap_or_else(|| live.phase.get().label().into())
}

fn rail_class(active: bool) -> &'static str {
    if active {
        "rail-button is-active"
    } else {
        "rail-button"
    }
}

fn agent_system_tone(live: &LiveStatus) -> &'static str {
    let Some(snapshot) = live.snapshot.get() else {
        return live_phase_tone(live.phase.get());
    };
    if snapshot
        .sessions
        .iter()
        .any(|s| s.state == "waiting" || s.state == "problem")
    {
        return "attention";
    }
    if snapshot.sessions.iter().any(|s| s.state == "running") {
        return "healthy";
    }
    "quiet"
}

fn titlebar_status_label(live: &LiveStatus) -> String {
    let Some(snapshot) = live.snapshot.get() else {
        return live.phase.get().label().into();
    };
    let running = snapshot
        .sessions
        .iter()
        .filter(|s| s.state == "running")
        .count();
    let waiting = snapshot
        .sessions
        .iter()
        .filter(|s| s.state == "waiting")
        .count();
    let problem = snapshot
        .sessions
        .iter()
        .filter(|s| s.state == "problem")
        .count();
    if problem > 0 {
        format!("{problem} sessions need attention")
    } else if waiting > 0 {
        format!("{running} running · {waiting} waiting")
    } else if running > 0 {
        format!("{running} running")
    } else {
        "All agents idle".into()
    }
}
